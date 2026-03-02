# Learnings: 02-build-http-client

## Critical Implementation Details
- Do NOT use async/await, tokio, or async runtimes for this client code or testing. Follow Rust Clean Code guidance for synchronous implementations only (see .agents/skills/rust-clean-code/implementation/skill.md).
- Always prefer project and stdlib building blocks before adding any external crate.

## Common Failures and Fixes
- Including async or tokio primitives leads to spec non-compliance; remove and refactor to sync patterns if found.
- Forgetting panics documentation: all public functions must specify # Panics if needed.
- Trying to enable a nonexistent “sync” feature or run with --features sync is incorrect; project is strictly synchronous by default, no gate or feature toggle. Synchronous is the enforced norm.
- The foundational Cargo feature in this project is “std.” If you need to pass a --features flag, it should be --features std, not sync.

## Testing Insights
- TestHttpServer::redirect_chain helper makes writing sequential redirect tests much simpler and reusable. Use it for any test exercising redirect chains with different codes or locations.
- Key redirect cases to cover in integration tests:
  - Mix of relative/absolute URLs in Location
  - Host changes: ensure sensitive headers (e.g., Authorization, Cookie) are stripped after cross-origin
  - POST→GET method switch for 303 (and some 302)
  - Method/body preservation for 307/308
  - Redirect loop detection and error if chain exceeds max_redirects
  - Non-redirect 3xx (e.g., 305, 306, 304) should NOT trigger another request
  - Edge cases: empty Location, invalid schemes, large chains, chunked encoding

- This project uses a dedicated test crate: ewe_platform_tests (defined in /tests/Cargo.toml). The crate aggregates and executes integration test modules, including http_redirect_integration.rs, from the /tests hierarchy.
- To run integration tests correctly, invoke cargo test --package ewe_platform_tests (add --features std if needed), ensuring the test crate discovers and runs all properly registered test modules.
- Test module discovery relies on mod.rs files registering test modules within the crate hierarchy. Verify mod.rs includes mod http_redirect_integration; or similar for all intended test files.
- Integration tests should not rely on async test setups. Use only synchronous test helpers from test infrastructure.
- Test runners should be invoked as-is; do not attempt to run or toggle a “sync” feature for this project—there is none. Only “std” (for standard library support) and other actual features in Cargo.toml are permitted.

- Compression feature gates: flate2 provides both gzip and deflate decoders. Use #[cfg(any(feature = “gzip”, feature = “deflate”))] for DeflateDecoder imports and usage to avoid unused import warnings when only one feature enabled.
- Brotli decompressor needs buffer size parameter (4096 works well): BrotliDecoder::new(inner, 4096)

## Dependencies and Interactions
- This project is strictly synchronous per stack standards; async/await dependencies must be disallowed at review.

## Future Considerations
- If async HTTP support is needed, a separate feature and architectural track must be proposed and accepted before any work.

---

*2026-02-28: Added a reminder from .agents/skills/rust-clean-code/implementation/skill.md—SYNC ONLY: Do not use async/await or tokio in client or test code for this spec. All patterns must follow synchronous design and Rust Clean Code rules only.*

*2026-02-28: “sync” is not a Cargo feature for this project. The codebase is synchronous by design and standards; never attempt to toggle or test with a sync feature. All code, tests, and runners assume sync-by-default.*

*2026-02-28: The correct Cargo feature to pass (where needed) is “std” for standard library support, not “sync.”*
