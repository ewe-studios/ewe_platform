import * as wire from "./foundation-ui-wire.js";
import { VOCABULARY_VERSION } from "./foundation-ui-vocab.js";
import { base64urlEncode } from "./foundation-ui-frames.js";

function requestBytes(session, request) {
  const builder = new wire.Builder();
  const frame = new wire.FrameT(1, session, 0n, wire.FrameKind.FrameRequest, request);
  wire.Frame.finishFrameBuffer(builder, frame.pack(builder));
  return builder.asUint8Array();
}
function pushed(bytes) {
  const frame = wire.Frame.getRootAsFrame(new wire.ByteBuffer(bytes));
  const body = frame.kindType() === wire.FrameKind.FrameMessage && frame.kind(new wire.FrameMessage());
  const control = body && body.messagesLength() ? body.messages(0)?.control() : null;
  return {frame, body, control};
}

import { frameMountTarget, frameConnections } from "./foundation-wasm-ui.js";

// Associate an ordered stream with the element that owns its connection.
function integer(element, attribute) {
  const raw = element.getAttribute(attribute);
  if (raw === null) return undefined;
  if (!/^\d+$/.test(raw)) throw new TypeError("Expected an unsigned integer for " + attribute);
  const n = BigInt(raw);
  if (n > 0xffffffffffffffffn) throw new RangeError(attribute + " exceeds u64");
  return n;
}

