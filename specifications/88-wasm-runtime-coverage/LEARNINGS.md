# Learnings: Spec 88, Wasm Runtime Coverage

_Written 2026-11-15 from the survey and the measurements that shaped the plan. Written in
ASD-STE100 Simplified Technical English (STE)._

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

**Result.** Decision D2 is one narrowing of the owner's answer: `foundation_core` uses the
owned harness on Deno and the browser, and the wasm-bindgen harness on Deno and wrangler.
`foundation_http` and `foundation_connectrpc` use the wasm-bindgen harness on all three hosts.
The feature files repeat the reason so no agent tries the owned path again.

**What would change the answer.** The limit is the import, not the crate name. Port
`foundation_netio`'s wasm fetch client to the owned ABI, in the style of `foundation_core`'s
`js-foundation-wasm`, and no wasm-bindgen glue enters the graph. Then the owned harness can host
all three crates. The cost is a second HTTP transport implementation: request and response
streaming, headers, abort and the websocket path. It gives no host coverage, because the
wasm-bindgen harness already runs on all three hosts. This is recorded as #577, a low-severity
issue, not as work in this specification.

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
| the owned harness supplies only `abi` | a crate that imports anything else cannot use it (entry 1) |
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

---

_Created: 2026-11-15_
