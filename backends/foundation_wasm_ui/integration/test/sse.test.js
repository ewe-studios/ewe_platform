// Feature 21 — the owned SSE stack: SseParser mirroring the Rust
// foundation_netio parser semantics (plus chunk-boundary torture the byte
// stream adds), and FetchEventSource (any-method SSE over fetch) with
// reconnect/Last-Event-ID/retry/close behavior.

import { test } from "node:test";
import assert from "node:assert/strict";

import { SseParser, FetchEventSource, SSETransport } from "../../runtimes/foundation-wasm-ui.js";

const enc = new TextEncoder();

/** Feed a whole string at once; return all events incl. EOF flush. */
function parseAll(text) {
  const parser = new SseParser();
  return [...parser.push(enc.encode(text)), ...parser.end()];
}

// ─── Parser battery (mirrors the Rust unit tests) ─────────────────────────────

test("simple message", () => {
  const events = parseAll("data: hello\n\n");
  assert.equal(events.length, 1);
  assert.equal(events[0].data, "hello");
  assert.equal(events[0].event, null);
  assert.equal(events[0].id, null);
});

test("message with id updates lastEventId", () => {
  const events = parseAll("id: 42\ndata: x\n\ndata: y\n\n");
  assert.equal(events[0].id, "42");
  assert.equal(events[0].lastEventId, "42");
  assert.equal(events[1].id, null);
  assert.equal(events[1].lastEventId, "42", "persists across events");
});

test("event type", () => {
  const events = parseAll("event: update\ndata: payload\n\n");
  assert.equal(events[0].event, "update");
});

test("multiline data joins with newline", () => {
  const events = parseAll("data: line1\ndata: line2\ndata: line3\n\n");
  assert.equal(events[0].data, "line1\nline2\nline3");
});

test("comments surface immediately", () => {
  const events = parseAll(": keep-alive\ndata: real\n\n");
  assert.equal(events[0].type, "comment");
  assert.equal(events[0].comment, "keep-alive");
  assert.equal(events[1].data, "real");
});

test("empty lines without data emit nothing", () => {
  const events = parseAll("\n\n\ndata: after\n\n");
  assert.equal(events.length, 1);
  assert.equal(events[0].data, "after");
});

test("retry field parses integers only", () => {
  const events = parseAll("retry: 3000\ndata: x\n\nretry: nope\ndata: y\n\n");
  assert.equal(events[0].retry, 3000);
  assert.equal(events[1].retry, null, "non-numeric retry ignored");
});

test("EOF flushes accumulated data", () => {
  const events = parseAll("data: trailing"); // no blank line, no final newline
  assert.equal(events.length, 1);
  assert.equal(events[0].data, "trailing");
});

test("id containing NUL is rejected", () => {
  const events = parseAll("id: bad\0id\ndata: x\n\n");
  assert.equal(events[0].id, null);
});

test("unknown fields and no-colon lines are ignored", () => {
  const events = parseAll("custom: nope\njustnoise\ndata: kept\n\n");
  assert.equal(events.length, 1);
  assert.equal(events[0].data, "kept");
});

test("CRLF and lone-CR line endings", () => {
  const events = parseAll("data: a\r\ndata: b\rdata: c\n\r\n");
  assert.equal(events[0].data, "a\nb\nc");
});

test("value space stripping is exactly one space", () => {
  const events = parseAll("data:  two spaces\ndata:none\n\n");
  assert.equal(events[0].data, " two spaces\nnone");
});

// ─── Chunk-boundary torture (the byte-stream additions) ───────────────────────

test("line split across chunks", () => {
  const parser = new SseParser();
  let events = parser.push(enc.encode("data: hel"));
  assert.equal(events.length, 0, "partial line buffered");
  events = parser.push(enc.encode("lo\n\n"));
  assert.equal(events[0].data, "hello");
});

test("CRLF split across chunks", () => {
  const parser = new SseParser();
  parser.push(enc.encode("data: x\r"));
  const events = parser.push(enc.encode("\ndata: y\r\n\r\n"));
  assert.equal(events[0].data, "x\ny", "the split \\r\\n is ONE terminator");
});

