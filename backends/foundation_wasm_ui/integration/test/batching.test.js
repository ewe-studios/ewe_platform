// Feature 11 — request batching: probe detection, the id'd batch format with
// single-request passthrough, batch-response routing per content type, and
// the WebSocket / Worker microtask coalescing queues.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  RequestQueue,
  WSBatchQueue,
  WorkerBatchQueue,
  probeBatching,
  parseBatchEntry,
} from "../../runtimes/foundation-wasm-ui.js";

/// Spec tests 1-3 — probe detection.
test("probeBatching: 200 enables, 404/throw disable", async () => {
  assert.equal(await probeBatching(async () => ({ ok: true })), true);
  assert.equal(await probeBatching(async () => ({ ok: false })), false);
  assert.equal(
    await probeBatching(async () => {
      throw new Error("offline");
    }),
    false,
  );
});

function captureTransport() {
  const sent = [];
  return { sent, transport: { send: (url, method, data) => sent.push({ url, method, data }) } };
}

/// Spec test 4 — three requests, one POST with three id'd items.
test("three same-tick requests flush as one id'd batch", async () => {
  const { sent, transport } = captureTransport();
  const queue = new RequestQueue(transport);
  queue.bundlingEnabled = true;

  queue.enqueue({ url: "/a", method: "GET" });
  queue.enqueue({ url: "/b", method: "POST", data: { x: 1 } });
  queue.enqueue({ url: "/c", method: "GET" });
  assert.equal(sent.length, 0, "nothing until the microtask");
  await Promise.resolve();

  assert.equal(sent.length, 1);
  assert.equal(sent[0].url, "/primal/messages");
  assert.equal(sent[0].method, "POST");
  assert.deepEqual(
    sent[0].data.map((r) => [r.id, r.url]),
    [
      [1, "/a"],
      [2, "/b"],
      [3, "/c"],
    ],
    "ids are sequential, order preserved",
  );
  assert.deepEqual(sent[0].data[1].body, { x: 1 });
});

/// Spec test 5 — a single queued request goes out as itself.
test("single request skips the batch wrapper", async () => {
  const { sent, transport } = captureTransport();
  const queue = new RequestQueue(transport);
  queue.bundlingEnabled = true;
  queue.enqueue({ url: "/solo", method: "PUT", data: 7 });
  await Promise.resolve();
  assert.deepEqual(sent, [{ url: "/solo", method: "PUT", data: 7 }]);
});

/// Spec tests 6 + 7 — disabled passthrough; empty flush is silent.
test("disabled queue passes through; empty flush is a no-op", async () => {
  const { sent, transport } = captureTransport();
  const queue = new RequestQueue(transport);
  queue.enqueue({ url: "/direct", method: "GET" });
  assert.equal(sent.length, 1, "sent immediately when disabled");
  queue.flush();
  assert.equal(sent.length, 1, "empty flush made no call");
});

/// Spec test 8 — batch responses route by per-entry content type.
test("parseBatchEntry routes per content type", () => {
  const html = parseBatchEntry({
    id: 1,
    status: 200,
    headers: { "content-type": "application/primal-html" },
    body: "<p>hello</p>",
  });
  assert.equal(html.type, "html");
  assert.equal(html.id, 1);

  const json = parseBatchEntry({
    id: 2,
    status: 200,
    headers: { "content-type": "application/primal-json" },
    body: { morph: { target: "#m", action: "replace-children", content: "<b/>" } },
  });
  assert.equal(json.type, "json-morph");

  const raw = parseBatchEntry({ id: 3, status: 204, headers: {}, body: "plain" });
  assert.equal(raw.type, "raw");
});

/// Spec tests 9-11 — WebSocket coalescing.
test("WSBatchQueue: single unwrapped, multiple wrapped, per-tick batches", async () => {
  const frames = [];
  const queue = new WSBatchQueue({ send: (f) => frames.push(JSON.parse(f)) });

  queue.send({ a: 1 });
  await Promise.resolve();
  assert.deepEqual(frames, [{ a: 1 }], "single message ships as-is");

  queue.send({ b: 2 });
  queue.send({ c: 3 });
  queue.send({ d: 4 });
  await Promise.resolve();
  assert.equal(frames.length, 2);
  assert.deepEqual(frames[1], { batch: [{ b: 2 }, { c: 3 }, { d: 4 }] });

  // A later tick gets its own frame.
  queue.send({ e: 5 });
  await Promise.resolve();
  assert.deepEqual(frames[2], { e: 5 });
});

/// Spec tests 12-13 — worker coalescing with transferables.
test("WorkerBatchQueue: one postMessage per tick, transferables forwarded", async () => {
  const calls = [];
  const queue = new WorkerBatchQueue({
    postMessage: (data, transfers) => calls.push({ data, transfers }),
  });
  const buffer = new ArrayBuffer(8);
  queue.postMessage({ kind: "a" });
  queue.postMessage({ kind: "b", transferable: buffer });
  await Promise.resolve();

  assert.equal(calls.length, 1);
  assert.equal(calls[0].data.batch.length, 2);
  assert.deepEqual(calls[0].transfers, [buffer], "buffer transferred, not copied");
});
