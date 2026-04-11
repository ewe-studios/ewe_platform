---
feature: "Library Tests"
description: "Unit and integration tests for the ffmpeg-converter crate covering FileStore implementations and OpusConverter"
status: "pending"
priority: "high"
depends_on: ["00-library-implementation"]
estimated_effort: "medium"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Feature 01: Library Tests

## Goal

Write unit and integration tests for `packages/ffmpeg-converter` that verify:

- `FileStore` trait implementations (local, S3, GCS)
- `OpusConverter::convert` end-to-end with a real opus sample file
- `ConversionFormat` enum methods
- Error propagation paths

---

## Test Directory Layout

```
packages/ffmpeg-converter/
└── tests/
    ├── common/
    │   └── mod.rs              # fixture helpers + AudioInfo probe helper
    ├── fixtures/
    │   └── sample.opus         # committed generated fixture (~20 KB)
    ├── test_local_store.rs     # LocalFileStore unit tests
    ├── test_s3_store.rs        # S3FileStore unit tests (mocked)
    ├── test_gcs_store.rs       # GcsFileStore unit tests (mocked)
    └── test_converter.rs       # OpusConverter integration tests
```

Unit tests for `ConversionFormat` and `ConverterError` live in the source files themselves (`#[cfg(test)]` modules).

---

## Task Breakdown

- [ ] **T01** — Generate and commit test fixtures via `mise`:

  The service root has a `mise.toml` that declares `ffmpeg = "latest"` as a managed tool,
  handling installation cross-platform. Run once to generate the fixture, then commit it:

  ```bash
  mise install          # installs ffmpeg via mise (macOS / Linux)
  mise run gen-fixtures # generates packages/ffmpeg-converter/tests/fixtures/sample.opus
  git add packages/ffmpeg-converter/tests/fixtures/sample.opus
  ```

  The `gen-fixtures` task produces a **5-second 440 Hz sine wave, mono, 48 kHz** encoded as
  Ogg/Opus at 32 kbps (~20 KB). The fixture parameters are fixed so expected output properties
  are known exactly — no reference binary needed.

  Add `fixture_path(name: &str) -> PathBuf` and `probe_audio(path: &Path) -> AudioInfo`
  helpers to `tests/common/mod.rs`:

  ```rust
  use std::fs::File;
  use std::path::{Path, PathBuf};
  use ac_ffmpeg::{format::demuxer::Demuxer, io::IO};

  pub fn fixture_path(name: &str) -> PathBuf {
      Path::new(env!("CARGO_MANIFEST_DIR"))
          .join("tests/fixtures")
          .join(name)
  }

  /// Properties extracted from an audio container using ac-ffmpeg's Demuxer.
  /// Used to validate conversion output without byte comparison
  /// (encoded audio is not bitwise reproducible across runs).
  #[derive(Debug)]
  pub struct AudioInfo {
      pub codec_name: String,   // e.g. "aac", "pcm_s16le"
      pub sample_rate: u32,
      pub channels: u32,
      pub duration_ms: u64,
  }

  pub fn probe_audio(path: &Path) -> AudioInfo {
      let file = File::open(path).expect("probe: file not found");
      let io = IO::from_seekable_read_stream(file);
      let demuxer = Demuxer::builder()
          .build(io).unwrap()
          .find_stream_info(None).map_err(|(_, e)| e).unwrap();

      let stream = demuxer.streams().iter()
          .find(|s| s.codec_parameters().is_audio_codec())
          .expect("probe: no audio stream");

      let params = stream.codec_parameters();
      AudioInfo {
          codec_name:  params.decoder_name().unwrap_or("unknown").to_string(),
          sample_rate: params.sample_rate() as u32,
          channels:    params.channel_layout().channels() as u32,
          duration_ms: stream.duration()
              .map(|d| (d.as_secs_f64() * 1000.0) as u64)
              .unwrap_or(0),
      }
  }
  ```

- [ ] **T02** — Unit tests for `ConversionFormat` in `converter.rs`:
  ```rust
  #[test]
  fn test_extension_wave() { assert_eq!(ConversionFormat::Wave.extension(), "wav"); }
  #[test]
  fn test_extension_mp4() { assert_eq!(ConversionFormat::MP4.extension(), "mp4"); }
  #[test]
  fn test_ac_ffmpeg_names_wave() {
      assert_eq!(ConversionFormat::Wave.ac_ffmpeg_names(), ("pcm_s16le", "wav"));
  }
  #[test]
  fn test_ac_ffmpeg_names_mp4() {
      assert_eq!(ConversionFormat::MP4.ac_ffmpeg_names(), ("aac", "mp4"));
  }
  ```

