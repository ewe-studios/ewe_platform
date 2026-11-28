# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | in review: pull request #1144 fixes the Cloudflare create stall; #1055 (moto) merged; T5 (Vault Transit) and T6 (WebCrypto) are in pull request #1225 |
| 01 DST fidelity: the real registry | stages A merged (#1116); stage B #1130 and stage C #1136 in review |
| 02 Sandbox follow-ups | in review: pull request #1111, 12 issues closed, #1104 fixed |
| 03 GCP KMS over gRPC | in review: pull request #1121, conflicts resolved |
| 04 Live cloud resources with the CLI | done: pull request #1103 merged; four live tests passed on 2026-11-28 |
| 05 Remove the auth back edge from connectrpc | planned: after #1121, fixes #1107 and #756 |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Merged on 2026-11-28

#1116 (the `Wait` re-drive), #1055 (moto for AWS KMS), #1049 (the DST workspace isolation),
#1103 (the live AWS and Google Cloud KMS proof), #1102 (the three master failures), #1075
(the wasm `has_root` fix) and #1145 (the Linux `is_ok_and` repair of #1102).

Master red for Linux after #1102: its clippy rewrite called `is_some_and` on a `Result`, and
`control_from_cmdline` compiles only on Linux. #1145 repairs it. macOS clippy cannot see
`cfg(target_os = "linux")` code; the amd64 CI image can.

## Feature 01: the deterministic simulation test (DST) turn hang

- **Stage A** (merged): `Stream::Wait` means "the queue is empty; run the producer and check
  again". The parking adapters treated it as `Pending` and parked on a wake that a
  single-threaded engine never sends. `WakeableStream` gained `drives_its_producer()` and
  `can_make_progress()`; the five adapters re-poll a driving stream with a bounded budget
  (`DEFAULT_WAIT_REDRIVE_BUDGET`, 100 on native, 1 on wasm). It also fixed #1108 and #1109.
- **Stage B** (#1130): the real `LocalInstanceRegistry` and `AgentSession` run under the
  simulator; `ScriptedCatalog` is the only fake. `foundation_ai` splits `testing-mock` from
  `testing`, and `foundation_agent_rpc` gains `dst`. Closes #908.
- **Stage C** (#1136): `run_until` no longer counts the executing task (#1128); the adapters
  also re-drive `Init` and `Pending` (#1129). The DST turn now goes through
  `AgentServiceClient::chat_stream` over the in-memory network.

## Feature 02: sandbox follow-ups

Pull request #1111 closes #966, #969, #970, #972, #935, #1013, #1022, #1023, #977, #978,
#1008 and #1106. #1095 (the Firecracker guest network) waits for a Linux host with KVM.

**#1104 root cause:** `tests/slave/support.rs` started a second valtron pool and leaked the
process-wide lifecycle guard with `mem::forget(initialize_pool(1, Some(4)))`. Every later
`pool()` call blocked on the gate, which held `serve::tcp_accepts_the_token` and
`serve::tcp_needs_token` for two hours. The tests now use the one shared test pool.

Linux run in the amd64 CI image: 311 passed, 19 failed. Every failure is an emulation limit
(one syscall qemu does not implement, and the fixed 5 s call deadline); #1137 records it.

## Feature 00 task T2: the Cloudflare create (2026-11-28)

The create timeout has two causes.

1. `ControlService::create` pulled the create progress iterator again in the same poll after
   `Stream::Wait`. The iterator is a driving valtron stream: `Wait` means that the queue is
   empty and that it must run again. On the single-threaded wasm host that loop starved the
   event loop, so the create never finished and the Worker stopped answering. The service
   now parks on the injected timer for `SandboxLimits::create_poll_interval` (4 ms). The
   Worker installs a JavaScript `setTimeout` timer in its `Ctx`: the executor's wasm timer
   drove the engine from inside the poll, re-entered the create task at
   `WorkerContainer::running`, and crashed the wasm instance on the runner (`Critical
   error`, then `Cannot invoke closure from previous WASM instance`). A self-wake instead
   wedged the local Worker, so the timer remains. The test
   `tests/control/service_create.rs` fails on the old code.
   The remote create uses the Worker's container boot deadline
   (`CloudflareConfig::boot_timeout`, 60 s) for the whole stream. The runner's create took
   23.8 s, so the unary defaults (5 s, then 10 s) were too short.
2. The slave did not detect the Cloudflare container runtime: the runtime writes neither
   `/.dockerenv` nor `/run/.containerenv`. The slave ran the virtual-machine boot duties,
   failed, and never listened on the slave port. It now also treats the `CLOUDFLARE_*`
   environment as a container (issue #1133).

Local result on macOS with `wrangler dev --local`: a create returns `Created` and the Worker
stays healthy. The local host runs the amd64 container under emulation, so the timings
depend heavily on the machine load.

The three follow-up defects are fixed on the same branch.

- #1140: the Worker route reads the sandbox identifier from one Connect envelope, so
  `Exec` and `FileRead` reach the Durable Object. A unary call keeps its bare JSON body.
- #1141: the Connect client decodes a gzip unary body that carries no `Content-Encoding`,
  and a body that a proxy compressed twice. The edge that fronts the local Worker
  compresses a chunked body and omits the header.
- #1142: the sandbox Durable Object builds its model configuration from `MODEL_*`, so a
  model spec reaches the model driver.

A second runner run (job `140770667220`) passed 7 tests: creates, `Exec` and the
container `Get` work, and the closure errors are gone. Two failures remained.

- A stop crash: `cleanup` asked `ContainerApi::running` on a container that had stopped.
  The Cloudflare host throws `Critical error` instead of answering false, and the Durable
  Object went down. The substrate handle now remembers that it destroyed the container,
  destroys at most once, and never reads the host state. A fake container that panics on
  `running` proves the old code fails.
- The 5 s call deadline: the runner's first `Exec` stream ended at 5.1 s because the
  transport read timeout was `rpc_timeout`, and `FileWriteBegin` answered 504 at 5.0 s
  because the slave call used `rpc_timeout`. `CloudflareConfig::call_timeout` (default
  30 s) is now the deadline for one call; the Durable Object applies it and the remote
  client uses it for `rpc_timeout`.

A third runner run (job `140784573393`) passed 11 tests: creates, `Exec`, the container
`Get`, the file writes and the model sandbox work. Four failures remained, now addressed.

- The index Durable Object read a `js_sys::Map` with `Object::values`, so `List` was always
  empty on a real Worker. It iterates the Map values now.
- An `Exec` stream error was replaced with `StreamLost`, which hid its cause. The reported
  error reaches the caller now, so the next runner log names the stage.
- The stop test counted the runtime's `proxy-everything` sidecar: its name contains
  `ewe-sandbox` and it stays after a stop. The e2e helper counts only the workload
  container.
- A relative guest path joins the workdir now (#1154), with the rule in `shared/spec`.

The end-to-end suite on this macOS host reports 13 passed and 4 failed on a fresh Worker,
which matches the runner's count. The four local failures are the same four as on the
runner. A container that starts under emulation can still exit during the start, so a run
on this host is not always the same.

A fourth runner run (job `140809210247`) passed 13 tests. Four failures remained, now
addressed. The `List` and the `index_put_delete_list` tests pass.

- `no_internet`: the runner answered `port error: Cannot perform I/O on behalf of a
  different Durable Object (I/O type: TcpPortState)`. A Cloudflare container is an
  input/output object of the Durable Object that made it. The Worker adapter kept one
  `worker::Container` for its whole life. It now holds the Durable Object state and asks
  it for the container at each call, so the container and the TCP port belong to the
  object that uses them. This change does not remove the error. The error comes from the
  local container runtime (issue #1221): in one `wrangler dev` process, the first
  container's port works, and every later container's port fails. This was shown with one
  test at a time on a fresh Worker: `no_internet` passes the first time and fails the
  second time; `two_sandboxes` starts both containers and then fails on the first
  sandbox's `run`.
- `stop`: the stop found a workload container after the destroy. The substrate handle
  marked the container as destroyed even when the destroy call failed, so `cleanup`
  skipped its only second chance. It sets the flag only after a successful destroy. The
  fake container fails one destroy, and `cleanup_retries_a_failed_destroy` fails on the
  old code. The stop test waits up to 10 s for the runtime to remove the container and
  prints the remaining names. On a clean run the stop test passes; in the full suite it
  finds the containers that the two panicking tests above left, so #1221 causes this
  failure too.
- `two_sandboxes`: the relative path `tmp/a` joins the workdir (`/workspace/tmp/a`), and a
  write does not make a missing parent. The test now runs `mkdir -p /workspace/tmp` first.
  The rule is recorded on #1154.
- `large_stream`: the command prints `x` then a newline. A stdout frame can end between
  the two bytes, so the old check `data.len() % 2 == 0` failed on a one-byte frame. The
  stream was correct; the check now follows the pattern across frame boundaries and
  checks the order. The test now fails at the `Exec` call because of #1221, not at its
  byte check.

The `sandbox-cloudflare` job stays red until #1221 is decided. The suite needs more than
one container in one `wrangler dev` process, and the local runtime refuses the port of
every container after the first. The production runtime is not affected.

A native boundary test also caught that `shared/control/service.rs` read `boot_timeout`
directly. `SandboxLimits::create_deadline()` gives the value, and the remote client reads
that accessor.

## Feature 00 tasks T5 and T6: Vault Transit and WebCrypto (2026-11-28)

**T5, Vault Transit in the sandbox job.** The test now reads `VAULT_ADDR` and `VAULT_TOKEN`.
When `VAULT_ADDR` is set, it uses the dev server of
`ci/local-services/docker-compose.yml`. When it is absent, the test starts the same pinned
image in a container. A Vault dev server mounts no secrets engine, so the test now enables
Transit before it creates the derived key. Before this change the key call returned 404 and
the test returned as a skip, so the backend had no live proof.

The `sandbox-e2e` job starts the Vault server with `--wait`, runs `vault-init` as a one-off
with `run --rm`, runs
`cargo test -p foundation_auth --features testing --profile uat kms_vault_transit`, and
stops the services with a trap on every path. The job fails when the output names a skip,
when the output holds no `1 passed` line, and when the output holds no
`vault_transit_seals_and_opens` case. The filter has no `kms::` prefix: the target root is
`tests/kms/mod.rs`, so the test is `kms_vault_transit::vault_transit_seals_and_opens`. A
`kms::` filter selects no case and looks green.

Local result, from a clean state (`down -v` first): `up -d --wait vault`,
`run -T --rm vault-init`, then
`cargo test -p foundation_auth --features testing --profile uat kms_vault_transit` gave
`test kms_vault_transit::vault_transit_seals_and_opens ... ok` and `1 passed; 0 failed`.

**T6, WebCrypto in the browser job.** `foundation_auth/tests/wasm/webcrypto_sealer_tests.rs`
is a `wasm-bindgen-test` target. It seals a payload, opens it back, refuses a different
record and proves that `crypto.subtle.exportKey` rejects the sealer's key. The `wasm-tests`
job runs it in headless Chromium with
`wasm-testbed test bindgen-web backends/foundation_auth --features secret,wasm-kek`.

`WebCryptoSealer::crypto_key()` is new public API, so `foundation_auth` moves to `0.0.17`
and every workspace consumer matches. The native test suites are marked native only, and
their file-system and database dev-dependencies sit behind
`cfg(not(target_arch = "wasm32"))`. Without that, the wasm test build compiled them and
failed in `socket2`.

Local result: headless Chromium ran the case through `wasm-testbed`:
`1 passed; 0 failed`.

## Feature 04 result (2026-11-28)

`foundation-ci live-kms` passed against the real services:

| Item | Value |
|---|---|
| AWS region | `us-east-1` |
| GCP project, location | `ewestudiosnigeria`, `global` |
| Tests | 4 passed, 0 failed |
| Teardown | done: both AWS keys are `PendingDeletion` (7 days) and both aliases are deleted |

The Google Cloud key ring and two key entries stay listed. Google Cloud cannot delete a key
ring, and it keeps a key entry until every version finishes destruction. The run destroys
every version.

- **#1113** (`severity: medium`, fixed): a run after a teardown found a key with every version
  scheduled for destruction, and the destroyed version was still the primary. The task now
  creates a version when none is enabled and makes the newest enabled version the primary.
- **#1114** (`severity: low`, open): the teardown should retry a transient precondition error
  right after the billing link.

## Today's findings

- **#1125** (`high`, fixed by #1144): `ControlService::create` spun on `Stream::Wait` in a
  synchronous loop on the single-threaded wasm host, so the Durable Object stopped answering.
- **#1133** (`high`, fixed by #1144): the slave did not recognise the Cloudflare container
  runtime, because it checked only `/.dockerenv` and `/run/.containerenv`.
- **#1140**, **#1141**, **#1142** (`high`, in #1144): the next Cloudflare failures: a
  streaming scoped call refused by the route check, a gzip unary body decoded as JSON, and a
  missing model configuration in the Durable Object.
- **#1107** (`high`, feature 05): the optional `foundation_auth` dependency makes a Cargo
  cycle that blocks any crate that needs both.
- **#1105**, **#1110**, **#1117**, **#1118**, **#1119**, **#1122**, **#1137**, **#1138**,
  **#1139**: medium and low items that wait their turn.

## Next

1. Merge the open pull requests in the order the continuous integration (CI) agent holds:
   #1144 → #1121 → #1111 → #1112 → #1130 → #1136 → #1123.
2. One full `packages=all` run on the final master; close #1104 with its `sandbox-e2e` link.
3. Feature 05 after #1121.
4. The Cloudflare demo under `examples/`, after #1144 merges.

## Feature 02 task T11: the Firecracker guest network (2026-11-28)

The guest has a network through a new helper program, `agent-fc-net`, and the shared
smoltcp data plane. The design changed from the issue text, because a TAP device has
one holder in the Linux kernel (see `learnings.md`).

- `agent-fc-net` makes a private user and network namespace, creates the persistent
  TAP device, opens an `AF_PACKET` socket on it, starts Firecracker in the namespace,
  and copies frames between the packet socket and descriptor 3. Descriptor 3 is one
  end of a Unix datagram pair that the control plane passed.
- The control plane runs the smoltcp stack in Ethernet mode over its end of the pair
  and relays the configured TCP and UDP ports to host services. The guest sees only
  the stack: there is no host route, no firewall rule and no NAT rule.
- `FirecrackerDriver::with_network` enables `NetworkMode::Host`. The default driver
  still refuses it, and the probe reports the capability only when it is configured.
- The slave applies `ewe.net=<address>/<prefix>,<gateway>` from the kernel command
  line at boot. `net.ifnames=0` keeps the guest name `eth0`.
- The namespace ends with its last holder. The helper has a parent-death signal from
  the control plane, and Firecracker has one from the helper. A stop signals the saved
  process handle; there is no `pkill`.
- Tests: the Ethernet path with two stacks over an in-memory wire runs on every
  operating system; the slave command-line parser has Linux tests; `e2e_host_network`
  runs only on the KVM runner, because it is the only host with `/dev/kvm`,
  `/dev/net/tun` and unprivileged user namespaces. #1095 stays open until that row is
  green on the runner.
