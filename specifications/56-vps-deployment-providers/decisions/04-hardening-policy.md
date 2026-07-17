# 04 — Hardening policy

**Date:** 2026-07-17
**Status:** **Resolved** (2026-07-17, owner)

## The question

The owner's requirement: **"deploy, setup and harden these VPS instances for
use."** A box that is reachable but not hardened is not deployed. So: what does
"hardened" mean concretely, and *where* is it applied?

This needs to be pinned down precisely, because a default-configured VPS with a
public IP is attacked within minutes, and because "hardened" is the kind of word
that passes review while meaning nothing.

## Where hardening can happen

| Layer | Runs | Good for | Limits |
|---|---|---|---|
| **Provider API** | at create | vendor firewalls, SSH keys registered with the account, private networking, disabling password auth at image level | vendor-specific; not every provider offers each control |
| **cloud-init `user_data`** | first boot, before sshd serves anyone | the whole base policy, applied *before* the box is exposed | opaque failures — if it errors you get a running box in an unknown state |
| **Post-boot over SSH** | after we can log in | verifiable (we can read back what we set), fixable, idempotent | there is a window between boot and hardening, and it needs a way in |

### **Resolved: cloud-init applies (best effort), SSH re-applies + verifies** (owner, 2026-07-17)

```
create(user_data: #cloud-config …)   -> hardened before sshd serves anyone
        |
        +-- ssh in as root -> apply again (no-op if already done) -> verify -> assert
```

Both, deliberately:

- **cloud-init closes the exposure window.** Without it the box sits with vendor
  defaults and a public IP from first boot until we finish — and a
  default-configured VPS is attacked within minutes.
- **the SSH path is the authoritative one**, and it is the one we can test. It
  applies *and* verifies, against the local docker-in-docker + sshd fixture, with
  no cloud account.

**The property this buys — worth stating plainly:** because the SSH path
re-applies, **a broken cloud-init cannot produce an unhardened box.** It can only
fail to close the window early. So correctness never depends on the one path we
cannot test here. That is why this is worth the extra work over "cloud-init
applies, SSH verifies" — there, the applying path is untestable *and* load-bearing.

It also makes Linode's gap a non-event: regions/images without the Metadata
service cannot take `user_data` at all (feature 03). With this shape that is a
lost optimisation, not a lost policy — log that `user_data` was unavailable and
carry on.

**The cost, and how it is contained:** the policy would be expressed twice — YAML
and shell — and two expressions drift. So, exactly as with the firewall:
**`HardeningPolicy` is the single source of truth, and both renderings are
generated from it.** Nobody hand-writes the cloud-config, and nobody hand-writes
the SSH commands. A policy field that only one renderer honours is a bug the
tests should catch (assert the emitted YAML and the applied state agree on every
field).

## Candidate policy (to confirm)

### SSH
- Key-only: `PasswordAuthentication no`, `ChallengeResponseAuthentication no`,
  `KbdInteractiveAuthentication no`.
- `PermitRootLogin prohibit-password` (or `no`, with a sudo user — see below).
- Our deploy key in `authorized_keys`, and **only** it.
- **Port: moved off 22** — **resolved** (owner, 2026-07-17). Default **2222**,
  carried as `HardeningPolicy::ssh_port` so it is one field, not a constant.

  What it buys: drive-by scan noise drops. What it costs — and what the
  implementation must handle:

  **1. The port becomes state every caller needs.** `connect_ssh` already takes it
  (`ssh://user@host[:port]`; `Host::resolve` accepts `[user@]alias[:port]` —
  verified 2026-07-17), but everything must now pass it: the deployables' host
  output, both firewall renderings, the verifier, and the local fixture (which
  should mirror the real port, or the tested path is not the shipped one).

  **2. The switchover can lock us out.** Hardening arrives on **22** and must end
  on **2222** — and the connection doing the work is the one being moved. Naive
  order (rewrite `Port 2222`, restart sshd) drops our own session and every later
  step dials a closed port. Safe sequence:

  ```
  Port 22 + Port 2222        # sshd listens on BOTH (OpenSSH allows repeated Port)
  restart sshd
  verify: a NEW connection on 2222 authenticates      # prove the new door opens
  remove Port 22, restart, re-verify on 2222          # only then close the old one
  ```

  Firewalls follow the same order: open 2222 *before* dropping 22, in both layers.
  Get it backwards and the box is unreachable — recoverable only via the vendor's
  console, and `destroy` (decision 03) is what saves the bill.

  **3. It is worth saying what this is not.** Moving the port does not stop an
  attacker who scans; it stops noise. The boundary is still key-only auth.

