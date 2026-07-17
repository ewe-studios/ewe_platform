# 04 — Hardening policy

**Date:** 2026-07-17
**Status:** Open

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

The window matters: with cloud-init the box is hardened before sshd accepts its
first connection; post-boot, it is exposed for however long boot takes.

Proposal: **cloud-init applies the policy, post-boot SSH verifies it** — belt and
braces. Cloud-init closes the window, and the verify step means a silent
cloud-init failure cannot masquerade as a hardened box. Every assertion the
verify step makes is also a test we can run against the docker-in-docker + sshd
fixture, with no cloud account.

## Candidate policy (to confirm)

### SSH
- Key-only: `PasswordAuthentication no`, `ChallengeResponseAuthentication no`,
  `KbdInteractiveAuthentication no`.
- `PermitRootLogin prohibit-password` (or `no`, with a sudo user — see below).
- Our deploy key in `authorized_keys`, and **only** it.
- Keep port 22? Moving it stops log noise, not attackers, and it complicates
  every downstream `connect_ssh`. Proposal: leave it.

### Users
- Root-only, or a non-root sudo user (`ewe`) with root login disabled? The latter
  is conventional, but every downstream call (`connect_ssh`, the Docker install,
  `docker system dial-stdio`) then needs sudo or docker-group membership. Docker
  group membership is root-equivalent, so the security gain is thinner than it
  looks.

### Firewall
- Default deny inbound, allow 22 + the proxy's ports (80/443, and 443/udp for
  HTTP/3 — spec-53 F19 ships QUIC).
- `ufw`, `nftables`, or the **provider's** firewall? A provider firewall sits
  outside the box, so a compromised box cannot switch it off — but it is
  vendor-specific and does not exist in the local fixture, which makes it
  untestable here.
- **Docker publishes past `ufw`** by writing its own iptables rules — a
  well-known trap. Any host-firewall policy has to account for that, or
  "default deny" silently doesn't apply to exactly the containers we deploy.

### Updates
- `unattended-upgrades` for security updates. Auto-reboot? A box that reboots
  under a running deployment is its own kind of outage.

### Not proposed (say if wanted)
- fail2ban (little value when password auth is off), SELinux/AppArmor profiles,
  auditd, CIS-benchmark conformance.

## What "verified" means here

Every item above is assertable against the local fixture:

- read back `sshd_config` (`PasswordAuthentication no`, `PermitRootLogin`);
- assert a password login is refused and a key login succeeds;
- assert `authorized_keys` holds exactly our key;
- assert the firewall's default policy and open ports;
- assert `docker version` answers.

None of that needs a cloud account. What cannot be verified locally: the
provider-side firewall and the cloud-init path itself (the fixture is a container,
not a cloud-init'd VM) — so if hardening lives in cloud-init, the local tests
verify the *post-boot verifier*, and the cloud-init YAML stays unproven until
someone runs it on a real VPS. That argues for keeping the policy in a form the
verifier can also *apply*, so one code path is tested and the cloud-init is an
optimisation.

## To resolve

1. Root-only or a non-root sudo user?
2. `ufw`/`nftables` on the box, provider firewall, or both — and how do we handle
   Docker writing past the host firewall?
3. Unattended upgrades: on? auto-reboot?
4. cloud-init applies + SSH verifies, or SSH applies (one tested path)?
5. Anything from the "not proposed" list that should be in?