test("multi-byte UTF-8 split across chunks", () => {
  const bytes = enc.encode("data: café \u{1f980}\n\n");
  const parser = new SseParser();
  const events = [];
  // Split INSIDE the crab emoji's 4-byte sequence.
  const cut = bytes.length - 4;
  events.push(...parser.push(bytes.slice(0, cut)));
  events.push(...parser.push(bytes.slice(cut)));
  assert.equal(events[0].data, "café \u{1f980}");
});

test("one byte at a time", () => {
  const bytes = enc.encode("event: tick\nid: 7\ndata: ok\n\n");
  const parser = new SseParser();
  const events = [];
  for (const byte of bytes) events.push(...parser.push(Uint8Array.of(byte)));
  assert.equal(events.length, 1);
  assert.deepEqual(
    [events[0].event, events[0].id, events[0].data],
    ["tick", "7", "ok"],
  );
});

// ─── FetchEventSource ──────────────────────────────────────────────────────────

/** A Response-alike whose body streams the given byte chunks. */
function streamedResponse(chunks, { ok = true, status = 200 } = {}) {
  return {
    ok,
    status,
    body: new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(enc.encode(chunk));
        controller.close();
      },
    }),
  };
}

test("any-method SSE: POST body + headers reach the fetch", async () => {
  const captured = [];
  const events = [];
  const source = new FetchEventSource("/feed", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: '{"q":1}',
    fetchFn: async (url, init) => {
      captured.push({ url, init });
      return streamedResponse(["event: json\ndata: {\"n\":1}\n\n"]);
    },
    onEvent: (e) => events.push(e),
  });
  await source.connect();
  source.close();

  assert.equal(captured[0].url, "/feed");
  assert.equal(captured[0].init.method, "POST", "EventSource could never do this");
  assert.equal(captured[0].init.body, '{"q":1}');
  assert.equal(captured[0].init.headers.accept, "text/event-stream");
  assert.equal(events[0].event, "json");
});

test("reconnect carries Last-Event-ID and honors retry", async () => {
  const inits = [];
  let calls = 0;
  const source = new FetchEventSource("/feed", {
    fetchFn: async (_url, init) => {
      inits.push(init);
      calls += 1;
      if (calls === 1) {
        return streamedResponse(["retry: 1\nid: 9\ndata: first\n\n"]);
      }
      return streamedResponse(["data: second\n\n"]);
    },
    onEvent: () => {},
  });
  await source.connect();
  // The first stream ended; the reconnect timer (1ms via retry:) is pending.
  await new Promise((resolve) => setTimeout(resolve, 20));
  source.close();

  assert.ok(calls >= 2, "reconnected after stream end");
  assert.equal(inits[1].headers["last-event-id"], "9");
});

test("close() stops reconnection", async () => {
  let calls = 0;
  const source = new FetchEventSource("/feed", {
    fetchFn: async () => {
      calls += 1;
      return streamedResponse(["retry: 1\ndata: x\n\n"]);
    },
    onEvent: () => {},
  });
  await source.connect();
  source.close();
  await new Promise((resolve) => setTimeout(resolve, 25));
  assert.equal(calls, 1, "no reconnect after close");
});

// ─── SSETransport over the owned stack ─────────────────────────────────────────

test("SSETransport: POST when data given, event-name demux as before", async () => {
  const results = [];
  let init = null;
  const transport = new SSETransport({
    fetchFn: async (_url, i) => {
      init = i;
      return streamedResponse([
        "event: html\ndata: <p>hi</p>\n\n",
        "data: unnamed-defaults-to-html\n\n",
      ]);
    },
  });
  await transport.connect("/feed", { filter: "a" }, (r) => results.push(r));
  transport.disconnect();

  assert.equal(init.method, "POST", "data implies POST");
  assert.equal(init.headers["content-type"], "application/json");
  assert.equal(results[0].type, "html");
  assert.equal(results[1].type, "html", "unnamed event defaulted to html");
});
