# Spec 52 — Decision Gaps & Concerns

Review of all 13 decisions across spec-52. Written 2026-07-17.

---

## GAP 1: `RouteDecision` is a god struct (Decision 02)

**Severity:** Medium
**Decision:** 02 — Route policy model

The `RouteDecision` struct carries 9 fields that conflate five distinct concerns:

| Field | Concern |
|---|---|
| `source`, `target` | Routing |
| `presentation`, `view_kind`, `native_view_id` | Presentation |
| `protocol` | Transport |
| `profile`, `capabilities` | Security |
| `cache_policy` | Caching |

Every route handler must know about all these concerns even when it only cares about one. A handler that just wants to say "render this in a WebView" still sees 8 irrelevant fields.

The synthesis document (`foundations/platform-synthesis.md`) already sketched a `RoutePolicy` shape with grouped sub-policies (`SourcePolicy`, `PresentationPolicy`, `SecurityPolicy`, etc.). The final `RouteDecision` struct didn't adopt this grouping.

**Resolution:** Keep the flat struct. It's data, not behavior — the builder
pattern already makes irrelevant fields invisible (`webview_app()` sets
source+protocol, user never sees `native_view_id`). Splitting into sub-structs
would add indirection without saving any actual code. If handler code starts
drowning in unrelated fields, regroup then. Status: **Accepted, no action.**

---

## GAP 2: `#[platform_bin]` source parser vs proc macro boundary (Decision 04, 13)

**Severity:** Medium
**Decisions:** 04 — Deployment surfaces, 13 — WASM app entrypoint

The spec defines `#[platform_bin]` as a source scanner (not a proc macro) that runs in `build.rs` and discovers annotations in the user's crate. But:

1. **Macro-generated annotations are invisible.** If a user writes a proc macro that expands to `#[wasm_app]`, the source parser won't find it. The spec doesn't address what happens with macro-generated annotations or whether they're even supported.

2. **Two code generation mechanisms.** `#[platform_bin]` is a source scanner, but `#[wasm_app]`, `#[platform_worker]`, `#[platform_service]`, and `#[platform_capability]` are proc macros. Users must understand two different generation pipelines that interact with each other. The source parser discovers the proc macro annotations, then the proc macros generate their own output. This is a maintenance burden and a source of subtle bugs (e.g., the proc macro generates code the source parser then tries to re-parse).

3. **Build ordering is fragile.** `build.rs` runs before compilation. If the source parser needs to see types that are defined elsewhere in the crate, it can only do textual scanning — it has no type information. This limits what the source parser can validate.

**Resolution:** Keep the source scanner. Proc macros that generate platform
annotations are theoretical in an application crate — users hand-write
`#[wasm_app]` and `#[platform_bin]` directly. The source scanner runs in
`build.rs` via `foundation_platform::generate_platform_code()`. Build
ordering is fine: the scanner only needs to see annotation text, not
resolved types. Constraint documented: macro-generated annotations are not
supported. Status: **Accepted with documented constraint.**

---

## GAP 3: Deployment surface v1 scope undefined (Decision 04)

**Severity:** Medium
**Decision:** 04 — Deployment surfaces

All five deployment surfaces are described as resolved, but no prioritization for v1 vs v2 exists. Each surface has its own build pipeline, transport wiring, update mechanism, and testing requirements. Decision 10 explicitly scopes its multi-WebView model (single-WebView = v1, multi-WebView = v2), but Decision 04 does not.

**Resolution:** MVP ships the core web-content surfaces first. The rest are
post-MVP layers built after the core rendering path is stable.

| Surface | Priority | Rationale |
|---|---|---|
| 1 — Bundled WASM in WebView | **MVP core** | Offline-first default, foundational |
| 4 — Remote server-driven | **MVP core** | Basecamp analogue, essential for server-driven apps |
| 5 — Cached offline replay | **MVP core** | Natural extension of surface 4, enables offline |
| 3 — Shell + WASM runtime (`#[wasm_app]`) | **Post-MVP layer** | Build pipeline defined (decision 13), ship after core surfaces stable |
| 2 — Native static library | **Post-MVP layer** | App-store friction, niche use case |

Decision 04 updated with MVP scope annotations.

---

## GAP 4: `ewe://` sub-scheme registration (Decision 03) — RESOLVED

**Severity:** Medium-High (was)
**Decision:** 03 — Session backbone transport

