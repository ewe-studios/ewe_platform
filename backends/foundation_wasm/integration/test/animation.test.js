// Tests for the AnimationDriver in foundation-wasm.js — the hook_up_animation_frames
// import driving trigger_animation_callbacks until get_total_animation_callbacks() is 0.
// Uses a fake requestAnimationFrame so the test drives frames deterministically.

import { test } from "node:test";
import assert from "node:assert/strict";

import { FoundationWasm } from "../../runtime/foundation-wasm.js";
import { makeMockWasm } from "../mock-wasm.js";

/** A fake rAF: captures frame callbacks so the test fires them by hand. */
function fakeRaf() {
  const frames = [];
  let canceled = 0;
  return {
    raf: { request: (cb) => { frames.push(cb); return frames.length; }, cancel: () => { canceled += 1; } },
    frames,
    get canceled() { return canceled; },
  };
}

function boot(rafHost) {
  const mock = makeMockWasm();
  const rt = new FoundationWasm({ rafHost });
  rt.init({ exports: { ...mock.exports, memory: mock.memory } });
  return { rt, mock };
}

test("hook_up_animation_frames runs frames until callbacks drain, then stops", () => {
  const raf = fakeRaf();
  const { rt, mock } = boot(raf.raf);
  mock.exports.__setAnimationCallbacks(2); // two callbacks; each frame consumes one

  rt.web_abi.hook_up_animation_frames();
  assert.equal(raf.frames.length, 1, "first frame scheduled");

  raf.frames[0](16); // count 2 -> 1, still > 0 -> reschedule
  assert.equal(raf.frames.length, 2, "rescheduled while callbacks remain");

  raf.frames[1](32); // count 1 -> 0 -> stop
  assert.equal(raf.frames.length, 2, "no reschedule once callbacks drain");
  assert.equal(rt.animation.running, false);
  assert.deepEqual(mock.calls.trigger_animation_callbacks, [16, 32]);
});

test("hook_up_animation_frames is idempotent while a loop is running", () => {
  const raf = fakeRaf();
  const { rt, mock } = boot(raf.raf);
  mock.exports.__setAnimationCallbacks(5);

  rt.web_abi.hook_up_animation_frames();
  rt.web_abi.hook_up_animation_frames(); // second call must not start a 2nd loop
  assert.equal(raf.frames.length, 1, "single loop despite repeated hook-up");
});

test("stop() cancels a running loop", () => {
  const raf = fakeRaf();
  const { rt, mock } = boot(raf.raf);
  mock.exports.__setAnimationCallbacks(3);
  rt.web_abi.hook_up_animation_frames();
  rt.animation.stop();
  assert.equal(raf.canceled, 1);
  assert.equal(rt.animation.running, false);
});
