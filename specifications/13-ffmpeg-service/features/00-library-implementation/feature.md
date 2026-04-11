---
feature: "Library Implementation"
description: "Core ffmpeg-converter crate: FileStore trait, OpusConverter struct, ConversionFormat, and upload support for local FS, S3, and GCS — using ac-ffmpeg for zero-disk streaming I/O"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 00: Library Implementation

## Goal

Implement the `ffmpeg-converter` library crate (`packages/ffmpeg-converter`) that provides:

1. A `FileStore` trait with `get_stream` (download) and `upload` (upload) methods
2. Three `FileStore` implementations: `LocalFileStore`, `S3FileStore`, `GcsFileStore`
3. A `ConversionFormat` enum listing all supported output formats
4. An `OpusConverter` struct that is a **pure stream transformer** — it knows nothing about `FileStore`, paths, or storage backends

`OpusConverter::convert` accepts a `source: impl io::Read + Send` and an `output: impl io::Write + Send` directly. The caller is responsible for obtaining these from `FileStore` (or anywhere else) and wiring them in. `FileStore::upload` returns an associated `Writer: io::Write + Send` type specific to each implementation, which is passed directly to `convert`. Nothing accumulates in memory; the encoded bytes flow straight from the muxer into the writer.

---

## Module Layout

```
packages/ffmpeg-converter/
├── Cargo.toml
└── src/
    ├── lib.rs           # re-exports: FileStore, OpusConverter, ConversionFormat, ConverterError
    ├── error.rs         # ConverterError — derive_more + serde (JSON-serializable, location-traced)
    ├── file_store.rs    # FileStore trait + LocalFileStore, S3FileStore, GcsFileStore
    └── converter.rs     # OpusConverter + ConversionFormat
```

---

## Task Breakdown

- [ ] **T01** — Update `packages/ffmpeg-converter/Cargo.toml` with required dependencies:
  - `ac-ffmpeg = "0.18"` — safe Rust wrapper over FFmpeg's `libav*`; replaces raw `ffmpeg-sys-next` bindings and eliminates the temp-file round-trip
  - `aws-sdk-s3 = "1"` (for S3FileStore)
  - `google-cloud-storage = "0.20"` (for GcsFileStore)
  - `tokio = { version = "1", features = ["full"] }` (async runtime for cloud SDKs)
  - `derive_more = { version = "1", features = ["display", "error", "from"] }` (error derives + Display)
  - `serde = { version = "1", features = ["derive"] }` (JSON-serializable errors)
  - `serde_json = "1"`
  - `error-stack = "0.5"` (location-aware error traces: file, line, column captured automatically)
  - `tracing = "0.1"` — structured instrumentation; the library only emits events, binaries configure the subscriber

  > Do **not** add `tempfile` — the streaming pipeline never materialises audio on disk.

