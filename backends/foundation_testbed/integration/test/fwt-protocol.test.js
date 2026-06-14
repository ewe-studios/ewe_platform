// Feature 13 e2e: the owned #[wasm_test] execution model against a REAL compiled
// module — __fwt_ exports run on foundation-wasm.js, outcomes flow back over the
// host_report import (TestReports), panics reach the runner BEFORE the abort trap
// (panic hook), async cases resolve through the owned schedule_timeout re-poll
// loop, and should_panic verdicts are inverted runner-side. No wasm-bindgen, no
// wasm-pack anywhere in this path.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { FoundationWasm, TestReports } from "../../../foundation_wasm/runtime/foundation-wasm.js";

const here = dirname(fileURLToPath(import.meta.url));
const wasmPath = join(here, "..", "fixtures", "fwt_sample.wasm");
const skip = existsSync(wasmPath) ? false : "wasm fixture not built (run ./build-module.sh)";

// A fresh instance per case: a trapped (panicked) instance is dead — the runner
// re-instantiates, exactly like the real CLI runner will.
function boot() {
  const rt = new FoundationWasm();
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(wasmPath)), {
    abi: rt.web_abi,
  });
  rt.init(instance);
  return { rt, instance };
}

/** Run one case end-to-end → { report, trapped }. */
async function runCase(exportName) {
  const { rt, instance } = boot();
  let trapped = false;
  let pendingAsync = false;
  try {
    pendingAsync = Number(instance.exports[exportName]()) === 1;
  } catch (error) {
    trapped = true; // wasm32 panic = abort trap; the report already arrived via the hook
    assert.ok(error instanceof WebAssembly.RuntimeError, `expected a trap, got ${error}`);
  }
  const report = pendingAsync || rt.testReports.pending > 0 || trapped
    ? await rt.testReports.next()
    : assert.fail(`${exportName} produced no report`);
  return { report, trapped };
}

test("sync pass reports STATUS_PASS with the case name", { skip }, async () => {
  const { report, trapped } = await runCase("__fwt_passes_simple");
  assert.equal(trapped, false);
  assert.equal(report.status, TestReports.PASS);
  assert.equal(report.message, "passes_simple");
});

test("assertion failure reports STATUS_FAIL with the panic text, then traps", { skip }, async () => {
  const { report, trapped } = await runCase("__fwt_fails_with_assertion");
  assert.equal(trapped, true, "a wasm32 panic aborts after the hook reports");
  assert.equal(report.status, TestReports.FAIL);
  assert.match(report.message, /fails_with_assertion: /);
  assert.match(report.message, /math is broken on purpose/);
});

test("ignored case reports STATUS_IGNORED without running the body", { skip }, async () => {
  const { report, trapped } = await runCase("__fwt_ignored_case");
  assert.equal(trapped, false);
  assert.equal(report.status, TestReports.IGNORED);
  assert.equal(report.message, "ignored_case");
});

test("should_panic case traps with a FAIL report the runner inverts to PASS", { skip }, async () => {
  const { report, trapped } = await runCase("__fwt_panics_as_expected");
  // Module-side this is a failure (it panicked); the manifest `p` flag tells the
  // runner to invert — so the assertable contract here is fail + trap.
  assert.equal(trapped, true);
  assert.equal(report.status, TestReports.FAIL);
  assert.match(report.message, /this panic is the expected outcome/);
  const verdictAfterInversion = report.status === TestReports.FAIL && trapped ? "pass" : "fail";
  assert.equal(verdictAfterInversion, "pass");
});

test("async case yields through schedule_timeout and reports on resolution", { skip }, async () => {
  const { rt, instance } = boot();
  const returned = Number(instance.exports.__fwt_async_completes_after_yield());
  assert.equal(returned, 1, "async case signals a pending report");
  assert.equal(rt.testReports.pending, 0, "report has not arrived synchronously");
  const report = await rt.testReports.next(); // resolves after the timer re-poll
  assert.equal(report.status, TestReports.PASS);
  assert.equal(report.message, "async_completes_after_yield");
});
