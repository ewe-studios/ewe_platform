---
feature: "CLI Binary"
description: "bin/converter CLI binary using clap: accepts source file path, optional --format flag, runs OpusConverter with LocalFileStore"
status: "pending"
priority: "medium"
depends_on: ["00-library-implementation"]
estimated_effort: "small"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature 02: CLI Binary

## Goal

Implement `bin/converter` as a command-line binary that:

1. Accepts a positional `<input>` file path argument
2. Accepts an optional `--format <FORMAT>` flag (defaults to auto-detect from file extension, or falls back to `mp4`)
3. Accepts an optional `--output <OUTPUT>` path (defaults to input path with output extension)
4. Uses `LocalFileStore` and `OpusConverter` to perform the conversion
5. Prints the output path on success or an error message on failure

---

## CLI Interface

```
converter [OPTIONS] <INPUT>

Arguments:
  <INPUT>   Path to the source opus file

Options:
  -f, --format <FORMAT>   Output format: wave, mp4 [default: auto-detect from extension]
  -o, --output <OUTPUT>   Output file path [default: <INPUT> with new extension]
  -v, --verbose           Print full error-stack trace on failure (repeat for more detail)
  -h, --help              Print help
  -V, --version           Print version
```

### Example Invocations

```bash
# Convert to WAV
converter call.opus --format wave

# Convert to MP4, default output path (call.mp4)
converter call.opus --format mp4

# Explicit output path
converter call.opus --format wave --output /tmp/output.wav

# Auto-detect format from output extension
converter call.opus --output call.wav

# Show full error-stack trace (file/line/column) on failure
converter bad_file.opus --format wave --verbose
```

---

## Task Breakdown

- [ ] **T01** — Update `bin/converter/Cargo.toml`:
  ```toml
  [dependencies]
  ffmpeg-converter = { path = "../../packages/ffmpeg-converter" }
  clap = { version = "4", features = ["derive"] }
  derive_more = { version = "1", features = ["display", "error", "from"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  error-stack = "0.5"
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  ```

- [ ] **T02** — Define `Cli` struct with `clap` derive macros in `bin/converter/src/main.rs`:
  ```rust
  #[derive(Parser)]
  #[command(name = "converter", version, about = "Convert opus audio to WAV or MP4")]
  struct Cli {
      input: PathBuf,
      #[arg(short, long)]
      format: Option<FormatArg>,
      #[arg(short, long)]
      output: Option<PathBuf>,
      /// Print full error-stack trace on failure (file/line/column at each error site)
      #[arg(short, long, action = clap::ArgAction::SetTrue)]
      verbose: bool,
  }
  ```

- [ ] **T03** — Implement `FormatArg` as a `clap`-parseable enum:
  ```rust
  #[derive(Clone, ValueEnum)]
  enum FormatArg {
      Wave,
      Mp4,
  }
  impl From<FormatArg> for ConversionFormat { ... }
  ```

- [ ] **T04** — Implement format resolution logic:
  - If `--format` provided: use it
  - Else if `--output` provided: infer from output file extension
  - Else: default to `ConversionFormat::MP4`

