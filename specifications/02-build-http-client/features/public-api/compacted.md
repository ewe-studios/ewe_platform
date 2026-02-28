# Compacted Context: Public API Feature

⚠️TEMP_FILE|DELETE_WHEN_DONE|GENERATED:2026-02-28T00:00Z

## LOCATION
workspace:ewe_platform|spec:02-build-http-client|file:specifications/02-build-http-client/features/public-api/start.md

## OBJECTIVE
Complete & verify HTTP client public API (one item at a time, TDD)

## REQUIREMENTS
req1:Expose clean SimpleHttpClient and ClientRequest API|hide:TaskIterator,internal machinery
req2:Support get/post/put/delete/patch/head/options|builder for config|optional:connection pooling|reuse existing types (SimpleHeaders, ResponseIntro, etc.)
req3:Public API, methods, and success mapped to feature.md spec|see:Design Principles table|code must pass all format/lint/tests|feature flags in Cargo
success:[ClientRequest methods work, HTTP/HTTPS, code quality checks, tests pass; see feature.md success table]

## TASKS
[ ]task:Create missing http_redirect_integration.rs test|files:[tests/backends/foundation_core/integrations/simple_http/http_redirect_integration.rs]|tests:[redirect chains, progressive read, one-shot, streaming]
[ ]task:Run full verification suite|files:[N/A]|tests:[cargo fmt, clippy, all tests]
[ ]task:Verify feature checklist in feature.md updated

## LEARNINGS
past1:Core logic should reuse existing types; don't re-implement basics (LEARNINGS.md)
past2:Integration tests should avoid duplicating mocks, use local servers; connection pool requires real concurrency (LEARNINGS.md)
past3:Edge-case: sensitive headers must be stripped on host change during redirects; POST→GET redirect logic needs explicit test (PROGRESS.md/LEARNINGS.md)

## CURRENT_STATE
progress:Core ClientRequest, redirect logic, build/testing pass, implementation done|next:Add missing integration test module|blockers:integration test for http_redirect_integration.rs not present, verification pending

## FILES_TO_MODIFY
read:[client/api.rs,client/client.rs,client/pool.rs,simple_http/mod.rs,simple_http/impls.rs,tests/backends/foundation_core/integrations/simple_http/]|update:[client/api.rs,client/client.rs,client/pool.rs,simple_http/mod.rs,feature.md,PROGRESS.md]|create:[tests/backends/foundation_core/integrations/simple_http/http_redirect_integration.rs]

## NEXT_ACTIONS
1. Create tests/backends/foundation_core/integrations/simple_http/http_redirect_integration.rs with redirect chain/edge tests
2. Run full verification suite (fmt, clippy, all tests)
3. Update PROGRESS.md and feature.md with actual completion status

---

⚠️ **AFTER READING**: Clear context, reload from this file only, start work