**Resolution:** Removed all sub-schemes. One `ewe://` scheme, registered once.
Transport is implied by `RouteSource` — the session selects it automatically
from the `RouteDecision`. The transport modes table now maps `RouteSource`
variants to transports, not URL schemes to transports. Decisions 02 and 03
updated.

---

## GAP 5: Three capability API surfaces may be overkill (Decision 07)

**Severity:** Low
**Decision:** 07 — Native capability contract

The spec provides three capability API surfaces (trait impl, `#[platform_capability]` proc macro, registry builder) — the same pattern as route handlers. But routing logic varies wildly (database queries vs simple pattern matching), justifying three surfaces. Capabilities are more uniform — they almost always wrap a native API call with a request/response shape.

**Question:** Do users actually implement the `Capability` trait directly often enough to justify it? If 95% of capabilities use the proc macro, the trait is dead abstraction weight.

**Resolution: Accepted, no action.** The three-surface pattern is consistent
with route handlers and costs nothing to declare. The trait impl surface exists
in the type system but the proc macro covers the common case. Users discover
the trait only when they need it. Status: **No change needed.**

---

## GAP 6: `src/generated/` checked into git (Decision 13)

**Severity:** Medium
**Decision:** 13 — WASM app entrypoint

The spec says generated code in `src/generated/` is "checked into git so CI and other developers don't need to run the build to see the generated code." This is a known failure mode:

1. Developer A changes a `#[wasm_app]` function, commits, but the generated `src/generated/` doesn't match the source.
2. CI passes (it regenerates).
3. Developer B pulls stale generated code.
4. Developer C runs `cargo build`, regenerates, now has different generated code than what's in git.

**Resolution:** Commit generated code AND add a CI check. Generated code has value — it's readable, reviewable, and can be modified by hand when needed (e.g. adding custom imports to a generated wrapper). CI runs `cargo build` (which triggers `build.rs` → `generate_platform_code()` to regenerate) and then `git diff --exit-code src/generated/`. If generated code doesn't match what's committed, CI fails — the developer forgot to commit updated generated code. This eliminates the stale-code problem while keeping the benefits of versioned generated files.

---

## GAP 7: Mutation queue conflict resolution underspecified (Decision 12)

**Severity:** Medium
**Decision:** 12 — Mutation queue and conflict resolution

The platform provides "version vectors, timestamp comparison, LWW default" and the user defines custom strategies via `#[mutation_conflict]`. But:

1. **Routing:** How does the platform know which conflict resolver applies to which mutation? The `#[mutation_conflict]` macro is shown but its matching mechanism is unspecified.
2. **Client-side races:** What happens when two conflicting mutations from the same client race (e.g., two concurrent edits to the same entity)?
3. **User resolution in background:** The "ask user" strategy requires UI — how does that work from a background worker ([decision 11])? The mutation queue is replayed by a worker, not by the UI thread.
4. **Idempotency contract:** The spec says "the server deduplicates by UUID" but doesn't specify the wire format for the UUID (header? body field?). If the server doesn't implement this, replay silently double-applies mutations.

**Resolution: Deferred post-MVP.** The mutation queue with LWW default is
sufficient for MVP. Detailed conflict resolution (registered resolvers by
mutation type, client-side race handling, "ask user" from background
workers, UUID wire format) is specified in decision 12 but implementation
is post-MVP. For MVP, last-write-wins covers the common case.

---

## GAP 8: `foundation_wasmtime` missing from Decision 01 dependency graph

**Severity:** Low
**Decisions:** 01 — Platform architecture, 13 — WASM app entrypoint

Decision 01's dependency graph lists `foundation_platform`'s dependencies but doesn't include `foundation_wasmtime`, which Decision 13 introduces as a new crate. The generated code uses `WasmtimeBuilder` from `foundation_wasmtime`, and the session registry stores `WasmtimeInstance`.

**Resolution: Fixed.** `foundation_wasmtime` and `foundation_packager` added
to Decision 01's dependency graph, Cargo.toml deps, and crate descriptions.
Decision 01 updated.

---

## GAP 9: `#[platform_background]` annotation proliferation (Decision 11)

**Severity:** Low
**Decision:** 11 — Background workers

The spec shows four distinct `#[platform_background]` variants:

- `#[platform_background(fetch)]` — iOS BGAppRefreshTask
- `#[platform_background(periodic)]` — Android WorkManager
- `#[platform_background(push)]` — Push notification handler
- `#[platform_background(download)]` — iOS Background URLSession

Combined with `#[platform_bin]`, `#[platform_worker]`, `#[platform_service]`, and `#[platform_capability]`, the annotation namespace is growing. Each variant generates different platform glue (BGTaskScheduler, WorkManager, push handlers, URLSession).

