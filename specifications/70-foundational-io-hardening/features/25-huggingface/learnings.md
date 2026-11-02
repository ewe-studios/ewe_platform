# Feature 25 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); application programming
interface (API); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## E1. The Hub client API is sync

- The async seam is `HttpClient::send_async` -> `SimpleResponse`. Use
  `into_async_parts()` to keep the body's read-wake source; `into_parts()` drops
  it and the body then falls back to a self-wake.
- `send_raw_async` returns `(Status, SimpleHeaders, AsyncSendSafeBody)`. `send_async`
  checks the status first and reads an error body only up to `max_error_body`
  with `collect_bytes_bounded`, so a 404 can never be written to disk as the
  file.
- `AsyncSendSafeBody` is `Unpin` (its fields are `Box`es and an `Arc`), so
  `futures_lite::StreamExt::next().await` works without a manual `Pin`.
- The download writes each chunk with `std::io::Write::write_all` to a `.part`
  file and renames only after every chunk is written. A dropped or failed
  download removes the `.part` file, so no truncated file can later pass as a
  valid cache entry.
- The sync boundary idiom is
  `block_on_future({ let x = x.clone(); async move { f(&x).await } })`. The
  future must own its data (`Send + 'static`), so a future that borrows a local
  cannot be blocked directly; clone the `Arc`-backed handle into the block.
- `Box<dyn std::error::Error>` is `!Send`, so a `#[valtron_test] async fn` that
  returns `Result<(), Box<dyn std::error::Error>>` fails the `block_on_future`
  `Send` bound. Return the concrete error (`HuggingFaceError`) or
  `Box<dyn std::error::Error + Send + Sync>` instead.
- The crate already had an unused `walkdir` dependency; it produced an
  `unused_dependencies` warning once the compile was run, so it was removed.

## E2. Uploads are buffered whole

- A streamed request body is `SendSafeBody::Stream(Some(Box::new(iter)))`. The
  iterator yields `Result<foundation_core::io::readers::Data, BoxedError>` where
  `BoxedError` is `foundation_core::extensions::result_ext::BoxedError`
  (`Box<dyn std::error::Error + 'static>`).
- The netio client chunk-frames every `SendSafeBody::Stream` request body: its
  `SimpleIncomingRequest::build()` calls `ensure_chunked_transfer_encoding`,
  which removes any `Content-Length` and adds `Transfer-Encoding: chunked`.
  Therefore `repo_create_commit` computes the total length up front (for callers
  and tests) and sets `Content-Length`, but the wire uses chunked framing. The
  Hub accepts this; the local test server reads both sized and chunked bodies.
- A file source is opened with `File::open` and its size read from
  `file.metadata().len()` — the same open handle — so the size and the content
  cannot race.
- The chunked file reader yields `Data::Bytes` only (never `Data::Retry`), but
  callers must still handle `Retry` because it is a valid `Data` variant.

## Dead ends

- None for E1.
- None for E2.
