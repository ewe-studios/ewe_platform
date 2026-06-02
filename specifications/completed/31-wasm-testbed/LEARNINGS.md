# Learnings: WASM Testbed

## Completed Work

- Feature 01-wasm-testbed: Crate structure created with all modules
  - All 9 source modules + 13 template files
  - Test server uses foundation_http (StaticFileHandler + HttpServer) — no external HTTP dep needed
  - Router's add_route_any takes handler directly (no ServeFactory wrapper needed)
  - EmbedDirectoryAs for template embedding (debug=disk reads, release=embedded bytes)
  - walrus for __wbgt_ test discovery, wasm-bindgen CLI for JS glue generation

## Lessons Learned

## Pending