- [ ] **T03** — Unit tests for `LocalFileStore` in `tests/test_local_store.rs`:
  - `test_get_stream_existing_file`: opens fixture, reads bytes, asserts non-empty
  - `test_get_stream_missing_file`: returns `Err` for non-existent path
  - `test_upload_returns_writer`: `upload` opens a `BufWriter<File>` at the destination path; write bytes into it, drop it, verify file content on disk

- [ ] **T04** — Unit tests for `S3FileStore` in `tests/test_s3_store.rs`:
  - Use `mockall` or `aws-smithy-mocks` to mock the S3 client
  - `test_get_stream_returns_bytes`: mock `get_object` response, verify readable stream
  - `test_upload_returns_writer`: mock multipart upload initiation, write bytes into the returned `S3UploadWriter`, drop it, verify `complete_multipart_upload` was called
  - `test_get_stream_not_found`: mock NoSuchKey error, verify `Err` returned

- [ ] **T05** — Unit tests for `GcsFileStore` in `tests/test_gcs_store.rs`:
  - Use `httpmock` to intercept GCS HTTP calls
  - `test_get_stream_downloads_object`: verify bytes returned from `get_stream`
  - `test_upload_returns_writer`: write bytes into returned `GcsUploadWriter`, drop it, verify the resumable upload session was finalised

- [ ] **T06** — Integration test for `OpusConverter` in `tests/test_converter.rs`:

  The fixture is a **5-second 440 Hz sine wave, mono, 48 kHz** — so expected output properties
  are known exactly. Use `probe_audio()` to assert them rather than byte comparison (encoded
  audio is not bitwise reproducible across runs).

  ```rust
  use common::{fixture_path, probe_audio};

  // Expected properties of sample.opus (fixed by gen-fixtures task)
  const EXPECTED_SAMPLE_RATE: u32 = 48_000;
  const EXPECTED_CHANNELS: u32    = 1;
  const EXPECTED_DURATION_MS: u64 = 5_000;
  const DURATION_TOLERANCE_MS: u64 = 100; // allow ±100 ms for encoder latency

  #[test]
  #[ignore = "requires ffmpeg system libs"]
  fn test_convert_opus_to_wav() {
      let store = LocalFileStore;
      let tmp = tempfile::tempdir().unwrap();
      let out_path = tmp.path().join("output.wav");

      let source = store.get_stream(fixture_path("sample.opus").to_str().unwrap()).unwrap();
      let writer = store.upload(out_path.to_str().unwrap()).unwrap();
      OpusConverter::convert(source, writer, ConversionFormat::Wave).unwrap();

      let info = probe_audio(&out_path);
      assert_eq!(info.codec_name,  "pcm_s16le");
      assert_eq!(info.sample_rate, EXPECTED_SAMPLE_RATE);
      assert_eq!(info.channels,    EXPECTED_CHANNELS);
      assert!(
          info.duration_ms.abs_diff(EXPECTED_DURATION_MS) <= DURATION_TOLERANCE_MS,
          "WAV duration {}ms outside ±{}ms of {}ms",
          info.duration_ms, DURATION_TOLERANCE_MS, EXPECTED_DURATION_MS,
      );
  }

  #[test]
  #[ignore = "requires ffmpeg system libs"]
  fn test_convert_opus_to_mp4() {
      let store = LocalFileStore;
      let tmp = tempfile::tempdir().unwrap();
      let out_path = tmp.path().join("output.mp4");

      let source = store.get_stream(fixture_path("sample.opus").to_str().unwrap()).unwrap();
      let writer = store.upload(out_path.to_str().unwrap()).unwrap();
      OpusConverter::convert(source, writer, ConversionFormat::MP4).unwrap();

      let info = probe_audio(&out_path);
      assert_eq!(info.codec_name,  "aac");
      assert_eq!(info.sample_rate, EXPECTED_SAMPLE_RATE);
      assert_eq!(info.channels,    EXPECTED_CHANNELS);
      assert!(
          info.duration_ms.abs_diff(EXPECTED_DURATION_MS) <= DURATION_TOLERANCE_MS,
          "MP4 duration {}ms outside ±{}ms of {}ms",
          info.duration_ms, DURATION_TOLERANCE_MS, EXPECTED_DURATION_MS,
      );
  }

  #[test]
  #[ignore = "requires ffmpeg system libs"]
  fn test_convert_error_on_invalid_input() {
      let store = LocalFileStore;
      let tmp = tempfile::tempdir().unwrap();
      let out_path = tmp.path().join("output.wav");

      // Pass raw text as input — FFmpeg will fail to probe the stream
      let source = io::Cursor::new(b"this is not audio data");
      let writer = store.upload(out_path.to_str().unwrap()).unwrap();
      let result = OpusConverter::convert(source, writer, ConversionFormat::Wave);

      let report = result.unwrap_err();
      eprintln!("{report:?}");   // full file/line/col trace visible on test failure
      assert!(report.contains::<ConverterError>());
  }
  ```

