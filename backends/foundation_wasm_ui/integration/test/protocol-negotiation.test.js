// Feature 04 (spec-42) — mount protocol negotiation: the `protocol`
// attribute override, WebSocket binary-frame envelope sniffing, and the
// base64 SSE `arrow` path. Precedence: attribute > headers/event name >
// channel default.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  decodeEnvelopeFrame,
  FetchTransport,
  ProtocolHandler,
  SSETransport,
  WebSocketTransport,
} from "../../runtimes/foundation-wasm-ui.js";

/** Envelope-frame a payload: [protocol][version][len u32 LE][payload]. */
function frame(protocol, version, payload) {
  const out = new Uint8Array(6 + payload.length);
  out[0] = protocol;
  out[1] = version;
  new DataView(out.buffer).setUint32(2, payload.length, true);
  out.set(payload, 6);
  return out;
}

/** A minimal wire-v1 columnar payload: pad shim + header, zero ops. */
function emptyColumnarPayload() {
  // pad_len=7 + 7 pad bytes + [count=0, flags=0] + the THREE empty string
  // columns (each contributes [last=0, dataLen=0] when count is 0).
  const out = new Uint8Array(1 + 7 + 8 + 3 * 8);
  out[0] = 7;
  return out;
}

function jsonResponse(body, contentType) {
  return {
    ok: true,
    status: 200,
    headers: { get: (k) => (k === "content-type" ? contentType : null) },
    json: async () => JSON.parse(body),
    text: async () => body,
    arrayBuffer: async () => new TextEncoder().encode(body).buffer,
  };
}

// ─── ProtocolHandler.named ─────────────────────────────────────────────────────

test("named() resolves the three handlers and rejects unknowns", () => {
  assert.ok(ProtocolHandler.named("arrow"));
  assert.ok(ProtocolHandler.named("json"));
  assert.ok(ProtocolHandler.named("html"));
  assert.equal(ProtocolHandler.named("yaml"), null);
  assert.equal(ProtocolHandler.named(undefined), null);
});

// ─── protocol attribute beats content-type (fetch) ────────────────────────────

test("fetch: protocol override wins over a wrong content-type", async () => {
  const transport = new FetchTransport({
    protocol: "json",
    fetchFn: async () => jsonResponse('{"sig": 1}', "text/html"), // header LIES
  });
  const result = await transport.send("/api", "GET");
  assert.equal(result.type, "json", "attribute > headers");
  assert.deepEqual(result.patches, { sig: 1 });
});

test("fetch: without the attribute, content-type still decides", async () => {
  const transport = new FetchTransport({
    fetchFn: async () => jsonResponse("<p>hi</p>", "text/html"),
  });
  const result = await transport.send("/api", "GET");
  assert.equal(result.type, "html");
});

// ─── SSE: attribute beats event names; arrow events decode base64 frames ─────

function sseResponse(text) {
  return {
    ok: true,
    status: 200,
    body: new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(text));
        controller.close();
      },
    }),
  };
}

test("sse: protocol attribute overrides the event name", async () => {
  const results = [];
  const transport = new SSETransport({
    protocol: "json",
    fetchFn: async () => sseResponse('event: html\ndata: {"a":1}\n\n'),
  });
  await transport.connect("/feed", undefined, (r) => results.push(r));
  transport.disconnect();
  assert.equal(results[0].type, "json", "attribute > event name");
});

test("sse: arrow events carry base64 envelope frames", async () => {
  const wire = frame(1, 1, emptyColumnarPayload());
  const b64 = Buffer.from(wire).toString("base64");
  const results = [];
  const transport = new SSETransport({
    fetchFn: async () => sseResponse(`event: arrow\ndata: ${b64}\n\n`),
  });
  await transport.connect("/feed", undefined, (r) => results.push(r));
  transport.disconnect();
  assert.equal(results[0].type, "arrow");
  assert.equal(results[0].columns.count, 0, "decoded through the envelope");
});

// ─── WebSocket: binary envelope sniffing + text default ───────────────────────

class FakeWebSocket {
  constructor() {
    this.sent = [];
    this.binaryType = "blob";
  }
  send(data) {
    this.sent.push(data);
  }
  close() {
    this.onclose?.();
  }
}

function wsHarness(config = {}) {
  const ws = new FakeWebSocket();
  const results = [];
  const transport = new WebSocketTransport({ ...config, wsFactory: () => ws });
  transport.connect("ws://x", undefined, (r) => results.push(r));
  return { ws, results, transport };
}

test("ws: binary v1 columnar frame routes as arrow via the envelope", () => {
  const { ws, results, transport } = wsHarness();
  assert.equal(ws.binaryType, "arraybuffer", "transport requests binary frames");
  ws.onmessage({ data: frame(1, 1, emptyColumnarPayload()).buffer });
  transport.disconnect();
  assert.equal(results.length, 1);
  assert.equal(results[0].type, "arrow");
  assert.equal(results[0].columns.count, 0);
});

test("ws: binary json envelope routes as json", () => {
  const { ws, results, transport } = wsHarness();
  ws.onmessage({ data: frame(2, 1, new TextEncoder().encode('{"n":2}')).buffer });
  transport.disconnect();
  assert.equal(results[0].type, "json");
  assert.deepEqual(results[0].patches, { n: 2 });
});

test("ws: malformed binary surfaces an error, applies NOTHING", () => {
  const { ws, results, transport } = wsHarness();
  const errors = [];
  const original = console.error;
  console.error = (...args) => errors.push(args.join(" "));
  try {
    ws.onmessage({ data: new Uint8Array([1, 1, 99, 0, 0, 0, 7]).buffer }); // length lies
    ws.onmessage({ data: new Uint8Array([9, 9, 0, 0, 0, 0]).buffer }); // unknown protocol
  } finally {
    console.error = original;
  }
  transport.disconnect();
  assert.equal(results.length, 0, "no silent json attempt");
  assert.equal(errors.length, 2, "typed errors surfaced");
});

test("ws: text frames keep the json default; protocol attr overrides", () => {
  const a = wsHarness();
  a.ws.onmessage({ data: '{"x":1}' });
  a.transport.disconnect();
  assert.equal(a.results[0].type, "json", "pre-feature-04 text default preserved");

  const b = wsHarness({ protocol: "html" });
  b.ws.onmessage({ data: "<p>hi</p>" });
  b.transport.disconnect();
  assert.equal(b.results[0].type, "html", "attribute overrides the text default");
});

// ─── decodeEnvelopeFrame: v2 requires a registered reader ─────────────────────

test("wire v2 without a reader is a typed error; with one, it routes", () => {
  const payload = new Uint8Array([1, 2, 3]);
  const errors = [];
  const original = console.error;
  console.error = (...args) => errors.push(args.join(" "));
  let result;
  try {
    result = decodeEnvelopeFrame(frame(1, 2, payload));
  } finally {
    console.error = original;
  }
  assert.equal(result, null);
  assert.match(errors[0], /no reader is registered/);

  ProtocolHandler.arrowIpcReader = (bytes) => ({ rows: bytes.length });
  try {
    const routed = decodeEnvelopeFrame(frame(1, 2, payload));
    assert.equal(routed.type, "arrow-ipc");
    assert.deepEqual(routed.table, { rows: 3 });
  } finally {
    delete ProtocolHandler.arrowIpcReader;
  }
});