- [ ] **T05** — Define a `CliError` type and implement `main` with verbosity control:
  ```rust
  use derive_more::{Display, Error};
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Display, Error, Serialize, Deserialize)]
  pub enum CliError {
      /// The input path and what was tried (flag value or extension) are captured so
      /// the user sees exactly which file and which hint failed.
      #[display("could not determine output format for '{input}' (tried: '{attempted}')")]
      UnresolvedFormat { input: String, attempted: String },

      /// Both the source path and target format are embedded so the error message
      /// is fully self-describing without needing to read attach_printable.
      #[display("failed to convert '{input}' → {format}")]
      FailedConversion { input: String, format: String },
  }

  fn main() {
      let cli = Cli::parse();

      // Initialise tracing before doing any work.
      // --verbose raises the log level from info to debug; RUST_LOG overrides both.
      let filter = tracing_subscriber::EnvFilter::try_from_default_env()
          .unwrap_or_else(|_| {
              if cli.verbose { "debug".into() } else { "info".into() }
          });
      tracing_subscriber::fmt()
          .with_writer(std::io::stderr)
          .with_env_filter(filter)
          .init();

      if let Err(report) = run(&cli) {
          if cli.verbose {
              // Full error-stack debug report: file/line/col at every site
              tracing::error!("{report:?}");
              eprintln!("{report:?}");
          } else {
              tracing::error!(%report, "conversion failed");
              eprintln!("Error: {report}");
          }
          std::process::exit(1);
      }
  }

  fn run(cli: &Cli) -> error_stack::Result<(), CliError> {
      let attempted = cli.format
          .as_ref().map(|f| format!("{f:?}"))
          .or_else(|| cli.output.as_ref()?.extension().map(|e| e.to_string_lossy().into_owned()))
          .unwrap_or_else(|| "<none>".into());

      let format = resolve_format(cli)
          .change_context(CliError::UnresolvedFormat {
              input: cli.input.display().to_string(),
              attempted,
          })
          .attach_printable("use --format wave or --format mp4")
          .attach_printable("or pass --output with the desired file extension")?;

      let format_label = format!("{format:?}");
      let store = LocalFileStore;
      let output_path = resolve_output_path(cli, &format);

      tracing::debug!(output_path = %output_path, "resolved output path");
      tracing::info!(
          input = %cli.input.display(),
          format = %format_label,
          "starting conversion",
      );

      // Obtain source stream and output writer from FileStore, then pass both to
      // OpusConverter::convert — the converter is unaware of paths or storage.
      let source = store
          .get_stream(&cli.input.to_string_lossy())
          .change_context(CliError::FailedConversion {
              input: cli.input.display().to_string(),
              format: format_label.clone(),
          })
          .attach_printable("ensure the input file exists and is readable")?;

      let writer = store
          .upload(&output_path)
          .change_context(CliError::FailedConversion {
              input: cli.input.display().to_string(),
              format: format_label.clone(),
          })
          .attach_printable(format!("could not open output path: {output_path}"))?;

      OpusConverter::convert(source, writer, format)
          .change_context(CliError::FailedConversion {
              input: cli.input.display().to_string(),
              format: format_label,
          })
          .attach_printable(format!("output path: {output_path}"))?;

      tracing::info!(output = %output_path, "conversion completed");
      println!("Converted: {output_path}");
      Ok(())
  }
  ```
  - Without `--verbose`: log level is `info`; user sees start/complete events plus any `error!` on failure
  - With `--verbose`: log level is `debug`; shows path resolution, FileStore operations, and the full error-stack trace on failure
  - `RUST_LOG` overrides both (e.g. `RUST_LOG=trace converter call.opus` to see packet-level FFmpeg events from the library)

- [ ] **T06** — Verify tracing output at each log level:

  The tracing subscriber is initialised in `main` before any other work. Confirm the following
  events appear in stderr at the expected levels when running the binary:

  | `RUST_LOG` | Events visible |
  |---|---|
  | `info` (default without `--verbose`) | `"starting conversion"`, `"conversion completed"`, `"conversion failed"` |
  | `debug` (`--verbose` or `RUST_LOG=debug`) | All `info` events + `"resolved output path"` + library events (file open, encoder config) |
  | `trace` (`RUST_LOG=trace`) | All `debug` events + packet-by-packet transcode loop events from the library |

---

## Tracing Reference

| Level | Location | Event |
|---|---|---|
| `info` | `run()` | `"starting conversion"` — `input`, `format` fields |
| `debug` | `run()` | `"resolved output path"` — `output_path` field |
| `info` | `run()` | `"conversion completed"` — `output` field |
| `error` | `main()` | `"conversion failed"` — `report` field (Display); debug format also printed to stderr |

`--verbose` sets the filter to `debug`, which also surfaces library-level events (file open,
S3/GCS requests, encoder configuration) from `ffmpeg-converter`. `RUST_LOG` always takes
precedence over `--verbose` when both are set.

---

## Success Criteria

- [ ] `cargo build -p converter` compiles clean
- [ ] `cargo clippy -p converter -- -D warnings` passes
- [ ] `converter --help` displays correct usage including `--verbose`
- [ ] `converter --version` displays version
- [ ] Running against a real opus file produces output at the expected path
- [ ] Invalid `--format` value produces a helpful clap error message
- [ ] Non-existent input file returns exit code 1 with a user-readable error message (no `--verbose`)
- [ ] Same failure with `--verbose` prints the full error-stack trace with file/line/column
- [ ] `attach_printable` hints are visible in the non-verbose output (e.g. "Pass --format wave...")
- [ ] `tracing` subscriber is initialised immediately after `Cli::parse()` — before any domain work — using the `--verbose` flag to select the log level
- [ ] `--verbose` sets log level to `debug`; `RUST_LOG` overrides `--verbose` when both are set
- [ ] `"starting conversion"` and `"conversion completed"` `info` events appear at default log level
- [ ] `"resolved output path"` `debug` event appears only at `debug` level or lower