---

## Test Dependencies (add to `Cargo.toml` under `[dev-dependencies]`)

```toml
[dev-dependencies]
tempfile   = "3"
mockall    = "0.12"
httpmock   = "0.7"
serde_json = "1"   # assert error JSON serialization round-trips
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
# ac-ffmpeg is already a direct dependency — probe_audio() in common/mod.rs uses it directly
```

`mise install` handles ffmpeg system library installation cross-platform before running tests:

```bash
mise install              # installs ffmpeg (macOS + Linux)
mise run gen-fixtures     # generate tests/fixtures/sample.opus — commit this file
cargo test --workspace    # ignored tests need ffmpeg libs: add --include-ignored to run them
```

---

- [ ] **T07** — Add tracing initialisation helper to `tests/common/mod.rs`:

  Tests that exercise `OpusConverter` or `FileStore` impls should initialise a subscriber so
  tracing events from the library are visible in the test output when a test fails. Use
  `try_init` — it returns `Err` if already initialised (e.g. from a previous test in the same
  process), which is safe to ignore:

  ```rust
  /// Call at the top of any test that exercises OpusConverter or FileStore.
  ///
  /// Output is captured per-test via `with_test_writer()` — events only appear
  /// in the terminal if the test fails, keeping successful output clean.
  pub fn init_tracing() {
      let _ = tracing_subscriber::fmt()
          .with_test_writer()
          .with_env_filter(
              tracing_subscriber::EnvFilter::try_from_default_env()
                  .unwrap_or_else(|_| "debug".into()),
          )
          .try_init();
  }
  ```

  Call it at the top of each test:

  ```rust
  #[test]
  #[ignore = "requires ffmpeg system libs"]
  fn test_convert_opus_to_wav() {
      common::init_tracing();
      // ...
  }
  ```

  The `RUST_LOG` environment variable overrides the default `debug` filter — set
  `RUST_LOG=trace` when diagnosing packet-level FFmpeg issues.

---

## Tracing in Tests

| Level | When visible in tests |
|---|---|
| `info` | Always (default filter is `debug`, which includes `info`) |
| `debug` | Always (default filter) — file open/close, S3/GCS requests, encoder config |
| `trace` | Only when `RUST_LOG=trace` — packet-by-packet transcode loop; very verbose |

Output from `with_test_writer()` is suppressed for passing tests and printed for failing ones,
so leaving `debug` as the default is safe — it does not pollute successful test runs.

---

## Error Assertion Pattern

Use `error-stack`'s report inspection in tests:
```rust
let report = result.unwrap_err();
// Check the error variant
assert!(report.contains::<ConverterError>());
// Optionally serialize to JSON to assert structure
let json = serde_json::to_string(report.current_context()).unwrap();
assert!(json.contains("Ffmpeg"));
```

---

## Success Criteria

- [ ] `cargo test -p ffmpeg-converter` passes all non-ignored tests
- [ ] `LocalFileStore` has 100% test coverage for public methods
- [ ] `OpusConverter` integration tests pass when ffmpeg libs are available
- [ ] No `unwrap()` or `expect()` in test assertions — use `assert!`, `assert_eq!`, or `report.contains::<ConverterError>()`
- [ ] `ConverterError` variants serialize to valid JSON (round-trip test with `serde_json`)
- [ ] All tests are deterministic (no flaky timing or network calls without mocks)
- [ ] `init_tracing()` is called in every integration test; tracing events from the library are visible on test failure
- [ ] No `tracing_subscriber::fmt().init()` (non-try variant) — only `try_init()` is used to avoid panics when multiple tests share a process
