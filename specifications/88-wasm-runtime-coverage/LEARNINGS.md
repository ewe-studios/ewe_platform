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
| The static server sent `.mjs` as `application/octet-stream`, and Chrome refused the module | the page stayed at "running…"; a probe page ran its module only with `text/javascript` | staged runner renamed to `runner.js`; #578 records the `foundation_http` fix |
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
