# Foundation Open Questions

## Purpose

This document lists architectural questions that should be answered before
`foundation_platform` implementation decisions are finalized. These questions are
not blockers to every small prototype, but they are blockers to a solid spec.

## 1. What is the first supported application mode?

Possible starting modes:

1. bundled local `foundation_wasm_ui` app;
2. server-rendered HTML app in Tauri;
3. hybrid local shell + remote content;
4. offline-first cached app;
5. server-driven DomOps stream.

Recommended decision: choose one primary v1 mode and treat the others as explicit
extensions. The architecture can support all, but implementation should not try
to ship all at once.

## 2. What is the route policy shape?

We need to define the route policy model:

- route matching syntax;
- source policy;
- presentation policy;
- render mode policy;
- offline/cache policy;
- capability/security policy;
- auth policy;
- whether policy can be server-provided;
- how server-provided policy is constrained by local app policy.

Key question:

> Is route policy a Rust type only, a serializable config document, or both?

## 3. What navigation semantics do we want in Tauri?

Hotwire Native maps URLs to native stacks. Tauri does not provide this out of the
box.

Questions:

- Do we model push/pop inside one WebView history?
- Do we model push/pop in Rust and drive WebView loads?
- Do modals become Tauri windows, in-DOM overlays, or platform-specific sheets?
- Do desktop and mobile share one navigation abstraction?
- Do we need screenshots/snapshots for native-feeling transitions?
- Do we preserve one WebView or create multiple WebViews?

## 4. What is the WebView profile model?

Different content needs different trust levels.

Questions:

- What profiles exist? `app`, `remoteTrusted`, `remoteUntrusted`, `auth`,
  `external`, `dev`?
- Which profiles can invoke native capabilities?
- Which profiles can access local custom protocols?
- Which profiles share cookies/storage?
- How do we configure CSP/origin behavior?
- How do we isolate cached remote HTML from privileged local app UI?

## 5. What is the custom protocol model?

Questions:

- Which protocol names/schemes do we use?
- Do we serve over custom scheme or HTTPS-like scheme?
- How are MIME types resolved?
- How are cache headers represented?
- How do we prevent arbitrary filesystem exposure?
- How do we map routes to resources?
- How do we stream large binary resources?
- How do we authenticate resource requests without exposing secrets?

## 6. How does `foundation_wasm_ui` bootstrap inside Tauri?

Questions:

- Are runtimes embedded at build time or served from app resources?
- Is application WASM embedded, loaded from disk, or fetched/cached?
- Who generates the boot HTML?
- How does dev mode differ from production mode?
- How do we version runtime JS and WASM together?
- What integrity checks are applied to remote/cached runtime assets?

## 7. Which communication lanes are v1?

Candidate lanes:

- Tauri command IPC;
- Tauri events;
- custom protocol resources;
- WebView fetch/SSE/WebSocket;
- `foundation_wasm_ui` runtime protocol;
- Rust storage/cache service;
- future native plugin/FFI lane.

Questions:

- Which are required for v1?
- Which are explicitly deferred?
- What payload sizes are allowed on command IPC?
- Which lane carries server-driven UI updates?
- Which lane carries large Arrow/data payloads?

## 8. What exactly does “Arrow for both lanes” mean?

The prior discussion expressed a desire to use Arrow broadly. We need precision.

Questions:

- Is Arrow required for UI DOM operations, or only for data payloads?
- Do we mean real Arrow IPC or the wasm-loop columnar v1 layout?
- Which crates own Arrow encoding/decoding?
- How does Arrow reach WebView JS: custom protocol, fetch, WASM memory, command
  IPC, or another lane?
- Is zero-copy required, or is copy-minimized binary transfer acceptable?
- How do small payload overheads affect control messages?

Recommended stance until decided: use existing `foundation_wasm_ui` protocols for
UI operations and real Arrow IPC for structured data payloads where it is useful.

## 9. What is the offline/cache model?

Questions:

- What gets cached: HTML, DomOps, data, route policy, WASM, JS, CSS, media?
- Is cache key route-based, URL-based, content-addressed, or hybrid?
- What invalidates cached content?
- How are stale states represented in UI?
- Are offline mutations allowed for all routes or only selected routes?
- What is the mutation queue format?
- What conflict model do we choose: last-write-wins, server reconciliation,
  event sourcing, CRDTs, app-defined?
- How does auth expiration affect cached screens?

## 10. What belongs in `foundation_platform` vs other crates?

Potential ownership boundaries:

- `foundation_platform`: Tauri host, routes, WebView profiles, protocol serving,
  bridge/capability registry.
- `foundation_wasm_ui`: UI runtime and DOM operations.
- `foundation_http`: HTTP/SSE server/client helpers.
- `foundation_db`: database/cache primitives.
- `foundation_arrow`: real Arrow IPC.
- `foundation_nativeapis`: native API wrappers.
- `foundation_packager` / build tools: packaging and asset generation.

Questions:

- Does `foundation_platform` depend directly on Tauri?
- Do we create a trait abstraction so other hosts can exist later?
- Which crate owns route policy types?
- Which crate owns capability definitions?
- Which crate owns offline sync abstractions?

## 11. What native capability contract do we want?

Questions:

- Is the bridge component model declarative in HTML attributes, custom elements,
  Rust APIs, or all of the above?
- How are permissions declared?
- How are route/session/page identities attached?
- How are progress/cancellation represented?
- How do native capability results re-enter `foundation_wasm_ui`: event, signal,
  DomOp, command response, or resource update?
- How do we test capabilities without actual devices?

## 12. How much native mobile code do we accept?

Tauri hides much but not all platform detail.

Questions:

- Are platform plugins acceptable in v1?
- Do we require Swift/Kotlin for selected capabilities?
- Do we isolate platform-specific code behind `foundation_nativeapis`?
- Is UniFFI part of v1, experimental, or out of scope?
- What is the rule for background execution APIs?

## 13. How do we handle remote UI security?

If the app can load server-rendered or server-driven UI, security policy becomes
central.

Questions:

- Can remote HTML invoke native capabilities?
- If yes, through what allowlist?
- Can server-provided route policy grant privileges, or only request privileges
  allowed by local policy?
- How are CSP and origins configured?
- How are injected scripts protected?
- How are tokens stored and scoped?
- Can remote UI load arbitrary third-party scripts?

Recommended stance: remote policy can reduce or request capabilities, but local
compiled policy must be the final authority.

## 14. What is the testing strategy?

Questions:

- How do we test route policy without launching Tauri?
- How do we test custom protocol responses?
- How do we test WebView bootstrap?
- How do we test mobile lifecycle behavior?
- How do we test offline/cache replay deterministically?
- Do we need browser/WebView integration tests in addition to Rust unit tests?
- How do we test security denials and stale page identity checks?

## 15. What is deferred explicitly?

Likely deferrals unless requirements demand otherwise:

- native SwiftUI/Compose renderer for `foundation_wasm_ui`;
- direct pointer sharing from Rust native memory to WebView JavaScript;
- UniFFI/native background lane;
- CRDT conflict model;
- multi-WebView native-stack simulation;
- app-store remote executable-code policy;
- full mobile background sync services.

## Summary

The foundations point to a strong architecture, but the spec should answer these
questions before implementation. The most important early decisions are route
policy, WebView/security profiles, custom protocol/resource serving,
communication lanes, and offline/cache ownership.
