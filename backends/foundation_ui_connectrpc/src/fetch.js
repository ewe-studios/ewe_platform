// JavaScript owns each Fetch response and its reader. Rust retains only the identifiers.
let next = 0;
const sessions = new Map();
const exchanges = new Map();
function allocate(table, value) {
  if (next >= 0xffffffff) throw new Error("No RPC handle identifiers remain");
  table.set(++next, value);
  return next;
}
export function registerSession(fetcher) { return allocate(sessions, fetcher); }
export function releaseSession(id) { sessions.delete(id); }
export function openExchange(session, method, url, headers) {
  const fetcher = sessions.get(session);
  if (!fetcher) throw new Error("Missing RPC session");
  return allocate(exchanges, { fetcher, method, url, headers: JSON.parse(headers), abort: new AbortController() });
}
export async function startExchange(id, body) {
  const e = exchanges.get(id);
  const response = await e.fetcher(e.url, { method: e.method, headers: e.headers,
    body: e.method === "GET" || e.method === "HEAD" ? undefined : body.slice(), signal: e.abort.signal });
  e.reader = response.body?.getReader();
  // Fetch exposes decoded content. Keep Connect envelope compression headers for Rust.
  const headers = new Headers(response.headers);
  if (headers.has("content-encoding")) {
    headers.delete("content-encoding");
    headers.delete("content-length");
  }
  return JSON.stringify([response.status, Array.from(headers.entries())]);
}
export async function readExchange(id, limit) {
  const e = exchanges.get(id);
  while (!e.chunk?.length) {
    if (!e.reader) return undefined;
    const part = await e.reader.read();
    if (part.done) return undefined;
    e.chunk = part.value;
  }
  const chunk = e.chunk.subarray(0, limit);
  e.chunk = e.chunk.subarray(chunk.length);
  return chunk;
}
export function closeExchange(id) {
  const e = exchanges.get(id);
  if (e) { exchanges.delete(id); e.abort.abort(); e.reader?.releaseLock(); }
}
