# 13 — API surface: cached/offline replay

**Date:** 2026-07-04
**Status:** Resolved (architecture), design exploration

### Decision

This is the "always responsive" surface. When connectivity is absent or a
remote fetch fails, the shell serves cached rendered content from local
storage. The user perceives an instant, working app. When connectivity
returns, the backend updates the page — either a full replace or surgical
diff. The session backbone coordinates cache lookups, rendering, and
subsequent updates.

This is not a standalone surface. It is a capability that every other surface
can use. A bundled WASM app may never need it. A remote server-driven app
needs it constantly.

### The Rust shell in this context

This is not a standalone deployment context — it is a capability that the
shell provides to every other surface. The shell always ships with a cache
layer. Every deployment context (bundled WASM, native lib, shell+WASM,
remote server-driven) can use it.

**In the context of caching, the shell:**

- Owns the cache database (SQLite, stored in the app's data directory,
  respecting Tauri's filesystem scope).
- Intercepts every route resolution. Before the session queries a backend,
  the shell checks the cache. If the cache policy says "cache-first" and a
  cached entry exists, the shell serves it immediately — no backend call.
- Manages the cache lifecycle: stores rendered pages on navigation, serves
  them on offline/reconnect, invalidates stale entries, prunes old entries
  when the size limit is hit.
- Handles the offline→online transition: serves cached content, replays
  the mutation queue, fetches fresh content, applies the update strategy
  (full replace or surgical diff), updates the cache.
- Is transparent to the user's backend code. The backend doesn't know
  whether the content it's serving is being cached. The route handler sets
  the cache policy; the shell executes it.

**What the shell always provides, regardless of context:**

- Session backbone — cache lookups are part of route resolution.
- Cache database — SQLite, always present, always available.
- Transport lanes — cached content is served through the custom protocol
  lane or the rendering lane, depending on the content type.
- Lifecycle management — cache is flushed on shutdown, validated on startup.

### What the shell provides

- **Rendered page cache** — a local SQLite database indexed by route. The
  shell stores the last rendered version of each page. Cache entries carry:
  route, timestamp, protocol type (HTML/DomOps/Arrow), raw bytes, version tag.
- **Cache API** — `session.cache().store(route, content)`,
  `session.cache().get(route) -> Option<CachedPage>`,
  `session.cache().invalidate(route)`, `session.cache().invalidate_all()`.
  Simple, fast, predictable.
- **Two update strategies, user-selectable per route:**

  **Full replace.** The shell serves the cached page instantly. When the
  backend sends the latest page, the shell replaces the entire view with the
  fresh content. Best for content-driven screens where surgical patching adds
  no value. Conceptually: "show the user what we had, then swap it out when
  the real thing arrives."

  **Surgical update.** The shell serves the cached page instantly. The client
  sends cached state to the backend (e.g., "user was on step 3 of this form
  with these values"). The backend responds with only what changed — DomOps,
  morph patches, or Arrow deltas. The shell applies them without disrupting
  the user's context. Best for preserving in-progress work.

- **Cache policy per route** — the route handler's `RouteDecision` carries:
  - `CacheFirst` — serve from cache, update in background.
  - `NetworkFirst` — try network, fall back to cache.
  - `OnlineOnly` — never serve from cache (auth screens, real-time data).
  - `LocalOnly` — never hit the network (bundled screens).
  - `StaleWhileRevalidate` — serve cache, fetch fresh in background, update
    when ready.

- **Snapshot resurrection** — when the user returns to a previously visited
  route, the shell serves the cached snapshot instantly. The app feels swift
  before any network response. When the backend responds, the shell applies
  the update strategy (full replace or surgical).

- **Offline mutation queue** — when the user performs an action while offline
  (form submit, data update), the shell can enqueue it. On reconnect, the
  queue replays. This is an optional layer — the user brings their own queue
  implementation if they need it. The platform provides the primitives (queue
  storage, replay trigger on connectivity change) but doesn't own the conflict
  model.

### How the user wires up

```rust
// Cache policy is per-route, in the RouteDecision.
session.route("/app/content/*", RouteDecision::remote_fetch()
    .with_cache_policy(CachePolicy::StaleWhileRevalidate)
    .with_update_strategy(UpdateStrategy::Surgical));

session.route("/app/static/*", RouteDecision::remote_fetch()
    .with_cache_policy(CachePolicy::CacheFirst)
    .with_update_strategy(UpdateStrategy::FullReplace));

session.route("/app/auth/*", RouteDecision::remote_fetch()
    .with_cache_policy(CachePolicy::OnlineOnly)); // never stale

// Manual cache operations when needed:
session.cache().preload("/app/offline-ready", &content);
session.cache().invalidate("/app/outdated");
```

### What the platform does NOT provide

- **Conflict resolution** — the user's backend decides what happens when a
  cached state conflicts with the server state. The platform delivers both
  the cached snapshot and the fresh content; the backend reconcilies.
- **Sync protocol** — the platform does not implement CRDTs, event sourcing,
  or last-write-wins. Users bring their own or use future platform-provided
  sync modules.
- **Mutation queue logic** — the platform provides queue storage and replay
  triggers. The user defines what goes in the queue and how it replays.

### How it works across surfaces

- **Bundled WASM (surface 9):** cache is mostly unnecessary — the WASM runs
  locally. But the shell still caches rendered pages for instant restore
  when the user navigates back.
- **Native static library (surface 10):** same — local execution means cache
  is a navigation optimization, not a connectivity workaround.
- **Shell + WASM (surface 11):** same.
- **Remote server-driven (surface 12):** cache is essential. Every remote
  route has a cache policy. The shell serves cached content on first paint,
  on reconnect, and on offline. The backend updates when connected.

### Flow: offline → online transition

1. User requests a remote route while offline.
2. Session resolves the route; the route handler says "network-first, fallback
   to cache."
3. Network unavailable → session queries cache → finds cached page for
   `/app/content/items`.
4. Session delivers cached page through rendering lane → WebView renders
   instantly. User sees content, may not even know they're offline.
5. If the user interacts: action is enqueued (if the user set up a queue).
6. Connectivity returns → shell detects online event.
7. Shell replays mutation queue (if any).
8. Shell fetches fresh content from remote server.
9. Shell applies update strategy: full replace or surgical diff.
10. Shell updates cache with fresh content.
11. User sees the updated page without disruption.

### Storage model

- **SQLite** — rendered pages stored as (route, timestamp, protocol_type,
  raw_bytes, version_tag). Indexed by route for O(1) lookup.
- **Version tags** — optional. The server can tag content versions. The cache
  uses tags to decide whether to serve stale content or wait for fresh.
- **Max size** — configurable. Oldest entries evicted when limit reached.
  LRU by last access time.
- **Integrity** — cached content is validated against a stored hash before
  serving. Corrupted cache entries are silently dropped and re-fetched.

### Tauri integration: bidirectional hooks

Caching is a platform capability used by every other surface. It hooks into
Tauri's filesystem, database, and lifecycle primitives.

**Shell → Tauri (what the platform wraps for caching):**

| Tauri primitive | How this surface hooks in |
|---|---|
| `AppHandle` / filesystem scope | The shell uses Tauri's scoped filesystem APIs to manage the SQLite cache database. Cache storage respects Tauri's app data directory and sandboxing. |
| Custom protocol | Cached content is served through the shell's custom protocol handler. When a route resolves to `CacheFirst` or `StaleWhileRevalidate`, the shell intercepts the custom protocol request and serves from cache instead of fetching. |
| `tauri::command` IPC | Cache management commands (invalidate, preload, stats) are exposed as Tauri commands so the WebView can trigger them. The session scope-gates them — a stale page can't invalidate the cache for a different route. |
| Event system | Cache invalidation events are emitted as Tauri events. The WebView listens for "this route has fresh content" and can trigger a re-render. |
| Secure storage | Cache integrity hashes are stored in Tauri's secure storage to prevent tampering. |

**Tauri → Shell → Cache (events triggering cache behavior):**

| Tauri callback | How it affects the cache |
|---|---|
| `setup()` | Shell initializes the cache database — creates tables, runs migrations, validates integrity. Pre-loads any bundled offline content. |
| `on_navigation()` | Every navigation triggers a cache lookup. The route's cache policy determines whether the shell serves from cache, fetches fresh, or both. The cache layer is transparent to the user's route handler — the session applies the policy. |
| Connectivity changes | Tauri fires online/offline events → shell switches cache strategy: online → can fetch fresh; offline → cache-only. Cached mutations replay when connectivity returns. |
| Mobile lifecycle | App backgrounded → shell flushes cache writes. App terminated → cache persists in SQLite. App foregrounded → shell validates cache integrity, prunes expired entries. |
| `RunEvent::Exit` | Shell closes the cache database cleanly. WAL checkpoint, integrity check, size enforcement. |

**Boundary principle:** The cache is a platform subsystem that hooks into
Tauri's storage and lifecycle primitives. It does not own state — the
backend does. It stores rendered output, indexed by route, validated by hash,
served by the custom protocol. It is a transparent layer between the session's
route resolution and the rendering lane.