- [ ] **T02** — Define `ConversionFormat` in `converter.rs`:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub enum ConversionFormat {
      Wave,
      MP4,
      // extendable
  }

  impl ConversionFormat {
      /// File extension used when naming output objects.
      pub fn extension(&self) -> &str {
          match self {
              Self::MP4  => "mp4",
              Self::Wave => "wav",
          }
      }

      /// Returns the (codec_name, container_name) pair expected by ac-ffmpeg's
      /// AudioEncoder::builder and OutputFormat::find_by_name.
      pub fn ac_ffmpeg_names(&self) -> (&'static str, &'static str) {
          match self {
              Self::MP4  => ("aac",       "mp4"),
              Self::Wave => ("pcm_s16le", "wav"),
          }
      }
  }
  ```

- [ ] **T03** — Define `FileStore` trait in `file_store.rs`:
  ```rust
  pub trait FileStore {
      /// Concrete writer type returned by upload(). Each impl owns its type.
      type Writer: io::Write + Send;

      fn get_stream(&self, path: &str)
          -> error_stack::Result<Box<dyn io::Read + Send>, ConverterError>;

      /// Opens an upload session to dest_path and returns a writer.
      /// The caller writes encoded bytes into it; dropping or flushing finalises the upload.
      fn upload(&self, dest_path: &str)
          -> error_stack::Result<Self::Writer, ConverterError>;
  }
  ```

- [ ] **T04** — Implement `LocalFileStore`:
  - `Writer = BufWriter<File>`
  - `get_stream`: `File::open(path)` wrapped in `Box<dyn io::Read + Send>`
  - `upload`: open `dest_path` for writing, return `BufWriter<File>`

- [ ] **T05** — Implement `S3FileStore`:
  - `Writer = S3UploadWriter` — a struct that buffers chunks internally and flushes parts to S3 multipart upload; `Drop` completes the upload
  - `get_stream`: call `get_object`, stream body as `io::Read`
  - `upload`: initiate a multipart upload, return `S3UploadWriter`

- [ ] **T06** — Implement `GcsFileStore`:
  - `Writer = GcsUploadWriter` — a struct that streams bytes to a GCS resumable upload session; `Drop` finalises it
  - `get_stream`: download object as stream
  - `upload`: open a resumable upload session, return `GcsUploadWriter`

- [ ] **T07** — Implement `OpusConverter` in `converter.rs` using the ac-ffmpeg streaming pipeline:

  `OpusConverter` has no generic parameters and no fields — it holds no state. The caller obtains
  a source stream and an output writer independently (e.g. from `FileStore`) and passes them in.

  ```rust
  use ac_ffmpeg::{
      codec::audio::{AudioDecoder, AudioEncoder, SampleFormat},
      format::{demuxer::Demuxer, muxer::{Muxer, MuxerOptions, OutputFormat}},
      io::IO,
  };

  pub struct OpusConverter;

  impl OpusConverter {
      pub fn convert(
          source: impl io::Read + Send,
          output: impl io::Write + Send,
          format: ConversionFormat,
      ) -> error_stack::Result<(), ConverterError> {

          // 1. Wrap source stream in AVIOContext — no disk write
          let io_in = IO::from_read_stream(source);

          // 2. Demux + probe
          let mut demuxer = Demuxer::builder()
              .build(io_in)
              .and_then(|d| d.find_stream_info(None).map_err(|(_, e)| e))
              .change_context(ConverterError::EncodingFailed { exit_code: -1 })
              .attach_printable("ffmpeg failed to probe stream info")?;

          let (stream_index, audio_stream) = demuxer
              .streams()
              .iter()
              .enumerate()
              .find(|(_, s)| s.codec_parameters().is_audio_codec())
              .ok_or_else(|| error_stack::report!(ConverterError::EncodingFailed { exit_code: -2 }))
              .attach_printable("no audio stream found in input")?;

          let mut decoder = AudioDecoder::from_stream(audio_stream)
              .and_then(|b| b.build())
              .change_context(ConverterError::EncodingFailed { exit_code: -3 })?;

          // 3. Build encoder + muxer for target format
          let (codec_name, container_name) = format.ac_ffmpeg_names();
          let codec_params = audio_stream.codec_parameters();

          let mut encoder = AudioEncoder::builder(codec_name)
              .and_then(|b| {
                  b.sample_rate(codec_params.sample_rate())?
                   .channel_layout(codec_params.channel_layout().clone())?
                   .sample_format(SampleFormat::find_by_name("fltp").unwrap())?
                   .build()
              })
              .change_context(ConverterError::EncodingFailed { exit_code: -4 })
              .attach_printable_lazy(|| format!("codec: {codec_name}"))?;

          let output_format = OutputFormat::find_by_name(container_name)
              .ok_or_else(|| error_stack::report!(ConverterError::UnsupportedFormat {
                  format: format.extension().into(),
              }))?;

          // 4. Wrap output writer in AVIOContext — muxer writes encoded bytes directly into it
          let io_out = IO::from_write_stream(output);

          let mut muxer = Muxer::builder()
              .add_stream(encoder.codec_parameters())
              .and_then(|b| b.build(io_out, output_format))
              .change_context(ConverterError::EncodingFailed { exit_code: -5 })?;

          // For MP4: fragmented mode so the muxer never seeks back to rewrite the moov atom.
          // WAV is sequential and needs no special flags.
          let mut header_opts = MuxerOptions::new();
          if matches!(format, ConversionFormat::MP4) {
              header_opts.set("movflags", "frag_keyframe+empty_moov")
                  .change_context(ConverterError::EncodingFailed { exit_code: -6 })?;
          }
          muxer.write_header(Some(&header_opts))
              .change_context(ConverterError::EncodingFailed { exit_code: -7 })?;

          // 5. Transcode loop: demux → decode → encode → mux
          while let Some(packet) = demuxer.take()
              .change_context(ConverterError::EncodingFailed { exit_code: -8 })?
          {
              if packet.stream_index() != stream_index { continue; }
              decoder.push(packet)
                  .change_context(ConverterError::EncodingFailed { exit_code: -9 })?;
              while let Some(frame) = decoder.take()
                  .change_context(ConverterError::EncodingFailed { exit_code: -10 })?
              {
                  encoder.push(frame)
                      .change_context(ConverterError::EncodingFailed { exit_code: -11 })?;
                  while let Some(pkt) = encoder.take()
                      .change_context(ConverterError::EncodingFailed { exit_code: -12 })?
                  {
                      muxer.write(pkt)
                          .change_context(ConverterError::EncodingFailed { exit_code: -13 })?;
                  }
              }
          }

          // 6. Flush decoder → encoder → muxer
          decoder.flush().ok();
          while let Some(frame) = decoder.take().ok().flatten() {
              encoder.push(frame).ok();
              while let Some(pkt) = encoder.take().ok().flatten() { muxer.write(pkt).ok(); }
          }
          encoder.flush().ok();
          while let Some(pkt) = encoder.take().ok().flatten() { muxer.write(pkt).ok(); }

          muxer.write_trailer()
              .change_context(ConverterError::EncodingFailed { exit_code: -14 })?;

          // Dropping the muxer drops io_out which drops the output writer,
          // finalising the upload (completing multipart upload / closing the file).
          Ok(())
      }
  }
  ```

  **Caller pattern** — the caller wires `FileStore` to `convert`, keeping the two concerns separate:
  ```rust
  let source = store.get_stream("gs://bucket/call.opus")?;
  let writer = store.upload("gs://bucket/call.mp4")?;
  OpusConverter::convert(source, writer, ConversionFormat::MP4)?;
  ```

- [ ] **T08** — Define error types in `error.rs` using `derive_more` + `serde`:
  ```rust
  use derive_more::{Display, Error};
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Display, Error, Serialize, Deserialize)]
  pub enum ConverterError {
      /// Emitted when the source file/stream cannot be opened.
      /// `path` is the exact URL or file path that was attempted.
      #[display("failed to read source: '{path}'")]
      ReadFailed { path: String },

      /// Emitted when FFmpeg demux, decode, encode, or mux fails.
      /// `exit_code` is a negative sentinel identifying the pipeline stage; no path is carried
      /// because convert() is path-agnostic — attach_printable adds context if needed.
      #[display("ffmpeg encoding failed (exit code {exit_code})")]
      EncodingFailed { exit_code: i32 },

      /// Emitted when the output cannot be written/uploaded.
      /// `destination` is the target URL or path.
      #[display("failed to write output to '{destination}'")]
      WriteFailed { destination: String },

      /// Emitted during format resolution — embed the raw string so the caller sees exactly what was rejected.
      #[display("unsupported conversion format: '{format}'")]
      UnsupportedFormat { format: String },
  }
  ```
  **Pattern rationale**:
  - Each variant carries the **on-site data that makes the instance unique** — path, exit code, destination — not a generic `message: String`.
  - `derive_more::Display` formats those fields directly into the error message so the variant is self-describing without reading `attach_printable`.
  - `attach_printable` is used for supplemental context: codec name, FFmpeg error strings, retry hints.
  - `io::Error` is not `Serialize` — convert it: `.change_context(ConverterError::ReadFailed { path: ... })`.
  - Do **not** derive `From<io::Error>` — use `change_context` so the trace is preserved.
  - Serialises to e.g. `{"ReadFailed": {"path": "gs://bucket/call.opus"}}` — directly usable in structured logs.
  - `exit_code` on `EncodingFailed` carries a negative sentinel (−1 through −14) identifying the pipeline stage that failed; no source path is included because `convert` is path-agnostic — the caller adds path context via `change_context` or `attach_printable` at the call site if needed.

- [ ] **T09** — Update `lib.rs` to re-export the public API:
  ```rust
  pub mod error;
  pub use converter::*;
  pub use file_store::*;
  pub use error::*;
  // Re-export error_stack so callers don't need to add it as a direct dep
  pub use error_stack;
  ```

---

## Technical Notes

### ac-ffmpeg Streaming Pipeline

`ac-ffmpeg` wraps FFmpeg's `AVIOContext` in a safe Rust API. `IO<T>` is the bridge:

```
IO::from_read_stream(T: Read)                 // non-seekable input  — Ogg/Opus, MKV, WAV
IO::from_seekable_read_stream(T: Read+Seek)   // seekable input      — MP4 source probing
IO::from_write_stream(T: Write)               // non-seekable output — used here
IO::from_seekable_write_stream(T: Write+Seek) // seekable output     — only if writer is seekable
```

The pipeline streams end-to-end with no intermediate buffering. `OpusConverter` sees only streams — the caller wires storage:

```
[caller]
  store.get_stream(src) ──► source: impl Read
  store.upload(dst)     ──► output: impl Write
                                │
