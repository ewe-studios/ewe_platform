// Owned #[wasm_test] runner (features 12/13 — no wasm-bindgen, no wasm-pack).
//
// Loads `module.wasm` + `cases.json` (the testbed's __fwt_ discovery output), runs
// every case on foundation-wasm.js, and prints a summary. One FRESH instance per
// case: a wasm32 panic aborts, so a trapped instance is dead — re-instantiation is
// the only safe continuation. should_panic verdicts are inverted here (the module
// cannot observe its own panic under abort semantics).
//
// Runs unchanged under node (`node runner.mjs`), deno (`deno run -A runner.mjs`),
// and the browser (loaded as a module by index.html; results mirror into #output).

import { FoundationWasm, TestReports } from "./foundation-wasm.js";

const here = (path) => new URL(path, import.meta.url);

async function loadBytes(url) {
  if (url.protocol === "http:" || url.protocol === "https:") {
    return new Uint8Array(await (await fetch(url)).arrayBuffer());
  }
  const { readFileSync } = await import("node:fs");
  return readFileSync(url);
}

const out = [];
function print(line) {
  out.push(line);
  console.log(line);
  if (typeof document !== "undefined") {
    const el = document.getElementById("output");
    if (el) el.textContent = out.join("\n");
  }
}

function exit(code) {
  if (globalThis.process?.exit) globalThis.process.exit(code);
  else if (globalThis.Deno?.exit) globalThis.Deno.exit(code);
  // Browser: no exit — the summary line in #output is the verdict (Playwright reads it).
}

async function runCase(moduleCompiled, testCase) {
  const rt = new FoundationWasm();
  const instance = new WebAssembly.Instance(moduleCompiled, { abi: rt.web_abi });
  rt.init(instance);

  let trapped = false;
  let pendingAsync = false;
  try {
    pendingAsync = Number(instance.exports[testCase.export]()) === 1;
  } catch (error) {
    if (!(error instanceof WebAssembly.RuntimeError)) throw error;
    trapped = true; // the panic hook already reported before the abort trap
  }

  const report =
    pendingAsync || trapped || rt.testReports.pending > 0
      ? await rt.testReports.next()
      : { status: TestReports.FAIL, message: `${testCase.name}: produced no report` };

  if (report.status === TestReports.IGNORED) return { verdict: "ignored", report };
  const failed = report.status === TestReports.FAIL;
  if (testCase.should_panic) {
    // Inverted: the expected outcome IS the panic.
    return failed && trapped
      ? { verdict: "pass", report }
      : { verdict: "fail", report: { ...report, message: `${testCase.name}: expected a panic, none occurred` } };
  }
  return { verdict: failed ? "fail" : "pass", report };
}

async function main() {
  const cases = JSON.parse(new TextDecoder().decode(await loadBytes(here("./cases.json"))));
  const moduleCompiled = new WebAssembly.Module(await loadBytes(here("./module.wasm")));

  let passed = 0;
  let failed = 0;
  let ignored = 0;
  for (const testCase of cases) {
    const { verdict, report } = await runCase(moduleCompiled, testCase);
    if (verdict === "pass") {
      passed += 1;
      print(`ok      ${testCase.name}`);
    } else if (verdict === "ignored") {
      ignored += 1;
      print(`ignored ${testCase.name}`);
    } else {
      failed += 1;
      print(`FAILED  ${testCase.name}`);
      if (report.message) print(`        ${report.message}`);
    }
  }

  const status = failed === 0 ? "ok" : "FAILED";
  print(`test result: ${status}. ${passed} passed; ${failed} failed; ${ignored} ignored`);
  exit(failed === 0 ? 0 : 1);
}

main().catch((error) => {
  print(`test result: FAILED. runner error: ${error?.stack ?? error}`);
  exit(1);
});