### Users — **resolved: root hardens, then a docker-group user; both pathways offered** (owner, 2026-07-17)

The sequence follows what the vendor actually hands us — a root login — and moves
off it:

1. **root hardens.** It is the only account that exists at first boot, and the
   work (sshd config, user creation, firewall, packages) is root's anyway.
2. **Hardening creates the deploy user** — key installed, **`docker` group**,
   **passwordless sudo** (`NOPASSWD`).
3. **Everything after hardening uses that user.** `connect_ssh("ssh://ewe@host")`,
   the bootstrap, `docker system dial-stdio` — all of it.

**Both pathways stay available**: a caller who wants root-based logins can have
them. So the user model is a field on the policy, not a hard-coded posture:

```rust
HardeningPolicy {
    login: LoginUser::Deploy { name: "ewe".into() },   // default: create + use it
    // or
    login: LoginUser::Root,                            // stay on root
    ..
}
```

**What this obliges:**

- `SshHardening` must accept **two identities**: the one it logs in *as* (root, at
  first boot) and the one it *creates*. Its own precondition is "I can reach this
  box as root"; its output is "you can now reach it as `ewe`".
- Whether root login is then **disabled or left at `prohibit-password`** follows
  the chosen pathway — `LoginUser::Root` obviously cannot disable it. Default for
  `Deploy`: keep `prohibit-password` (key-only) rather than `no`, so a broken
  deploy user does not lock us out of a box we are still paying for. (Say if you
  want `no` once the deploy user is verified working.)
- **Be honest about the gain:** `docker` group membership *is* root-equivalent
  (anyone in it can `docker run -v /:/host`), and passwordless sudo is root on
  request. So this is conventional and audit-friendly, not a privilege boundary —
  it does not mean a compromised deploy key is less than full control. The real
  boundary remains "who holds the key".
- The deploy user must exist **before** anything tries to use it — which is why
  this lives in hardening, not bootstrap.

### Firewall — **resolved: both, provider outside + host inside** (owner, 2026-07-17)

Default deny inbound; allow **22, 80, 443, and 443/udp** (HTTP/3 — spec-53 F19
ships QUIC). Enforced in two places:

| Layer | Enforced by | Survives a root compromise? | Verifiable locally? |
|---|---|---|---|
| **Outer** | the vendor's firewall (Hetzner Firewalls / DO Cloud Firewalls / Linode Cloud Firewalls) — in front of the NIC | **yes** — a compromised box cannot switch it off | **no** — mock only; the fixture is a container |
| **Inner** | `ufw`/`nftables` **+ Docker's `DOCKER-USER` chain** | no (root can flush it) | **yes** — fully, on the dind + sshd fixture |

They cover each other's blind spot: the outer one is the real boundary, the inner
one is the one we can actually prove works.

**`DOCKER-USER` is not optional.** Docker writes its own iptables rules and
publishes **past** `ufw`, so a plain host firewall's "default deny" silently does
not apply to exactly the containers we deploy — a `-p 9000:9000` container is
reachable from the internet while `ufw status` cheerfully reports deny. Docker
consults `DOCKER-USER` before its own publish rules, which is the documented hook:

```
ufw default deny incoming; ufw allow 22,80,443,443/udp
iptables -I DOCKER-USER ! -s <allowed> -j DROP
```

