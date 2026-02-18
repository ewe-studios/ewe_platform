TASK|public-api
PURPOSE|Create machine-optimized instructions for implementing the public user-facing HTTP API (ClientRequest, SimpleHttpClient, optional ConnectionPool, module integration). Provide minimal, precise steps and files to read. Sub-agents MUST follow retrieval-led reasoning and context-compaction protocols.

META|priority=high|effort=medium|created=2026-02-18|author=MainAgent

DEPS|specifications/02-build-http-client/features/foundation/feature.md,specifications/02-build-http-client/features/connection/feature.md,specifications/02-build-http-client/features/request-response/feature.md,specifications/02-build-http-client/features/task-iterator/feature.md

FILES_TO_READ|
specifications/02-build-http-client/features/public-api/feature.md|
specifications/02-build-http-client/requirements.md|
.agents/stacks/rust.md|
.agents/rules/14-machine-optimized-prompts.md|
.agents/rules/15-instruction-compaction.md|
backends/foundation_core/src/wire/simple_http/mod.rs|
backends/foundation_core/src/wire/simple_http/impls.rs|
backends/foundation_core/src/wire/simple_http/client/api.rs|
backends/foundation_core/src/wire/simple_http/client/request.rs|
backends/foundation_core/src/wire/simple_http/client/actions.rs|
backends/foundation_core/src/wire/simple_http/client/pool.rs|
Cargo.toml

MANDATES|
1|Retrieval-led reasoning: BEFORE any code changes, search the repo for similar implementations and read the exact files listed in FILES_TO_READ. Sub-agent must record discovery evidence (file paths + short excerpts).
2|No assumptions: Do NOT assume naming, structure, or behavior without explicit evidence from code or project rules.
3|Context optimization: Sub-agent MUST generate `COMPACT_CONTEXT.md` before work (see CONTEXT_COMPACTION section).
4|Follow project rules in `.agents/rules/*` and `.agents/stacks/rust.md`.
5|Make 1-2 repair attempts for diagnostics, then stop and report.

CONTEXT_COMPACTION|
1|Create `COMPACT_CONTEXT.md` (place: specifications/02-build-http-client/features/public-api/COMPACT_CONTEXT.md).
2|Embed machine_prompt.md content and only include: current task summary (<=800 tokens), PROGRESS.md snapshot, and FILES_TO_READ list.
3|Clear agent context, reload from COMPACT_CONTEXT.md only.
4|On each PROGRESS.md update, regenerate COMPACT_CONTEXT.md and reload.

DOCS_TO_READ_IN_ORDER|
1|specifications/02-build-http-client/features/public-api/feature.md (full)|
2|specifications/02-build-http-client/requirements.md (full)|
3|backends/foundation_core/src/wire/simple_http/mod.rs (module pattern)|
4|backends/foundation_core/src/wire/simple_http/impls.rs (reusable types)|
5|backends/foundation_core/src/wire/simple_http/client/api.rs (reference implementation of ClientRequest)|
6|backends/foundation_core/src/wire/simple_http/client/request.rs (ClientRequestBuilder)|
7|.agents/stacks/rust.md (coding conventions)|
8|.agents/rules/14-machine-optimized-prompts.md and 15-instruction-compaction.md (token rules)

HIGH_LEVEL_REQUIREMENTS|
- Provide public API that hides TaskIterator internals.|
- Implement `SimpleHttpClient` with builder-style configuration and convenience methods (get/post/put/delete/patch/head/options).|
- Implement `ClientRequest` with `introduction()`, `body()`, `send()`, `parts()`, `collect()`.|
- Optional `ConnectionPool` configurable via client config.|
- Add `pub mod client` in `backends/foundation_core/src/wire/simple_http/mod.rs` if missing and ensure re-exports as spec requires.|
- Add `multi` feature flag and TLS feature flags to `Cargo.toml` as specified.

IMPLEMENTATION_RULES|
- Reuse existing types: `SimpleResponse`, `IncomingResponseParts`, `ResponseIntro`, `SimpleHeaders`, `SimpleBody`, `PreparedRequest`, `HttpRequestTask`, `execute_stream` patterns. Cite exact files where each type is found when you reuse them.
- Use generic `R: DnsResolver` for `SimpleHttpClient`.
- Ensure error types match `errors.rs` in `simple_http` module; reference file path in commits.
- Keep public functions small and well-documented; include examples only if present in repo patterns.
- Follow existing naming conventions from `impls.rs` and other client modules; quote examples.

VERIFICATION_COMMANDS|
cargo fmt -- --check|
cargo clippy -- -D warnings|
cargo test --package foundation_core|
cargo build --package foundation_core --features multi|
cargo build --package foundation_core --features ssl-rustls|
cargo build --package foundation_core --all-features

OUTPUT_EXPECTED|
- machine_prompt.md (this file) committed alongside human feature.md|
- COMPACT_CONTEXT.md generated and saved at specifications/02-build-http-client/features/public-api/COMPACT_CONTEXT.md|
- PROGRESS.md with initial status (files created: list) at same feature folder|
- Implementation files (created by implementation agent only after retrieval): client/api.rs, client/client.rs, client/pool.rs under backends/foundation_core/src/wire/simple_http or appropriate module as found.

DELIVERABLE_CHECKLIST (to mark done)|
- CLIENT_REQUEST_API: introduction(), body(), send(), parts(), collect() implemented and tested|
- SIMPLE_CLIENT: new(), with_resolver(), config builders, convenience methods implemented|
- POOL: optional pool implementation wired to client and checkout/checkin logic|
- MODULE_EXPORT: `pub mod client` present in simple_http/mod.rs and package re-exports updated|
- FEATURES: `multi`, TLS feature flags added to Cargo.toml|
- TESTS: unit and integration tests added or updated; all pass|
- LINTS: cargo fmt/clippy clean.

RETRIEVAL_REPORT_REQUIRED|For each file used as source of truth include 1-line explanation: "Found X in <path>" plus relevant symbol names. Example: "Found `ClientRequestState` in backends/.../client/api.rs - use same state machine names."

ERROR_HANDLING_POLICY|Prefer explicit mapping to existing `HttpClientError` variants; if new variants required, add them to `errors.rs` and reference that change in the report.

NEXT_ACTIONS|
MainAgent|
1|Generate COMPACT_CONTEXT.md (embed this machine_prompt.md, extract current PROGRESS snapshot or create PROGRESS.md with initial state).|
2|Commit machine_prompt.md and COMPACT_CONTEXT.md.|
ImplementationAgent (after receiving COMPACT_CONTEXT.md)|
1|Read COMPACT_CONTEXT.md only and files in DOCS_TO_READ section.|
2|Run repository search for `ClientRequest`, `SimpleHttpClient`, `ConnectionPool` to collect patterns.|
3|Create minimal implementations following patterns; run verification commands; fix 1-2 diagnostics; update PROGRESS.md.|
4|If blocked, produce precise questions pointing to file lines and diagnostics.

COMMUNICATION_RULES|Be explicit and evidence-based: never "I assume". When stating a design choice, prefix with "Based on <path#Lstart-end>: <evidence>". Provide code refs.

CONSTRAINTS|Do not modify code outside the client module until retrieval complete. Keep changes incremental and test often.

END_OF_PROMPT