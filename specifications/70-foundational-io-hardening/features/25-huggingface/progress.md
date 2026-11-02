# Feature 25 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); application programming
interface (API); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## Landed

- E1 (the Hub client API is sync):
  - `foundation_deployment_huggingface` turns
    `client::{whoami, auth_check, list_models, list_datasets, list_spaces,
    create_repo, delete_repo, move_repo}` and
    `repository::{repo_info, repo_exists, repo_revision_exists, repo_list_tree,
    repo_download_file, repo_upload_file, repo_delete_file, repo_create_commit}`
    into `async fn`s over `send_async`/`send_raw_async`. The sync `send`,
    `send_raw`, `send_json` helpers and the `LimitedWriter` struct are gone (no
    sync twins). (`src/client.rs`, `src/repository.rs`.)
  - `repo_download_file` streams the body chunk by chunk to a `.part` file with
    `AsyncSendSafeBody` + `futures_lite::StreamExt::next`, then renames it into
    place. It never calls `collect_bytes_async` for files. An error answer is
    read only up to `max_error_body` via `collect_bytes_bounded`.
  - Version bump: `foundation_deployment_huggingface` 0.1.2 -> 0.1.3.
    `foundation_ai` and `foundation_testing` require 0.1.3 in the same change.
  - Callers keep their sync signatures and wrap the async Hub calls at the
    boundary with `foundation_core::valtron::block_on_future`. `foundation_ai`'s
    candle and gguf providers and `foundation_testing::huggingface::TestHarness`
    now drive `repo_download_file`/`repo_list_tree` that way. The
    `ModelProvider` trait stays sync.
  - `foundation_testing`'s inline `#[cfg(test)]` tests moved to
    `tests/huggingface_tests.rs` (gated on the `huggingface` feature). The
    `TestHarness` gains a public `artefacts_dir` accessor so the moved test can
    reach that field.
  - Tests (all `#[valtron_test] async fn`):
    `hub_whoami_awaits_without_blocking` (a 300 ms server delay; a ticker
    zipped with the await keeps ticking) and `hub_download_streams_to_file` (a
    64 KiB body round-trips through the streamed path). The older offline and
    ignored integration tests are converted to the same async form.
  - Backlog 20 "The Hub client API is sync." entry deleted.

## In progress

- E2 (streamed multipart upload body) and E3 (Link pagination).

## Done

- E1.