**The cost this buys, and how to keep it from biting:** two rule sets can drift,
and a port open in one but not the other is a miserable debug ("why is this
closed?"). So:

- **one source of truth** — the allowed-ports list lives in `HardeningPolicy`, and
  both layers are rendered from it; nobody hand-writes either side;
- **the verifier asserts both** (§verified below): the inner one directly, the
  outer one by reading the vendor's firewall back through the API and comparing it
  to the same list.

**The test that matters** — and the one that would catch a `DOCKER-USER` mistake —
is not "is ufw enabled". It is: **run a container publishing a port that is not on
the allow-list, and assert it is unreachable from off-box.** That runs on the local
fixture, no cloud account.

### Updates — **resolved: unattended security upgrades, with auto-reboot** (owner, 2026-07-17)

`unattended-upgrades` on the security pocket, **and** allowed to reboot when a
patch needs it (a kernel CVE that sits in pending-reboot forever is not patched).

**This has a hard prerequisite, and it is not in place.** A box that reboots at
03:00 comes back with **every container down** unless they carry a restart policy
— and spec-53's `ContainerConfig` **has none**:

| | State (checked 2026-07-17) |
|---|---|
| `HostConfig.RestartPolicy` in the Docker client | **exists** (`foundation_deployment_docker`, generated) |
| `ContainerConfig::restart_policy(..)` in the platform | **missing** — nothing sets it |

So auto-reboot as chosen would silently trade "unpatched box" for "box up, service
down at 3am". Before it ships:

- add `restart_policy` to `ContainerConfig`, wired to `HostConfig.RestartPolicy`
  (the client side already supports it — this is the same "capability exists,
  never wired" shape spec-53's audit kept finding);
- anything this spec deploys sets `unless-stopped` (or `always`);
- **the test is the honest one**: reboot the host in the fixture (or restart the
  daemon), and assert the container comes back.

Reboot window: 03:00 by default, and it should be a policy field — a fixed hour is
someone else's peak.

### fail2ban — **resolved: in** (owner, 2026-07-17)

Included. Worth being clear-eyed about what it buys, so nobody mistakes it for the
boundary: with `PasswordAuthentication no` there is **nothing to brute-force** —
its value here is log-noise reduction and slowing scanners, not stopping the
attack that matters (a stolen deploy key).

**Never-ban list — `ignoreip`.** Verified against fail2ban's shipped `jail.conf`
(2026-07-17):

> *"`ignoreip` can be a list of IP addresses, CIDR masks or DNS hosts. Fail2ban
> will not ban a host which matches an address in this list. Several addresses can
> be defined using space (and/or comma) separator."*

So it takes a **set**, not one address: individual IPs, **CIDR ranges**
(`203.0.113.0/24`) or DNS hostnames, in `[DEFAULT]` (every jail) or per-jail. That
becomes a policy field:

```rust
HardeningPolicy {
    fail2ban_ignore: vec!["203.0.113.7".into(), "198.51.100.0/24".into()],
    ..
}   // -> ignoreip = 127.0.0.1/8 ::1 203.0.113.7 198.51.100.0/24
```

Loopback stays in the list (fail2ban's own default is `127.0.0.1/8 ::1`) —
dropping it lets the box ban itself.

Two things it obliges:

- **do not let it lock us out.** A flapping deploy key or a retrying CI runner can
  trip the sshd jail against *our own* address, and the box is then unreachable
  until the bantime expires — on a machine that is billing. Put the deployer's
  source in `fail2ban_ignore` where it is known and stable, and keep bantimes
  short. Where the source is **not** stable (CI runners on rotating egress IPs, a
  laptop on hotel wifi), a static list does not help: that is what
  **`ignorecommand`** is for — an external command handed the IP, returning true to
  ignore — if we ever need it. Do not pretend a static list covers a dynamic
  source.
- it is one more daemon to install and verify, so the verifier asserts it is
  running **and that `ignoreip` contains what the policy asked for** (§verified).

### Not proposed (say if wanted)
- SELinux/AppArmor profiles, auditd, CIS-benchmark conformance.

## What "verified" means here

Every item above is assertable against the local fixture:

- read back `sshd_config` (`PasswordAuthentication no`, `PermitRootLogin`);
- assert a password login is refused and a key login succeeds;
- assert `authorized_keys` holds exactly our key;
- assert the firewall's default policy and open ports;
- assert `docker version` answers.

None of that needs a cloud account.

Two things still cannot be verified locally, and neither is load-bearing:

- **the cloud-init path's effect** — the fixture is a container, not a cloud-init'd
  VM. It stays unproven until a live run, but per the resolution above the SSH path
  re-applies, so a broken cloud-init costs the early window, not the policy. What
  *is* testable: the emitted YAML is deterministic, so assert it against the policy
  (and that both renderings agree field-for-field).
- **the vendor firewall** — mock-only, read back through the API and compared to
  the same port list the host firewall is rendered from.

## To resolve

1. Root-only or a non-root sudo user?
2. ~~`ufw`/`nftables`, provider firewall, or both — and Docker writing past the
   host firewall?~~ — **resolved**: both; one port-list in the policy renders both
   layers; `DOCKER-USER` closes the Docker bypass; the proof is a published-port
   container being unreachable.
3. ~~Unattended upgrades: on? auto-reboot?~~ — **resolved**: both on. Blocked on
   adding `restart_policy` to spec-53's `ContainerConfig` first, or a 03:00 reboot
   takes every container down with it.
4. ~~cloud-init applies + SSH verifies, or SSH applies (one tested path)?~~ —
   **resolved**: cloud-init applies (best effort, closes the window) **and** SSH
   re-applies idempotently + verifies (the tested, authoritative path). One
   `HardeningPolicy` renders both.
5. ~~Anything from the "not proposed" list?~~ — **resolved**: fail2ban is in
   (with a whitelist for the deployer); SELinux/auditd/CIS are out.
