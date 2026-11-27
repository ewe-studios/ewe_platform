---
feature: "Sandbox follow-ups"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "2"
depends_on: []
crates: ["foundation_sandbox", "foundation_deployment_docker", "foundation_deployment_cloudflare"]
---

# Feature 02: Sandbox follow-ups

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Close the ten findings of spec 77 that this spec's requirements list. Each row is one issue.
The order is severity first, then effort. No row changes a public API unless it says so.

## 2. Tasks

| # | Issue | Problem | Change | Severity |
|---|---|---|---|---|
| T1 | #977 | The slave unpack writes each chunk with a blocking call and blocks on the child's `ready` line, so a pool worker is held for the whole unpack | Write through a handle that parks on writability, read `ready` as a stream, and test a 100 MiB archive on the single-threaded engine | medium |
| T2 | #969 | The nested watcher treats any `get` error as "the child is gone", so a transport blip cleans up a live child | Treat only `NotFound` and `SandboxStopped` as gone; retry a transport error until `boot_timeout` | medium |
| T3 | #966 | Two flattened Docker structs both name `Devices`; setting one writes `null` | Stop flattening both; one field carries `Devices`, and the workaround in `sandbox_body` goes | medium |
| T4 | #970 | A nested child's `terminate` ignores the outer grace | Wait on the child up to the outer deadline, then `kill` | low |
| T5 | #935 | A promoted Worker URL lacks the account subdomain | Read the Worker subdomain, or take the first plan route | medium |
| T6 | #1013 | `e2e_build_network` always skips and uses `127.0.0.1:9` | Register it for the libkrun host and start a reachable local server | medium |
| T7 | #1022 | The layer stream test serves a plain body, so gzip decode is untested | Serve a real gzip layer | low |
| T8 | #1023 | The differential shell listing never ran and may miss busybox tools | Run it on the Linux host, with forms busybox has, or list through the harness | medium |
| T9 | #972 | The no-KVM scenario has no CI proof | Add **both** forms: (a) start the parent without the `/dev/kvm` device and with `--control`, then probe it (runs on today's KVM runner); (b) the real no-KVM daemon form, which runs on a Mac (Docker there has no `/dev/kvm`) and on any runner without the device | low |
| T10 | #1008 | A clippy run without `--no-deps` stops in `foundation_compact` and `foundation_jsonschema` | Clear the 19 pedantic lints | low |

## 3. Design decisions this feature records

- **#978 is decided: the code stays on `exec`.** The unpack child is an `exec` of
  `agent-slave --unpack-child`, not `fork` plus `chroot`. The owner decided this on
  2026-11-27. A threaded process cannot call `fork` and run arbitrary code safely: the child
  may allocate, lock a mutex that another thread held, or call into code that is not
  async-signal-safe. `exec` starts one clean thread and a fresh address space, which is the
  only safe way to change the root. Feature 12's design text changes to the `exec` form; no
  code change is made.
- **#1013 is coverage, not a capability.** The Firecracker guest has no network, so the
  network case belongs to libkrun with block support. The test keeps a skip reason for every
  host that cannot run it.

## 4. Tests

Each task adds or changes the test named in its issue. The feature is done when the issue is
closed with that test named in its closing comment.

## 5. Verify

```bash
cargo fmt -p foundation_sandbox -p foundation_deployment_docker -p foundation_deployment_cloudflare -- --check
cargo clippy -p foundation_sandbox --features "docker firecracker slave testing" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_sandbox --features "docker firecracker slave testing" --profile uat
cargo test -p foundation_deployment_docker --profile uat
cargo test -p foundation_deployment_cloudflare --profile uat
```

## 6. Done when

#977, #978, #966, #969, #970, #972, #1013, #1022, #1023, #935 and #1008 are closed, each in
the change that fixes it.
