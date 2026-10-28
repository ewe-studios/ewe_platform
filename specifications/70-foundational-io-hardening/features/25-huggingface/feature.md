---
feature: "foundation_deployment_huggingface: async Hub client, streamed multipart uploads, Link pagination"
status: "pending"
priority: "medium"
depends_on: ["03-client-body-readiness", "05-push-body-and-accept"]
branch: "fix/io-25-huggingface"
crates: ["foundation_deployment_huggingface", "foundation_ai", "foundation_testing"]
backlog: "docs/bug-backlog.md §20 (3 entries)"
---

# Feature 25: `foundation_deployment_huggingface`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); application programming
interface (API); WebAssembly (wasm); pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry.

## T0. Check that every entry is still open (table in the PR body)

## E1. The Hub client API is sync

- **Where:** `client::{whoami, list_*, create_repo, …}` and `repository::*` (`src/client.rs`,
  `src/repository.rs`) block on `HttpClient::send`.
- **Fix:** `async fn`s over `send_async`; downloads stream to the destination
  (`AsyncSendSafeBody`, chunk by chunk; not `collect_bytes_async` for files). Callers
  (`foundation_ai`'s candle and gguf providers, `foundation_testing`) `.block()` only at real
  boundaries. Remove the sync functions (no twins).
- **Tests:** `hub_whoami_awaits_without_blocking` (a ticker keeps ticking while the mock server
  waits 300 ms); `hub_download_streams_to_file` (a 64 MiB file; peak buffer under the chunk size).

## E2. Uploads are buffered whole

- **Where:** `repo_create_commit` reads each file with `std::fs::read` and builds the whole
  multipart body.
- **Fix:** a streamed multipart body (`SendSafeBody::Stream` over file chunks, feature 05's push
  body), with the length computed up front from the file sizes and the part headers.
- **Test:** `hub_commit_streams_multipart` (a 64 MiB file; the mock server checks the body is
  correct and `Content-Length` matches; a counting reader shows no read larger than the chunk size).

## E3. `list_*` and `repo_list_tree` read one page only

- **Fix:** parse `Link: <…>; rel="next"` and fetch the next page lazily when the stream is pulled
  past the current page. A page limit on config guards against loops (default 10 000 pages).
- **Tests:** `hub_list_follows_link_pages` (3 pages; all items; page 2 requested only after page
  1's items are pulled); `hub_list_stops_without_next_link`.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_deployment_huggingface foundation_ai foundation_testing; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] No sync Hub function is left; `rg -n 'fn \w+\(' src | rg -v async` shows only helpers.
- [ ] CI green.