**Resolution: Accepted, no action.** The current annotations mirror the OS APIs
they wrap — `#[platform_background(fetch)]` reads as "background fetch handler."
Consolidating under `#[platform_worker(mode = ...)]` would trade enum parameters
for distinct annotations without reducing complexity. Keep as-is.

---

## GAP 10: Native view instantiation via Tauri escape hatches (Decision 02)

**Severity:** Low-Medium
**Decision:** 02 — Route policy model

Decision 02 designs native view support (SwiftUI/Jetpack Compose) using Tauri's escape hatches (`run_on_main_thread`, `run_on_android_context`). This is sound in principle but:

1. **No Tauri native view abstraction exists.** The platform builds it from scratch using thread hooks, JNI, and Swift FFI. This is a large surface area that the spec doesn't fully specify (e.g., how does a SwiftUI view communicate back to the session? How is the native view lifecycle managed when the app backgrounds?).

2. **Swift/Kotlin code generation unspecified.** The spec shows `register_native_view("settings", |intent, shell| { ... })` but doesn't address how the Swift/Kotlin native view components are generated, compiled, and linked into the Tauri app.

**Resolution: Post-MVP.** Native view support via Tauri escape hatches is
post-MVP. The `ViewKind::Native` variant exists now so the type system doesn't
change later, but the native view registry, Swift/Kotlin build pipeline, and
instantiation code ship after the WebView rendering path is stable. MVP renders
everything in WebViews. Decision 02 updated.

---

## GAP 11: State ownership contradiction (Decisions 03, 05, synthesis)

**Severity:** Medium
**Decisions:** 03 — Session backbone, 05 — Offline model, platform-synthesis.md

Decision 03 and the synthesis doc state: "The server owns all state. The platform does not own state machines, conflict resolution, or sync protocols." But Decision 05 defines a two-tier offline model where the platform caches rendered pages and Decision 12 defines a mutation queue with conflict resolution primitives.

The platform says it doesn't own state but provides the storage, replay, and conflict detection infrastructure. The line between "infrastructure" and "state ownership" is blurry.

**Resolution: Fixed.** Added explicit "Platform provides mechanisms, app
provides policy" table to Decision 03's State Ownership section. The boundary
is now documented: cache storage vs cache policy, queue storage vs conflict
resolution, routing infrastructure vs route definitions. Decision 03 updated.

---

## GAP 12: No migration path from existing Tauri apps (Decision 01)

**Severity:** Low
**Decision:** 01 — Platform architecture

The spec describes a greenfield `foundation_platform` crate but doesn't address how an existing Tauri app migrates to it. The `PlatformBuilder` wraps `tauri::Builder`, but what happens to existing Tauri `#[tauri::command]` handlers, plugin registrations, and window configurations?

**Resolution: Deferred.** Migration path from existing Tauri apps is not needed
for MVP — the first apps are greenfield. `PlatformBuilder` wraps
`tauri::Builder`, so existing Tauri commands and plugins can be incrementally
migrated as apps adopt `foundation_platform`. Document when needed. Status:
**No action for MVP.**

---

## Summary by resolution

| Gap | Severity | Resolution |
|---|---|---|
| GAP 1: God struct | Medium | Accepted, no action — builder pattern handles it |
| GAP 2: Source parser vs proc macro | Medium | Accepted with constraint — macro-generated annotations not supported |
| GAP 3: v1 scope | Medium | **Fixed** — MVP core surfaces defined, post-MVP layers marked |
| GAP 4: ewe:// sub-schemes | Medium-High | **Fixed** — single ewe:// scheme, transport implied by RouteSource |
| GAP 5: Capability API surfaces | Low | Accepted, no action — consistent with route handler pattern |
| GAP 6: Generated code in git | Medium | **Fixed** — CI check enforced |
| GAP 7: Conflict resolution | Medium | **Deferred post-MVP** — LWW default sufficient for MVP |
| GAP 8: Missing dep graph entry | Low | **Fixed** — foundation_wasmtime + foundation_packager added |
| GAP 9: Annotation proliferation | Low | Accepted, no action — mirrors OS APIs |
| GAP 10: Native view instantiation | Low-Medium | **Post-MVP** — ViewKind::Native type exists, impl deferred |
| GAP 11: State ownership | Medium | **Fixed** — mechanisms vs policy table added to decision 03 |
| GAP 12: Migration path | Low | Deferred — greenfield for MVP, migration path when needed |