export function bindLiveMounts(runtime, frames) {
  const streams = new Map();
  const connections = frames.deps.connections ?? frameConnections();
  runtime.connections = connections;
  const previousGap = frames.deps.onGap;
  frames.deps.onGap = gap => {
    for (const binding of streams.get(gap.streamId) ?? []) {
      if (!gap.view || gap.view === binding.owner) binding.resume(gap.resumeFrom);
    }
    previousGap?.(gap);
  };
  runtime.onMountConnect = (element, streaming) => {
    if (element.getAttribute("protocol") !== "frame") return null;
    let stream = integer(element, "data-channel-id");
    const owner = frames.viewOwner(element);
    const recovery = frames.loaders.get(owner);
    const generation = recovery?.generation;
    const current = () => active && (!recovery || recovery.accepts(generation));
    const expired = () => {
      if (!current()) return;
      if (recovery) {
        // The loader reports a failed recovery to its owner.
        Promise.resolve(recovery.expire(stream)).catch(() => {});
      }
      else if (frames.deps.requestSnapshot) Promise.resolve(frames.deps.requestSnapshot({element, stream})).catch(error => frames.deps.onError?.(error));
      else frames.deps.onError?.({code:"ReplayExpired", stream});
    };
    const start = integer(element, "data-resume-from") ?? 0n;
    const local = element.getAttribute("transport") === "local";
    let active = true;
    let resumeOnConnection;
    function attach(id) {
      stream = id;
      if (start && frames.cursor(id, owner) === undefined) frames.setCursor(id, start, owner);
      if (streaming && !local) {
        let mounts = streams.get(id);
        if (!mounts) streams.set(id, mounts = new Set());
        mounts.add(binding);
      }
    }
    const binding = {
      local,
      owner,
      expired,
      managed({ api, data, transport }) {
        const kind = element.getAttribute("transport") ?? "sse";
        const sharing = element.getAttribute("data-connection-shared");
        const shared = sharing === null ? kind === "websocket" || kind === "ws" : sharing === "true";
        const keepAlive = element.getAttribute("data-connection-keep-alive") === "true";
        const socket = kind === "websocket" || kind === "ws";
        const adapter = socket ? "websocket" : kind === "http" ? "chunked" : kind;
        if (!connections.supports(adapter)) return null;
        const negotiate = Boolean(frames.deps.deliverySession) && (socket || ["sse", "chunked"].includes(adapter));
        if (!shared && !keepAlive && !negotiate) return null;
        if (sharing === null && !keepAlive && !negotiate && !frames.deps.connections) return null;
        const endpoint = (shared || negotiate) ? api : binding.url(api);
        const subscription = connections.nextTag();
        const hello = new wire.ClientHelloT(VOCABULARY_VERSION, wire.Platform.Web, wire.MountSurface.Web, 0n, true, subscription);
        const greeting = requestBytes(0n, new wire.FrameRequestT(null, hello));
        const report = error => frames.deps.onError?.({code: "Transport", detail: String(error)});
        let handle, session, wasConnected = false, pendingCursor = recovery && !(shared && adapter === "sse") ? start : start || null;
        const resume = async cursor => {
          if (stream === undefined || !session) { pendingCursor = cursor; return; }
          const request = new wire.FrameRequestT(null, null, new wire.ResumeSubscriptionT(session, subscription, stream, cursor));
          const bytes = requestBytes(session, request);
          if (socket) await handle.send(bytes);
          else {
            const response = await (frames.deps.fetchFn ?? fetch)(endpoint, {method: "POST", body: bytes,
              headers: {...frames.deps.connectionHeaders, "content-type": "application/primal-frame"}});
            if (response.status === 410) { expired(); return; }
            if (!response.ok) throw new Error("Resume request failed with HTTP " + response.status);
          }
        };
        const context = bytes => {
          const caps = pushed(bytes).control?.caps();
          if (!caps?.subscriptionResume() || (socket && caps.requestTag() !== subscription)) return;
          session = caps.streamId();
          if (wasConnected && !shared && stream !== undefined) pendingCursor ??= frames.cursor(stream, owner) ?? start;
          wasConnected = true;
          if (pendingCursor !== null) { const cursor = pendingCursor; pendingCursor = null; resume(cursor).catch(report); }
        };
        if (negotiate) resumeOnConnection = cursor => resume(cursor).catch(report);
        handle = connections.acquire({
          transport: adapter, endpoint, shared, keepAlive,
          base: element.ownerDocument?.baseURI ?? globalThis.location?.href,
          headers: negotiate && !socket ? {...frames.deps.connectionHeaders, Accept: adapter === "chunked" ? "application/primal-frame" : "text/event-stream", "X-Ewe-Hello": base64urlEncode(greeting)} : frames.deps.connectionHeaders,
          isContext: !socket && negotiate ? bytes => Boolean(pushed(bytes).control?.caps()) : undefined,
          channel: element._transportOverride, settings: element._transportOverride ?? (element.hasAttribute?.("data") ? data : frames.deps.connectionSettings),
          authority: frames.deps.authority,
          data: element.getAttribute("data") === null ? undefined : data,
        }, {
          owner,
          onOpen() {
            if (socket && negotiate) queueMicrotask(() => { if (active) handle.send(greeting).catch(report); });
          },
          onContext: context,
          accept(bytes) {
            const {frame, body, control} = pushed(bytes);
            if (!body) return false;
            if (control?.caps()) return socket && control.caps().requestTag() === subscription;
            return (stream === undefined || stream === frame.streamId()) &&
              !body.ack() && (!body.subscriptionId() || body.subscriptionId() === subscription);
          },
          receive(bytes) {
            const {control} = pushed(bytes);
            if (control?.caps()) { context(bytes); return; }
            if (control?.replayExpired()) {
              expired();
              return;
            }
            binding.receive({ type: "frame", frames: [bytes] });
          },
          onError(cause) {
            session = null;
            if ((socket || !shared) && stream !== undefined) pendingCursor = frames.cursor(stream, owner) ?? start;
            report(cause);
          },
        });
        return handle;
      },
      url(api) {
        const cursor = stream === undefined ? undefined : frames.cursor(stream, owner) ?? start;
        if (stream === undefined || !cursor) return api;
        const url = new URL(api, element.ownerDocument?.baseURI ?? globalThis.location.href);
        url.searchParams.set("stream_id", stream.toString());
        url.searchParams.set("resume_from", cursor.toString());
        return url.href;
      },
      receive(result) {
        if (!current()) return;
        if (result.type !== "frame") throw new Error("Frame mount received another protocol");
        for (const bytes of result.frames) {
          frames.receive(bytes, { active:current, get viewMount() {
            return active && frameMountTarget(element, element.getAttribute("target"), element.ownerDocument);
          }, observeStream(id) {
            if (stream === undefined) attach(id);
            else if (stream !== id) throw new Error("Unexpected stream identifier");
          } });
        }
      },
      resume(cursor) {
        if (!current()) return;
        element.setAttribute("data-resume-from", String(cursor));
        if (resumeOnConnection) resumeOnConnection(cursor);
        else element._restartFrameStream?.();
      },
      close() {
        active = false;
        const mounts = streams.get(stream);
        mounts?.delete(binding);
        if (mounts?.size === 0) streams.delete(stream);
      },
    };
    if (stream !== undefined) attach(stream);
    return binding;
  };
}
