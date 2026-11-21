import { frameMountTarget } from "./foundation-wasm-ui.js";

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
  const previousGap = frames.deps.onGap;
  frames.deps.onGap = gap => {
    for (const binding of streams.get(gap.streamId) ?? []) binding.resume(gap.resumeFrom);
    previousGap?.(gap);
  };
  runtime.onMountConnect = (element, streaming) => {
    if (element.getAttribute("protocol") !== "frame") return null;
    let stream = integer(element, "data-channel-id");
    const start = integer(element, "data-resume-from") ?? 0n;
    const local = element.getAttribute("transport") === "local";
    let active = true;
    function attach(id) {
      stream = id;
      if (start && frames.cursor(id) === undefined) frames.setCursor(id, start);
      if (streaming && !local) {
        let mounts = streams.get(id);
        if (!mounts) streams.set(id, mounts = new Set());
        mounts.add(binding);
      }
    }
    const binding = {
      local,
      url(api) {
        const cursor = stream === undefined ? undefined : frames.cursor(stream) ?? start;
        if (stream === undefined || !cursor) return api;
        const url = new URL(api, element.ownerDocument?.baseURI ?? globalThis.location.href);
        url.searchParams.set("stream_id", stream.toString());
        url.searchParams.set("resume_from", cursor.toString());
        return url.href;
      },
      receive(result) {
        if (!active) return;
        if (result.type !== "frame") throw new Error("Frame mount received another protocol");
        for (const bytes of result.frames) {
          frames.receive(bytes, { get viewMount() {
            return active && frameMountTarget(element, element.getAttribute("target"), element.ownerDocument);
          }, observeStream(id) {
            if (stream === undefined) attach(id);
            else if (stream !== id) throw new Error("Unexpected stream identifier");
          } });
        }
      },
      resume(cursor) {
        if (!active) return;
        element.setAttribute("data-resume-from", String(cursor));
        element._restartFrameStream?.();
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