[OpusConverter::convert]        │
  source → IO::from_read_stream → Demuxer → AudioDecoder
                                                    ↓
  output ← IO::from_write_stream ← Muxer ← AudioEncoder
  (S3UploadWriter / GcsUploadWriter / BufWriter<File>)
```

MP4 output uses `frag_keyframe+empty_moov` so the muxer writes sequentially and never seeks back — compatible with any non-seekable writer.

### Seekable vs Non-Seekable Sources

The container format — not the codec — determines whether seeking is needed during probe:

| Container       | from_read_stream OK? | Notes                                      |
|-----------------|---------------------|--------------------------------------------|
| Ogg (Opus)      | Yes                 | Sequential by design — primary DE-4295 case |
| Matroska / WebM | Yes (usually)       | EBML is sequential                         |
| MPEG-TS         | Yes                 | Live-stream native format                  |
| WAV / AIFF      | Yes                 | Simple RIFF headers                        |
| MP4 / M4A       | **No**              | moov atom may be at end; probe must seek   |

For MP4 sources (fallback path only), buffer the full object into a `Cursor<Vec<u8>>`:
```rust
let mut buf = Vec::new();
self.store.get_stream(source_path)?.read_to_end(&mut buf)?;
let io = IO::from_seekable_read_stream(io::Cursor::new(buf));
```
Only apply this when the source is confirmed MP4 and file size is bounded. The primary DE-4295 source is Ogg/Opus so `from_read_stream` is the default.

### Async Considerations

S3 and GCS SDKs are async. Use `tokio::runtime::Runtime::block_on` inside the sync `FileStore` impl methods to keep the `FileStore` trait synchronous and avoid `async_trait` complexity in the library API.

### System Library Requirements

`ac-ffmpeg` links against the same libraries as `ffmpeg-sys-next`:

```bash
# macOS
brew install ffmpeg

