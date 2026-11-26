// Run with node --test backends/foundation_compact/tests/wasm_hosts/crypto.test.mjs.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { FoundationWasm } from "../../../foundation_wasm/runtime/foundation-wasm.js";

const root = fileURLToPath(new URL("../../../../", import.meta.url));

function buildProbe(backend) {
  const output = execFileSync("cargo", [
    "build", "--profile", "uat", "--package", "foundation_compact",
    "--example", "compact_crypto_probe", "--target", "wasm32-unknown-unknown",
    "--no-default-features", "--features", `std,crypto-chacha,${backend}`,
    "--message-format=json",
  ], { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
  const artifacts = output.trim().split("\n").map((line) => JSON.parse(line));
  const probe = artifacts.find((record) => record.reason === "compiler-artifact" &&
    record.target.name === "compact_crypto_probe");
  assert.ok(probe, "Cargo produced the probe");
  return probe.filenames.find((name) => name.endsWith(".wasm"));
}

for (const backend of ["wasm-bindgen", "host-abi"]) {
  test(`host entropy and authenticated encryption through ${backend}`, async (t) => {
    const wasm = buildProbe(backend);
    let check;
    if (backend === "wasm-bindgen") {
      const directory = mkdtempSync(join(tmpdir(), "compact-crypto-"));
      t.after(() => rmSync(directory, { recursive: true, force: true }));
      execFileSync("wasm-bindgen", [wasm, "--target", "nodejs", "--out-dir", directory]);
      check = createRequire(import.meta.url)(join(directory, "compact_crypto_probe.js")).check_host_crypto;
    } else {
      const runtime = new FoundationWasm();
      const { instance } = await WebAssembly.instantiate(readFileSync(wasm), { abi: runtime.web_abi });
      runtime.init(instance);
      check = instance.exports.check_host_crypto;
    }
    assert.equal(check(), 0, "real Web Crypto supplies the nonce");
    const source = Object.getOwnPropertyDescriptor(globalThis, "crypto");
    try {
      Object.defineProperty(globalThis, "crypto", { configurable: true, value: undefined });
      assert.equal(check(), 1, "the caller receives an entropy error");
    } finally {
      Object.defineProperty(globalThis, "crypto", source);
    }
  });
}
