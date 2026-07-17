---
feature: "VPS hardening"
description: "One hardening policy applied to every provider's VPS — key-only SSH, no password auth, firewall with default deny, unattended security updates — applied at first boot and verified over SSH afterwards"
status: "not-started"
priority: "high"
phase: 2
depends_on: ["04-hardening-policy"]
estimated_effort: "medium"
created: 2026-07-17
---
# Feature 05: VPS Hardening

## What

The shared hardening step every provider's `deploy` runs. Same policy on
DigitalOcean, Hetzner and Linode — only the delivery differs (each vendor's
cloud-init hook), so this is one crate-level module the three provider crates
call, not three copies.

A box that is reachable but not hardened is **not deployed**. `deploy` does not
return until this has been applied *and verified*.

## Why it is its own feature

Because the policy is the interesting part, not the plumbing, and because it is
the one part of this spec that is **fully verifiable locally**. Everything here
can be asserted against the docker-in-docker + sshd fixture that
`foundation_deployment_docker::ssh_transport_tests` already uses: real sshd, real
`sshd_config`, real `authorized_keys`, real package installs. No cloud account,
no cost.

## Scope

Exact policy is [decision 04](../../decisions/04-hardening-policy.md) — it is
**open**, and this feature should not start until it is resolved. The shape:

1. **Apply** — at first boot via cloud-init `user_data` (before sshd serves
   anyone), *and/or* post-boot over SSH. Decision 04 asks which; the leaning is
   "cloud-init applies, SSH verifies", with a caveat that a single applied-and-
   tested path may be better than two paths where one is untestable here.
2. **Verify** — read the box's state back over SSH and assert it. A silent
   cloud-init failure must not pass as a hardened box.
3. **Fail loudly** — if verification fails, `deploy` fails. Per decision 03, a
   partial failure should not strand a billing instance.

## The verifier is the deliverable

Each assertion is a test:

| Assertion | How |
|---|---|
| password auth is off | `sshd_config` says `PasswordAuthentication no`; a password login is refused |
| our key works | key login succeeds |
| only our key is trusted | `authorized_keys` contains exactly the deploy key |
| root login policy | `PermitRootLogin` matches the decision |
| firewall default-deny | firewall status; only `ssh_port`/80/443 (+443/udp) open |
| sshd moved off 22 | a connection on `ssh_port` authenticates; **22 refuses** |
| the switchover never strands us | both ports listen → new port verified → only then 22 closes |
| **Docker does not bypass the firewall** | a container published on a port the firewall denies is **not** reachable from off-box |
| security updates on | `unattended-upgrades` enabled, security pocket |
| auto-reboot configured | `Automatic-Reboot` + the window from the policy |
| **containers survive a reboot** | restart the daemon/host in the fixture → the container comes back |
| fail2ban running | the sshd jail is active |
| fail2ban never bans our sources | `ignoreip` contains loopback + every entry in `fail2ban_ignore`; a failed auth from an ignored source does **not** ban it |
| Docker answers | `docker version` over SSH |

The Docker/firewall row is the one that will bite: **Docker writes its own
iptables rules and publishes past `ufw`**, so "default deny" can be true of the
host and false of exactly the containers we deploy. If that is not asserted, the
firewall is decorative for our workload.

## Verification

- Against the **local sshd fixture** for everything above. This is a real sshd
  with a real config — the same fixture that proved `connect_ssh` works in
  spec-53's review.
- The **cloud-init path itself cannot be verified here** (a container is not a
  cloud-init'd VM). If the policy lives in cloud-init, the local tests verify the
  *verifier*, and the YAML stays unproven until someone boots a real VPS. Decision
  04 weighs that.

## Prerequisite this feature carries

Auto-reboot (decision 04) is useless — worse, harmful — without container restart
policies, and **spec-53's `ContainerConfig` has none** (the Docker client's
`HostConfig.RestartPolicy` does; nothing wires it). So this feature includes:

- `ContainerConfig::restart_policy(..)` → `HostConfig.RestartPolicy`
- everything this spec deploys defaults to `unless-stopped`
- a test that restarts the daemon/host in the fixture and asserts the container
  returns

Without it, the 03:00 security reboot brings the box back with every service down.

## Acceptance criteria

- [ ] Decision 04 resolved
- [ ] One hardening module, used by all three provider crates
- [ ] Applied before the box is usable; `deploy` does not return un-hardened
- [ ] Every assertion in the table above is a test against the local sshd fixture
- [ ] The Docker-past-the-firewall case is explicitly asserted
- [ ] A failed verification fails `deploy` and does not leave a billing instance
- [ ] Bantimes are short and `fail2ban_ignore` is honoured — a self-ban should be waited out, not need a console trip (though the vendor console is always the backstop)
- [ ] Idempotent: re-running hardening on a hardened box is a no-op
- [ ] `ssh_port` (default 2222) is one policy field, rendered into sshd, both firewall layers, the verifier and the host output — never hard-coded
- [ ] `fail2ban_ignore` (IPs / CIDRs / hostnames) renders into `ignoreip`, loopback always retained; asserted by failing auth from an ignored source and confirming no ban
- [ ] The port switchover listens on both, verifies the new one, and only then closes 22 — and the fixture exercises that sequence