# Ubuntu / Debian
apt-get install -y \
  libavcodec-dev libavformat-dev libavutil-dev \
  libswresample-dev libswscale-dev pkg-config
```

Add the `apt-get` block to the Lambda Dockerfile. `ac-ffmpeg` has no mandatory feature flags — codec and format support is determined by what FFmpeg was compiled with on the host.

- [ ] **T10** — Add tracing instrumentation throughout the library:

  The library emits structured events via `tracing`; the subscriber (format, output) is configured
  by the binary that links the library. Never call `tracing_subscriber::fmt().init()` inside the
  library.

  **`OpusConverter::convert`** — instrument the function and emit events at each pipeline stage:

  ```rust
  #[tracing::instrument(skip(source, output), fields(format = ?format))]
  pub fn convert(
      source: impl io::Read + Send,
      output: impl io::Write + Send,
      format: ConversionFormat,
  ) -> error_stack::Result<(), ConverterError> {
      info!("conversion started");

      // after demuxer + stream probe:
      debug!(stream_index, codec = ?audio_stream.codec_parameters().decoder_name(), "audio stream identified");

      // after encoder + muxer built:
      let (codec_name, container_name) = format.ac_ffmpeg_names();
      debug!(%codec_name, %container_name, "encoder and muxer configured");

      // inside the transcode loop — one event per packet (trace level to stay quiet by default):
      trace!(stream_index = packet.stream_index(), "processing packet");

      // after the loop, entering flush phase:
      debug!("flushing decoder and encoder");

      // on success:
      info!("conversion completed");
      Ok(())
  }
  ```

  `skip(source, output)` prevents the `impl Read`/`impl Write` arguments (which don't implement
  `Debug`) from being captured as span fields. The `format` field is recorded on the span so it
  appears on every event emitted within the function without being repeated at each call site.

  **`LocalFileStore`** — `debug!` on every storage operation:

  ```rust
  fn get_stream(&self, path: &str) -> ... {
      debug!(%path, "opening local file for streaming");
      // ...
  }

  fn upload(&self, dest_path: &str) -> ... {
      debug!(%dest_path, "creating local file for upload");
      // ...
  }
  ```

  **`S3FileStore`** — `debug!` to record which bucket/key is touched:

  ```rust
  fn get_stream(&self, path: &str) -> ... {
      debug!(%path, "initiating S3 GetObject");
      // ...
  }

  fn upload(&self, dest_path: &str) -> ... {
      debug!(%dest_path, "initiating S3 multipart upload");
      // ...
  }
  ```

  **`GcsFileStore`** — same pattern as S3:

  ```rust
  fn get_stream(&self, path: &str) -> ... {
      debug!(%path, "initiating GCS object download");
      // ...
  }

  fn upload(&self, dest_path: &str) -> ... {
      debug!(%dest_path, "initiating GCS resumable upload session");
      // ...
  }
  ```

---

## Tracing Reference

| Level | Location | Event |
|---|---|---|
| `info` | `OpusConverter::convert` | `"conversion started"` (format in span) |
| `debug` | `OpusConverter::convert` | `"audio stream identified"` — stream_index, codec |
| `debug` | `OpusConverter::convert` | `"encoder and muxer configured"` — codec_name, container_name |
| `trace` | `OpusConverter::convert` (loop) | `"processing packet"` — stream_index |
| `debug` | `OpusConverter::convert` | `"flushing decoder and encoder"` |
| `info` | `OpusConverter::convert` | `"conversion completed"` |
| `debug` | `LocalFileStore::get_stream` | `"opening local file for streaming"` — path |
| `debug` | `LocalFileStore::upload` | `"creating local file for upload"` — dest_path |
| `debug` | `S3FileStore::get_stream` | `"initiating S3 GetObject"` — path |
| `debug` | `S3FileStore::upload` | `"initiating S3 multipart upload"` — dest_path |
| `debug` | `GcsFileStore::get_stream` | `"initiating GCS object download"` — path |
| `debug` | `GcsFileStore::upload` | `"initiating GCS resumable upload session"` — dest_path |

The `trace` level events (one per packet in the transcode loop) are intentionally excluded from
`debug` so that `RUST_LOG=debug` remains usable in production without flooding logs. Enable
`RUST_LOG=trace` only for diagnosing pipeline-level FFmpeg issues.

---

## Success Criteria

- [ ] `cargo build -p ffmpeg-converter` compiles clean with no warnings
- [ ] `cargo clippy -p ffmpeg-converter -- -D warnings` passes
- [ ] All three `FileStore` impls compile with correct associated `Writer` types
- [ ] `OpusConverter::convert` compiles with no `FileStore` import or generic — takes only `impl io::Read + Send`, `impl io::Write + Send`, and `ConversionFormat`
- [ ] `OpusConverter::convert` runs against a real `.opus` stream without writing to disk and without buffering the full output in memory
- [ ] `OpusConverter::convert` produces a valid `.mp4` (fragmented) and `.wav` output
- [ ] Public API is stable: `OpusConverter`, `ConversionFormat`, `FileStore`, `LocalFileStore`, `S3FileStore`, `S3UploadWriter`, `GcsFileStore`, `GcsUploadWriter`, `ConverterError` all exported from `lib.rs`
- [ ] `tracing` events are emitted at correct levels per the Tracing Reference table
- [ ] No `tracing_subscriber` initialisation inside the library — subscriber setup belongs in binaries only
- [ ] `#[tracing::instrument]` span on `convert` correctly skips `source` and `output` params (they are not `Debug`)
