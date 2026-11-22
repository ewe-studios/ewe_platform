# Learnings: Spec 88, Wasm Runtime Coverage

_Written 2026-11-15, updated 2026-11-16, from the survey, the measurements and the owner's
answers. Written in ASD-STE100 Simplified Technical English (STE)._

Each entry names what we expected, what we measured, and what the plan does because of it. A
dead end stays here so the next reader does not walk it again.

## 1. The harness cannot host every crate

**Expected.** The owner's answer was "owned `#[wasm_test]` where a crate can run without
wasm-bindgen; the wasm-bindgen harness where it cannot". The plan first put `foundation_http`
and `foundation_connectrpc` on the owned harness, on the reasoning that both crates have a
wasm feature.

**Measured.** A probe crate that depends on `foundation_http` (`wasm`) and `foundation_netio`
(`wasm-fetch`) builds a wasm module with **seven** imports, all of them `__wbindgen_*` or
`__wbg___wbindgen_*` (requirements section 2.7). The owned runtime supplies only the `abi`
import module, so it cannot instantiate that module. The import comes through
`foundation_netio`, which every wasm HTTP or RPC path reaches.

**Result.** Decision D2 is the owner's answer: both harnesses run where a crate allows it
(owner, 2026-11-16). `foundation_core` uses the owned harness on Deno and the browser, and the
wasm-bindgen harness on Deno and wrangler. Features 02 and 03 add a crate feature that gates
`foundation_netio`'s `wasm-fetch`, build a second test package with the feature off, measure its
imports, and run the pure-Rust cases on the owned harness on Deno and Chromium. A transport case
stays on the wasm-bindgen harness on all three hosts. The feature files repeat the reason so no
agent tries the owned path with the fetch client still on.

**What does not change the answer.** The `abi` runtime compiles a JavaScript function body at
runtime and binds it to a handle (`Operations::MakeFunction`, `foundation-wasm.js:1202-1218`). A
crate calls `abi::web::register_function("function(){ … }")` and gets its own host function
(`foundation_core/integration/wasm_tests/src/lib.rs:28-34`). This adds no import. It supplies a
missing host primitive, such as a clock. It cannot supply the wasm-bindgen runtime intrinsics:
heap slots by index, memory-backed strings and throws, the `externref` table. Only the generated
glue provides those. A hand-written shim is a version-locked copy of the wasm-bindgen runtime and
gives no coverage.

**What would change the answer.** Port `foundation_netio`'s wasm fetch client to the owned ABI,
in the style of `foundation_core`'s `js-foundation-wasm`. Then no wasm-bindgen glue enters the
graph. The cost is a second HTTP transport implementation: request and response streaming,
headers, abort and the websocket path. It gives no host coverage, because the wasm-bindgen
harness already runs on all three hosts. This is recorded as #577, a low-severity issue, not as
work in this specification.

**Rule for the future.** Before you pick a harness for a crate, build its wasm module and
count the imports. One `__wbindgen_*` import decides it.

## 2. A runner that cannot fail is worse than a missing runner

**Expected.** `test bindgen-deno` and `test bindgen-web` reported `test result: ok` at the end
of the survey's runs. We took that as a pass.

**Measured.** A probe crate with one failing case prints
`test result: FAILED. 1 passed; 1 failed; …` and then `test result: ok`, and the process exits
0 (requirements section 2.6). The Deno template awaits `cx.run(...)` and ignores its boolean.
The web template writes `test result: ok` into the page unconditionally. Only the wrangler
worker reads the verdict.

**Result.** #572 is `severity: high`, and its fix is task T5 of feature 00, before any new
case is added. Every suite in this specification proves it can fail (R14) before it is called
done.

**Rule for the future.** A test runner's exit code is part of its contract. Prove it with a
failing fixture, not with a passing suite.

## 3. The browser host failed four ways, in four layers

The browser host looked like one broken feature. It was four separate defects, and each one
hid the next:

| Layer | Defect | Found by |
|---|---|---|
| the task | `mise run test:wasm-testbed:web` calls a subcommand that was renamed to `browser` | running the task (#566) |
| the preflight | `run_web` demands `node`, `npm` and `npx` for a pure-Rust run | reading the code (#567) |
| the install | the task installs Playwright's Chromium, which the driver never searches; the task needs `sudo` and reads `/etc/os-release` | reading the task and `resolve_binary` (#568) |
| the driver | `Target.createTarget` answers, but the first command with a `sessionId` (`Page.enable`, `Page.navigate`) gets no answer, so the run fails after 30 s | the smoke test and the owned run (#569) |

Two facts change the plan: the Linux CI image already installs Chrome, and a macOS runner
ships Chrome in `/Applications`, where `resolve_binary` never looks
(`foundation_browser/src/supervisor.rs:201-220`). So the Chromium cost is not paid per run; the
driver's search list is the fix, and a cache is the fallback for a runner with no browser.

**Rule for the future.** When one command fails, fix the first failing layer and re-run. Do not
fix all four at once; a later defect stays hidden behind an earlier one.

## 4. The wasm-bindgen twin already existed

**Expected.** The first draft added a new crate, `foundation_core/integration/wasm_valtron`, to
hold the wasm-bindgen twin of the owned suite.

**Measured.** `foundation_core/tests/` is a single test target (`mod`) that includes
`tests/valtron/`, and `tests/valtron/mod.rs` keeps about forty modules on wasm. It holds the
19 `dual!` cases of `async_first_api.rs` and the 15 `#[wasm_bindgen_test]` cases of
`wasm_js_yield_integration.rs`, and both suites share `tests/valtron/wasm_wake_support.rs`
through `#[path]`. No CI step builds or runs it, which is why nobody noticed (#575).

**Result.** No new crate. Feature 01 runs the existing target on three hosts and deepens the
shared scenario file, so each new scenario runs under both harnesses.

**Rule for the future.** Count the existing wasm-eligible modules in a crate's `tests/` tree
before you write a new integration crate.

## 5. The working precedent is `foundation_db`

`backends/foundation_db/integration/wasm_cf/` is the only wasm test crate that runs on the
real host today. Four of its choices are copied into features 02 and 03:

1. it sits **outside** the root workspace, so the parent's native dev-dependencies never build
   for wasm32 (R11);
2. it pins `wasm-bindgen = "=0.2.126"` and `wasm-bindgen-test = "=0.3.76"`, matching the CLI
   `mise.wasm.toml` installs;
3. it holds the case file in `tests/` and the helpers in `src/lib.rs`, so `--tests` has one
   target (R10);
4. it reads the real bindings from `globalThis.__CF_TEST_ENV__`, which the worker template sets.

**Dead end recorded.** A probe crate without the pins resolved `wasm-bindgen-test 0.3.130`
against the 0.2.126 CLI and failed with `wasm-bindgen failed (exit code 1)`. The pins are not
optional.

## 6. Two checks, not one

**Expected.** The `wasm-tests` package uses `cargo check --tests --target wasm32` to compile
test code the default gate never sees.

**Measured.** That form works for `foundation_core`, `foundation_netio` and
`foundation_wireguard`. It cannot work for `foundation_http`: the dev-dependency
`foundation_testing` pulls `socket2`, which has no wasm backend, so the check fails before it
reaches our code (#576). The library itself checks clean for wasm32.

**Result.** The gate has two forms: a lib check for `foundation_http` and
`foundation_connectrpc`, and a `--tests` check for each standalone integration crate. Both are
named in feature 00 T8.

## 7. `foundation_connectrpc` needs one feature set on wasm

**Measured.** The default feature `rpc_multi` enables `foundation_core/multi`, which is a hard
`compile_error!` on wasm. `--no-default-features --features rpc` checks clean for wasm32.
`rpc` pulls `arrow`, `auth`, `zstd` and `brotli`, and all four build for wasm32.

**Consequence.** Every wasm crate and every wasm CI step that names `foundation_connectrpc`
uses that feature set. Feature 03 states it twice, because a sub-agent that runs `cargo check`
with the default fails with a `compile_error!` it cannot interpret.

## 8. What the hosts cannot share

| Fact | Consequence |
|---|---|
| workerd has no DOM and no page | browser-only paths are not tested there |
| a browser cannot serve; a worker cannot open a socket | the suite reaches the testbed's Rust server over the loopback |
| a workerd fetch to the loopback may be refused | feature 03 measures it first and records the answer |
| the owned harness supplies only `abi` | a glue import blocks it; `abi` can still compile a new host function (entry 1) |
| `foundation_core/multi` is a hard error on wasm | a bundled dep asking for it breaks every wasm build; target-gate the dep |

## 9. Writing rules that kept the plan honest

- **A case names its oracle.** "It works" is not an oracle. Each case says example, bound,
  model or fault, and the failure message names the host, the case and the first unexpected
  value (R2, R3).
- **A suite proves it can fail** (R14). The plan budgeted one deliberate break per layer, with
  the red output recorded per host, before any suite is called done.
- **Never add a stub to make a host green.** The whole point of the specification is to remove
  false greens; a stub in the suite would recreate one.
- **A missing tool is not a failing case.** It is a runner defect, and the fix belongs in
  feature 00.

## 10. The Playwright install outlived its runner

**Expected.** Spec 43 replaced the Node browser runner with the pure-Rust CDP client in
`foundation_browser`. The browser task looked repaired.

**Measured.** The install stayed in two places. `mise.toml:145-153` defines
`setup:playwright`, and the browser task still depends on it (`mise.toml:1083`). That task
runs `npx playwright install --with-deps chromium`. It needs `sudo`, and it puts Chromium in
the Playwright cache, where `resolve_binary` never looks. The file
`backends/foundation_testbed/integration/package.json` holds the Playwright development
dependency and a `setup:web` script, and nothing reads it. No test or CI step checks that a
removed tool is gone.

**Result.** The owner removed Playwright on 2026-11-16. Feature 00 T1 deletes the task and
the package file, and its done-when searches the name. `foundation_browser` supplies the
CDP client.

**Rule for the future.** When a runner changes its transport, delete the old transport's
install in the same change. A task that installs a tool no code calls is a false dependency.

---

_Created: 2026-11-15_

## 11. The page session stalled behind the read deadline (#569)

**Expected.** The first command that carries a `sessionId` (`Page.enable`) never answered. Both
observed failures were exactly 30 s, so the plan guessed that a socket read deadline equal to
the call deadline ended the reader.

**Measured.** With `PRIMAL_CDP_TRACE=1` on Chrome for Testing 157.0.8103.51, macOS arm64,
`wasm-testbed browser integration/module --filter passes --headless` printed these frames:

```
17:29:08.224  send id=1 Target.createTarget
17:29:08.493  recv id=1 result
17:29:08.493  send id=2 Target.attachToTarget
17:29:38.241  recv event Target.attachedToTarget
17:29:38.241  recv id=2 result
17:29:38.241  send id=3 Page.enable session=5DE5…
17:30:08.245  reader thread exiting reason=stopped
             error: Page.enable timed out after 30000ms
```

The answer to `Target.attachToTarget` arrived 29.7 s after the command was sent. The reader
then delivered the next command at once and timed out again. The reader did not exit before
the call deadline: the engine was dropped after the timeout, and the reason was `stopped`.

**Root cause.** The `foundation_netio` WebSocket task drains its outbound pipe between socket
reads, and one blocking read used the engine's read deadline (30 s). A command queued while
the reader was blocked in that read was not written until the read timed out. So a CDP call
could only start after the previous read returned, and the first read after a quiet moment
cost up to 30 s. The protocol and the frame parser were correct.

**Fix.** The engine sets a short read deadline (default 50 ms), so it writes a queued command
within one read. Every call now answers in 20-50 ms in the same trace. The reader contract
holds at the same time: a close, a read error or the end of the stream marks the connection
dead, fails every waiting call with `ConnectionLost`, and makes a later call fail at once
(invariant I6). `RpcEngineConfig` holds `call_timeout`, `read_timeout` and `idle_sleep`;
`LaunchConfig` holds the engine config and `navigate_timeout`.

**Two more blockers appeared after the CDP fix.** Each one is a GitHub issue.

| Defect | Evidence | Result |
|---|---|---|
| The static server sent `.mjs` as `application/octet-stream`, and Chrome refused the module | the page stayed at "running…"; a probe page ran its module only with `text/javascript` | feature 00 staged `runner.js`; #578 was closed in the review: `StaticFileHandler` now answers `.mjs` with `text/javascript`, and `static_file_serves_mjs_as_a_module_script` proves it |
| The runner read the first `#output` line as the verdict | "owned harness run failed with exit code 1" after `test result: ok` | last `test result:` line is the verdict; #579 |
| Synchronous WebAssembly compile threw above 8 MiB | the core module failed with `RangeError: WebAssembly.Compile is disallowed on the main thread` | async compile and instantiate; #580 |
| The install copied the binary without `icudtl.dat` | the binary exited before it opened the debug port; #581 | the task copies the whole archive |

**Rule for the future.** A blocking transport that multiplexes reads and writes on one thread
must wake often. A read deadline is a poll interval, not the time a caller should wait. When a
diagnosis shows a delay equal to a timeout, measure the send and receive times of every frame
before you change the contract.

## 12. The endpoint on each host (T6)

**Answer to the open question: workerd permits a subrequest to the dev server's own origin.**
No service binding is needed. Measured on 2026-11-16 with `wrangler dev --local`: the fixture's
`echo_endpoint_answers` case fetched `http://127.0.0.1:<port>/echo` from inside the worker and
got 200 with its method, header and body. The first attempt failed with "Network connection
lost", but the cause was not a refusal: workerd gave the worker a request URL without the port
(`http://127.0.0.1`), so the fetch went to port 80. The runner now sends the origin in an
`x-fwt-base-url` header and the worker reads it.

| Fact | Evidence | Consequence |
|---|---|---|
| the request body arrives as a stream, not a sized value | the handler answered 400 "not chunked" with a `Content-Length` request | `EchoHandler` collects the body with `DataBytesIterator`, and stops at the limit |
| a page and the endpoint must share one origin | the page loaded from `localhost`, the base URL used `127.0.0.1`, so Chrome blocked the fetch | `TestServer::url` now uses the same origin as `base_url` |
| the embedded Deno runtime has no executor pool of its own | the server rejected every connection with "valtron pool not initialized" | the runner calls `foundation_browser::runtime::ensure_runtime` first |
| the embedded Deno fetch speaks correct HTTP/1.1 | a raw listener received `POST /echo HTTP/1.1` with the headers and the body | the endpoint works on both owned hosts |

**Cases that prove it.** The owned sample crate has the async case `echo_endpoint_answers`
(registered host function + one yield per poll). The bindgen fixture has the case of the same
name, which uses `web_sys` fetch so it runs on Deno, the page and workerd. The filter `--filter
echo` runs each alone. The route itself has two tests in `tests/echo_server_tests.rs`.

## 13. The artifact choice and the wrangler filter (T7)

**Measured.** A scratch crate with two test files (`tests/one.rs`, `tests/two.rs`) built two
wasm modules that both carry `__wbgt_` exports. The old code took the first one cargo reported
(or the first in `deps/`), so one test target ran and the other vanished with no message.

**Fix.** `build::run_impl` collects every candidate that answers the request and stops at the
first one only when there is exactly one. Two or more fail with
`AmbiguousTestArtifact { candidates, hint }`; the message lists both paths and says to keep one
test target (rule R10). Verified:

```
Error: more than one wasm test artifact carries `__wbgt_` exports:
["…/out/two-1e9256…wasm", "…/out/one-34af42…wasm"]
Keep one test target per crate (rule R10): put every case in one tests/*.rs file, or keep a single [[test]] target.
```

**Also in T7.** `test bindgen-wrangler --filter <name>` keeps only the matching cases before it
writes `worker.js`. On the fixture, `--filter a_passes` reported "Discovered 1 tests" and `test
result: ok. 1 passed`. The two stale runner names are corrected (#574).

## 14. The CI shape that runs all five executors (T8)

**Five executors, cheapest failure first.** The `wasm-tests` package runs the owned Deno host,
the bindgen Deno host, the 19 `dual!` cases, the worker, the owned browser host, the bindgen
web host, then the failing fixture. The fixture step passes only when both bindgen runners
reject `b_fails`.

**Measured results of each new task.**

| Task | Result |
|---|---|
| `test:wasm-bindgen:core` | Discovered 19 tests; `test result: ok. 19 passed` |
| `test:wasm-bindgen:web` | Discovered 1 test; `test result: ok. 1 passed` |
| `test:wasm-testbed:bindgen-fails` | both runners exit non-zero; the step passes |

**What did not happen as planned.** `test:wasm-bindgen:web` was meant to run the valtron
`dual!` cases. That module traps while it starts in Chromium (`RuntimeError: unreachable`), so
the page prints no result. Feature 01 owns that runtime work; #586 records it. The web step
runs the fixture's passing case until then, so the executor still proves it runs and reports
the truth. The web runner also gained `--filter` here (#582), because without it the step could
not name one case.

**Chromium cost.** The Linux image links Google Chrome on `PATH`, so the install task now checks
`PATH` before it downloads. The workflow caches `~/.cache/foundation/chromium` with a key that
names the pinned version. The macOS job needs no browser install: Chrome is in `/Applications`,
and the search order of invariant I4 reads that path. The macOS job runs two direct `cargo run`
commands instead of the mise task, because the macOS runner has no mise.

**Rule for the future.** A CI step that runs a mise task inherits a tool the runner may not
have. Prefer the direct command on a platform whose tool set the image does not control.

## 15. The Chromium start trap was libtest's `main` (#586)

**Expected.** The web host traps inside wasm-bindgen's start function, before a case runs. The
plan looked for a glue version mismatch, a missing import, or a valtron start path.

**Measured.** With `PRIMAL_CDP_TRACE=1` on Chrome for Testing 157.0.8103.51, macOS arm64, the
page's `#output` carried this stack (shortened):

```
at mod-<hash>.wasm.__rustc[<hash>]::__rust_abort (…wasm-function[29762])
at mod-<hash>.wasm.__rustc[<hash>]::rust_panic (…wasm-function[29244])
at mod-<hash>.wasm.std[<hash>]::panicking::panic_with_hook (…wasm-function[4416])
at mod-<hash>.wasm.core[<hash>]::panicking::panic_fmt (…wasm-function[16865])
at mod-<hash>.wasm.core[<hash>]::result::unwrap_failed (…wasm-function[11766])
at mod-<hash>.wasm.std[<hash>]::thread::functions::spawn::<mod[<hash>]::io::buffer_pool_tests::test_concurrent_access::{closure#0}, ()> (wasm-function[3579]:0x2ace2b)
```

The trap was not a start-table walk. A plain `#[test]` case ran, and it called
`std::thread::spawn`.

**Root cause.** The wasm-bindgen CLI writes `wasm.__wbindgen_start();` into the glue at every
target. In a test binary, that start function does two jobs. It calls the
`__wbindgen_init_externref_table` import. Then it calls the binary's `main`, which is libtest's
test harness. The harness runs every plain `#[test]` case. The crate's `tests/mod.rs` compiles
`io` and `synca` for wasm, and `io::buffer_pool_tests::test_concurrent_access` spawns ten
threads. Threads are not supported on `wasm32-unknown-unknown`, so `std::thread::spawn` panics.
The panic aborts the module, and the browser reports `RuntimeError: unreachable`. The page
never reaches `new WasmBindgenTestContext()`, so no `test result:` line appears.

This was measured on the background wasm module: the exported `__wbindgen_start` calls import
57 `__wbindgen_init_externref_table`, then an unnamed wrapper, then the `main` export. The
module has 32291 code bodies, and the plain test cases are in the same module.

The Deno path passed because `run_bindgen_deno` already removed the call with
`content.replace("wasm.__wbindgen_start();", "")`. The web path in `run_bindgen_web` and in
`bindgen_runner::run_bindgen` did not. The fixture case passed on the web host because the
fixture crate has only `#[wasm_bindgen_test]` cases, so its libtest `main` runs nothing.

**Fix.** `wasm::patch_bindgen_glue` in `foundation_testbed/src/wasm/wasm.rs` replaces the call
in every glue target with the externref table setup only, because a `JsValue` constant maps to a
wrong table slot without it. It appends `export { wasm };` for the deno and wrangler runners.
It returns an error when the glue has no such call, so a changed CLI fails loudly instead of
leaving the harness `main` in place. The web, deno and wrangler runners and
`bindgen_runner::run_bindgen` call it. `tests/bindgen_glue_tests.rs` has three cases.

**Measured after the fix.**

| Check | Result |
|---|---|
| `test bindgen-web backends/foundation_core --features js-wasmbindgen,std --headless` | Discovered 34 tests; `test result: ok. 34 passed` |
| `test bindgen-deno backends/foundation_core --features js-wasmbindgen,std` | 34 passed |
| `bindgen backends/foundation_core --features js-wasmbindgen,std --headless` | 34 passed |
| `MISE_ENV=wasm mise run test:wasm-bindgen:core` | 19 passed |
| `MISE_ENV=wasm mise run test:wasm-bindgen:web` | 34 passed |
| `MISE_ENV=wasm mise run test:wasm-testbed:bindgen-fails` | both runners reject `b_fails` |

**The case count.** Entry 14 names 19 cases, because the `test:wasm-bindgen:core` task filters
to `async_first_api`. Without a filter, foundation_core has 34 wasm-bindgen cases: 19 in
`async_first_api` and 15 in `wasm_js_yield_integration`. Both hosts run all 34.

**Rule for the future.** A test binary built with `--tests` holds two test systems in one
module: libtest's `main` and the `__wbgt_` exports. A JavaScript host must call only the
exports. Remove the start call, and keep the externref table setup that shares its body.

## 16. A trap in a host callback lost the case name (#587)

**Expected.** A failing case reports its verdict and its message, and the run continues to the
next case. The panic hook runs before the abort trap, so a panic reaches the runner.

**Measured.** The owned Deno run ended with `RuntimeError: unreachable`. No `test result` line
followed, and no case name appeared. The trap happened in a host callback: a `setTimeout`
callback re-entered the wasm module and an assertion panicked there.

**Root cause, part one (the missing name).** The runner printed case lines into its `out`
array, but it sent the structured report to Rust only after the last case. A trap in the middle
of the run lost every line. The line buffer was also invisible to the driver: no JavaScript
frame held it, and the event loop returned an error to Rust.

**Root cause, part two (the missing message).** The panic hook ships the message through the
`host_report` import into `TestReports`. The runner awaits `testReports.next()` while a case is
pending, so `push` resolved a pending promise and did not buffer the report. The awaiting
continuation never ran after the trap, so the message sat in a dead promise.

**Fix.** The runner sends a report after each case. It exposes `__FWT_DIAGNOSTICS__`: the case
in flight, `TestReports.snapshot()` (buffered reports plus the last delivered one), and the
line buffer. When the event loop ends with an error, `run_staged_harness` reads that object,
appends the error and the diagnostics to the output, and counts the run as one failure.
`BrowserRunLimits` timeout diagnostic reads the same object.

**Measured after the fix.** In the `linux/amd64` CI image on an `arm64` host, the run names the
case and the message:

```
failing case: any_readiness_resumes_when_a_child_is_ready
case report: any_readiness_resumes_when_a_child_is_ready: panicked at
  src/../../../tests/valtron/wasm_wake_support.rs:684:13:
resumed 64ms after the child was ready (window 25ms)
```

**What the message shows.** The wake path passed its first check: `polls_at_fire` was 1, so the
executor did not poll the task while it was parked. The run failed on the 25 ms resume window
alone. The host was emulated. This is the family of #589, not a lost wake. The checks after the
window did not run.

**Rule for the future.** A harness that runs many cases in one event loop must report each case
as it ends. An awaiting promise is not a store: anything a trap can cut off needs a copy that a
later reader can reach.

## 17. A build script needs its tool on every operating system (#599)

**Symptom.** A full CI run ended with `failure` while every package job passed. The separate
`macOS build` job panicked at `foundation_agent_rpc/build.rs:31` with "failed to run protoc
(protoc): No such file or directory".

**Root cause.** `protoc` was pinned in only one place: the Linux CI image, through
`ci/platform-images/mise/mise.toml` and `mise.lock`. The hosted macOS and Windows runners have
no `protoc`. Every package job ran in that image, so the gap stayed hidden until the job that
builds on the bare runner. The macOS and Windows test jobs also need `protoc` when
`foundation_agent_rpc` is in their matrix (`runs = ["", "gguf"]`).

**Fix.** The `macos-test`, `windows-test` and `macOS build` jobs run a new
"Install the pinned protoc" step. It picks the release asset from `uname -s` and `uname -m`,
checks its SHA-256 against the value in `mise.lock`, unpacks it and adds `bin` to
`$GITHUB_PATH`. The Linux image keeps its own install.

**Two traps in the step.** A zip file does not record the executable bit, so unpacking with
`zipfile.extractall` left `protoc` not executable. The step runs `chmod +x` after it. On
Windows, `$RUNNER_TEMP` holds a native path, so the step uses `cygpath -m` to give bash and
the native Python one path they both read.

**Measured.** The step ran on this macOS `arm64` host. It verified the checksum, printed
`libprotoc 36.2` and wrote the `bin` directory to `GITHUB_PATH`. `actionlint` accepts the
workflow. Run 43754950849 then passed the `macOS build` job on a macOS `arm64` runner.

**Rule for the future.** A tool that a build script runs is part of the build. Pin it for each
operating system that builds the crate, in the same change that adds the build script.

## 18. The `mod` target is the whole wasm-bindgen surface (feature 01)

**Expected.** The target held about forty wasm-eligible modules, and no step ran them (#575).

**Measured.** With feature 00's verdict fix, the target discovers 35 cases before this feature:
19 in `async_first_api`, 15 in `wasm_js_yield_integration` and 1 in `js_stream_park`. This
feature adds six shared scenarios to `wasm_js_yield_integration` and the two `#[task]` cases to
`state_task_wasm`, so the target discovers 43. All 43 pass on Deno, headless Chromium and
workerd through `wrangler dev --local`. The owned suite grew from 33 to 47 cases.

**Result.** `test:wasm-bindgen:core` drops the `async_first_api` filter and fails when the
count falls below 34. A later change that drops a module then shows as a count drop, not as a
silent gap. The Chromium form (`test:wasm-bindgen:web`) and the workerd form run the same 43.

**Rule for the future.** Keep the count floor. A test target that no step runs holds every
compile error in that target; run it before you add cases to it.

## 19. The R14 break: `WakeRegistry::fire` (feature 01 T6)

**The break.** The feature names `WakeRegistry::wake_all`; the code name is
`WakeRegistry::fire` (`backends/foundation_core/src/valtron/wake.rs:157`). The break took the
registered callback list and called none of the callbacks.

**The case.** `two_waiters_both_resume` registers two callbacks in one `WakeRegistry`. Each
callback sets a `ResumeSignal` on which one task parks. Without the calls, neither task resumes.

**Red output** (one case per host, `--filter two_waiters_both_resume`):

| Host | Output |
|---|---|
| wasm-bindgen Deno | `two_waiters_both_resume: the parked task never ran again (a lost wake)`; `test result: FAILED. 0 passed; 1 failed`; exit 1 |
| wasm-bindgen Chromium | the same line; `test result: FAILED. 0 passed; 1 failed`; exit 1 |
| wasm-bindgen workerd | `{"passed":false}`, the same line; exit 1 |
| owned Deno | `failing case: two_waiters_both_resume`; the same line; exit 1 |
| owned Chromium | `FAILED  two_waiters_both_resume`; the same line; exit 1 |

**Reverted.** Yes. The same filtered case is green on all five hosts after the revert, and the
full suites are green.

**Rule for the future.** Prove a fire-all registry with a case whose wake must reach a task. A
registry case that only counts callbacks passes even when the wake path is broken.

## 20. The owned wasm executor pays one host timer per task step (feature 01 T3)

**Expected.** The feature set a bound of 3 s for `sixty_four_tasks_share_the_loop` (64 tasks of
four yields each, 256 task steps).

**Measured.** On the owned Deno host (macOS `arm64`, 2026-11-22) the case took **4.004 s**,
about 15 ms per step. The executor hands each step to the JS event loop
(`local.rs:3389`, `yield_for(planned)`), and a host timer costs about 10 ms
(`constants.rs`). The 500-poll `WaitUntil` default also failed first: one harness re-entry does
not advance every task, so the case needs its own poll bound.

**Result.** The case keeps its own stated bounds: a 2000-re-entry poll bound (the anti-spin
guard) and a 10 s wall-clock bound. Every task still ends and none starves. Issue #740 records
the cost and the likely fix (expose the yield quantum, or emit several `Ready` values per host
turn).

**Rule for the future.** A wall-clock bound over many JS-hosted task steps must include the
host timer cost: multiply the step count by the yield quantum before you pick the bound.

## 21. The wasm writer path routed on the full URL (feature 02 T4)

**Expected.** `serve_writer::handle_request` routed a request to its handler, as the web and
Cloudflare bridges do.

**Measured.** Every writer route answered 404. The code passed `req.request_url.url` to the
router, and that value is the whole URL (`http://example.test/writer`), not the path. No test
exercised a writer route before (#570), so the defect was invisible.

**Result.** The path now comes from `request_uri.path()`. Case 16 (`serve_writer_round_trip`)
proves it. Issue #747 records the defect.

**Rule for the future.** Route on `request_uri.path()`. A full URL has a scheme and an
authority, and a route tree matches path segments.

## 22. A route miss was a rejected promise, and a method miss was a 404 (feature 02 T2)

**Expected.** `WasmHttpApp::handle_request` answers 404 for an unknown path and 405 for a known
path with a wrong method (stories S3 and S4).

**Measured.** `dispatch_web` returned `Err(ServeError::BadRequest { status: 404 })`, and the
bridge maps every error to `JsError`. The caller got a rejected promise, not a `Response`. The
router also returned `None` for both a path miss and a method miss, so it could not tell them
apart.

**Result.** `RouteOp::MethodNotAllowed` and `Router::dispatch_result` keep the miss kind. The
web and Cloudflare dispatchers build a 404 or 405 `Response`. Issues #744 and #745 record the
defects. The break of the 405 answer to 404 is the R14 record of this feature.

**Rule for the future.** A route miss is an HTTP response, not a transport error. Keep the miss
kind until the response is built.

## 23. Measure wasm imports without `wasm2wat` (feature 02 T6)

**Expected.** The feature's T6 measurement uses `wasm2wat <artifact> | rg '\(import '`.

**Measured.** The host has no `wasm2wat`, `wasm-tools` or `wasm-objdump`. A `strings` search is
not a substitute: it also matches internal function names.

**Result.** A 40-line Python reader parses the wasm import section and prints
`module.field (kind N)`. The owned module printed only `abi.host_report`,
`abi.host_fill_random`, `abi.host_wall_clock_ms` and `abi.host_monotonic_now_ms`, with no
`__wbindgen_` line. The reader ran with `python3 -I` from a temporary directory.

**Rule for the future.** A wasm import proof reads the import section. `strings` does not prove
an absence.

## 24. Owned cases live in the crate lib, not `tests/` (feature 02 T6)

**Expected.** The feature's section 6.8 step 5 says to write the owned cases in `tests/owned.rs`.

**Measured.** The owned runner (`wasm-testbed deno` / `browser`) builds the package lib and
reads its `__fwt_` exports (spec 88 section 5, rule R9). A case in `tests/` compiles into a
libtest binary and never runs.

**Result.** The 40 cases sit in `src/lib.rs`. The feature file records the correction. The
precedent `foundation_core/integration/wasm_tests` already does this.

**Rule for the future.** An owned case lives where the `cdylib` carries it: the crate lib.

## 25. A workerd body read can starve the worker (feature 02 case 2)

**Expected.** `collect_string_async` reads a streamed request body on all three hosts.

**Measured.** It passed on Deno and Chromium. On workerd the worker never answered the test
request, and the run failed after the 300 s request timeout with
`read status: Resource temporarily unavailable (os error 35)`. `AsyncSendSafeBody` with no
read-wake source calls `cx.waker().wake_by_ref()` on an empty turn. On workerd that did not let
the bridge's reader task run.

**Result.** The case reads the stream itself and yields one host turn (`setTimeout(0)`) per
`Data::Retry`, with a 1,000-turn bound. It passes in 0.48 s. Issue #749 records the netio
defect and the likely fix (a `ReadWakeSource` from `WasmByteStream`).

**Rule for the future.** A wasm future that waits on host I/O must yield to the host, not
self-wake. A self-wake is not a host turn.

## 26. Chromium hides `Set-Cookie` from a constructed `Response` (feature 02 case 8)

**Expected.** A case can read two `Set-Cookie` values from a `Response` and check their order.

**Measured.** Deno returned them through `Headers.getSetCookie`. Chromium returned an empty
list from both `getSetCookie` and `get`, although the response carried the headers.

**Result.** Case 8 uses a normal duplicate header (`x-order`). The Fetch API joins its values in
append order, which is the property under test. The case comment names the reason.

**Rule for the future.** Test header order with a normal header. `Set-Cookie` is special in the
Fetch API, and its order is not portable across hosts.

## 27. A wasm panic aborts the module (feature 02 case 9)

**Expected.** A panicking handler becomes a 500 and the host stays up.

**Measured.** The panic left `serve_web` and aborted the module (`RuntimeError: unreachable`).
wasm-bindgen-test caught it at the test boundary and reported `handler_panic_becomes_a_500 ...
FAIL`. A `wasm32-unknown-unknown` build uses `panic = "abort"`, so `catch_unwind` cannot catch
it.

**Result.** The case is gated by the crate feature `panicking-handler-case`. Issue #742 records
the defect and the two possible fixes (a guarded host call, or unwinding when the target
supports it).

**Rule for the future.** Do not promise a 500 from a panic in a `panic = "abort"` module. The
module stops.

## 28. The wasm-bindgen crate must not use `host-abi` (feature 03)

**Expected.** The feature's T1 `Cargo.toml` block gives `foundation_context` and
`foundation_compact` the `host-abi` feature.

**Measured.** With `host-abi`, the wasm-bindgen module imports `abi`. The Deno host does not
supply that module, and the run stops at import time:
`error: Import "abi" not a dependency`. `host-abi` wins over `wasm-bindgen` when both are on,
so the explicit feature overrode `foundation_core`'s `js-wasmbindgen` choice.

**Result.** The wasm-bindgen crate uses no explicit clock feature. `foundation_core`'s
`js-wasmbindgen` selects the wasm-bindgen clock and entropy backend. The owned crate keeps
`host-abi`, because its module imports only `abi` (rule R15).

**Rule for the future.** A wasm-bindgen module takes the wasm-bindgen backend. `host-abi` is
for the owned harness only.

## 29. A browser needs `duplex: "half"`, and it still cannot stream an upload to HTTP/1.1 (feature 03 T5)

**Measured on Chromium 157.** Two facts, in order:

1. `new Request(url, { body: ReadableStream })` throws
   `TypeError: The duplex member must be specified for a request with a streaming body`.
   The wasm fetch client set no `duplex` (#754). Fixed in `build_web_request`.
2. With `duplex: "half"`, the fetch promise rejects with `TypeError: Failed to fetch` when
   the origin is HTTP/1.1. A minimal page fetch with a one-chunk `ReadableStream` body fails
   the same way. The Rust client is not the cause (#753).

**Result.** `H1Transport` always sends a streaming body, so it cannot reach the HTTP/1.1
testbed from a page. Feature 03 gates transport cases 14 to 16 on #752. Case 17 still runs.

**What would change the answer.** Send a fixed-length body when the producer has already
closed the body, or serve the route over HTTP/2. Do not fake the endpoint.

**Rule for the future.** Upload streaming is not portable. A transport that always streams a
request body does not work from every browser to an HTTP/1.1 origin.

## 30. The `rpc` feature set is not glue-free (feature 03 T7)

**Expected.** The wasm-safe set `default-features = false, features = ["rpc"]` builds a module
with no wasm-bindgen glue, so the owned harness can load it.

**Measured.** `rpc` includes `auth`. The wasm target dependency of `foundation_auth` enables its
`wasm` feature, which enables `foundation_netio/wasm-fetch`, which brings wasm-bindgen glue.
The tree shows `foundation_netio feature "wasm-fetch" <- foundation_auth feature "wasm" <-
foundation_connectrpc feature "auth" <- "rpc"`.

**Result.** The owned package builds with no features and no `wasm-fetch`. Its module imports
`abi.schedule_timeout`, `abi.host_report` and `abi.host_monotonic_now_ms` only. Issue #756
records the feature graph.

**Rule for the future.** Before you promise a glue-free module, measure its imports. A feature
set that "looks wasm-safe" can still pull the fetch client through a sibling crate.

## 31. `valtron::execute` hangs in a wasm-bindgen test body (feature 03 T5)

**Expected.** Drive the `H1Transport` pump, which `valtron::send` schedules, with
`valtron::execute(from_future(call)).into_ready_future()`.

**Measured.** The call never finishes and the browser run stops after 30 s. The netio suite
records the same limit: `Driving valtron::execute from a test body hangs under the Deno runner`
(`tests/wasm/websocket.rs`, #565).

**Result.** The test driver polls the client future, steps the single-threaded engine with
`foundation_core::valtron::single::run_once()` while it can progress, then yields to the host
with `setTimeout(0)`. A no-op waker is enough because the loop polls every turn. A 4,000-turn
bound fails the case instead of hanging it (rule R4).

**Rule for the future.** Do not call `valtron::execute` from a wasm-bindgen test body. Poll and
step the engine, with a host yield and a stated turn bound.

## 32. The `/connect` route needs the prefix, and the body must be read first (feature 03 T4)

**Measured.** Two things the first route version missed:

- The router is keyed by the full path. A client calls `{base}/connect/case.CaseService/Unary`,
  so the procedures are registered as `/connect/case.CaseService/Unary`. A registration without
  `/connect` answers 404.
- The router's `dispatch` reads the body with `in_memory_body`, which cannot wait on a
  connection. A browser fetch with a streamed body sends a chunked body, and the read failed.
  The route now reads the body first, bounded by `ServerLimits::max_connect_body`, and hands
  `SendSafeBody::Bytes` to dispatch.
- A route miss has no response header, and the HTTP/1.1 renderer rejects an empty header map
  (`HeadersRequired`). The route adds a `connection` header when the map is empty.

**Rule for the future.** A prefix route registers its full paths, reads its own body when the
router cannot, and gives the renderer at least one header.

## 33. A Connect stream's EndStream frame is its own sink chunk (feature 03 case 5)

**Expected.** Case 5 says the sink receives open, three chunks, then finish.

**Measured.** The sink receives open, four chunks, then finish. The fourth chunk is the
Connect `0x02` EndStream frame; the protocol writer sends it as its own envelope.

**Result.** The case asserts three message frames in order, then the end-stream frame, then
finish. Nothing is weakened: the order and the frame contents are checked.

**Rule for the future.** Count the protocol's terminal frame when you assert a stream's chunk
list.
