// primal-test.js — the injected test helper (spec-43 build-order §6).
//
// WHY: A browser test driver needs two browser-side primitives: wait until a
// server-driven update has settled (so an assertion isn't racing the stream),
// and read many element boxes in ONE layout pass (cheap multi-position asserts).
//
// WHAT: `installPrimalTest()` defines `window.__primalTest`:
//   - `frames()`        → the runtime frame instrument `{ seq, at }`.
//   - `waitForReactive({ quietMs, timeoutMs })` → resolves true once no frame has
//     applied for `quietMs` (the DOM is quiet), false on timeout.
//   - `batchLayout(selectors)` → `{ selector: {x,y,width,height} | null }` read in
//     one pass.
//
// HOW: Self-contained — it reads `globalThis.__primalFrames`, which the runtime
// (`foundation-wasm-ui.js`) updates on every applied frame. No imports, so the
// driver can inject it as a plain script on any page.

export function installPrimalTest(win = globalThis) {
  const now = () => (typeof performance !== "undefined" ? performance.now() : Date.now());
  const frames = () => win.__primalFrames ?? { seq: 0, at: null };

  const api = {
    frames,

    async waitForReactive({ quietMs = 50, timeoutMs = 5000 } = {}) {
      const start = now();
      for (;;) {
        const { at } = frames();
        if (at != null && now() - at >= quietMs) return true;
        if (now() - start > timeoutMs) return false;
        await new Promise((r) => setTimeout(r, Math.min(quietMs, 25)));
      }
    },

    batchLayout(selectors) {
      const doc = win.document;
      const out = {};
      for (const sel of selectors) {
        const el = doc.querySelector(sel);
        if (!el) {
          out[sel] = null;
          continue;
        }
        const r = el.getBoundingClientRect();
        out[sel] = { x: r.x, y: r.y, width: r.width, height: r.height };
      }
      return out;
    },
  };

  win.__primalTest = api;
  return api;
}

if (typeof window !== "undefined") installPrimalTest(window);
