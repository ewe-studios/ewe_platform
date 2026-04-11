---
feature: "CI, Docker Images"
description: "GitHub Actions test job, Dockerfiles with statically-linked FFmpeg for converter CLI and lambda_worker, and mise tasks for local Docker builds"
status: "pending"
priority: "high"
depends_on: ["00-library-implementation", "02-cli-binary", "03-lambda-worker"]
estimated_effort: "medium"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# Feature 04: CI, Docker Images

## Goal

1. Add a `call-quality-ffmpeg-test` job to `.github/workflows/deploy-data-services.yaml` that installs mise, installs ffmpeg dev libs, and runs all tests (fixture is committed — no regeneration needed in CI)
2. Create `bin/converter/Dockerfile` — three-stage build with statically-linked FFmpeg; runtime stage has **zero ffmpeg dependencies**
3. Create `bin/lambda_worker/Dockerfile` — same static approach; runtime stage is the bare AWS Lambda provided image with **zero ffmpeg dependencies**
4. Add `docker:build:converter` and `docker:build:lambda` tasks to `mise.toml` for local builds

### Static FFmpeg Linking

`ac-ffmpeg` uses `pkg-config` to locate FFmpeg at compile time. By building FFmpeg with
`--disable-shared --enable-static` in a dedicated stage and pointing `PKG_CONFIG_LIBDIR` at the
resulting `.pc` files with `PKG_CONFIG_ALL_STATIC=1`, Cargo statically links all libav* code
directly into the binary. The linker embeds the FFmpeg object code — no `.so` files are needed at
runtime. This is fully supported by `ac-ffmpeg` and `ffmpeg-sys-next`; no additional crate
features are required.

Benefits:
- Runtime stage needs no FFmpeg packages — just the binary
- Alpine or `provided:al2023` both work as runtime bases (see note below on Alpine)
- Smaller attack surface; no shared library version mismatches

Only the codecs and formats actually used are compiled in (`--disable-everything --enable-*`),
keeping the binary addition to ~5–10 MB.

---

## Task Breakdown

- [ ] **T01** — Add `call-quality-ffmpeg` path filter to the `changes` job in `.github/workflows/deploy-data-services.yaml`:

  ```yaml
  # Under jobs.changes.steps[filter].with.filters — add alongside existing filters:
  call-quality-ffmpeg:
    - '.github/workflows/deploy-data-services.yaml'
    - 'services/call-quality-ffmpeg/**'
  ```

- [ ] **T02** — Add `call-quality-ffmpeg-test` job to `.github/workflows/deploy-data-services.yaml`:

  The test fixture (`tests/fixtures/sample.opus`) is committed to the repo — CI does not
  regenerate it.

  ```yaml
  call-quality-ffmpeg-test:
    name: "Test: services/call-quality-ffmpeg"
    needs: [changes]
    if: contains(fromJson(needs.changes.outputs.changes), 'call-quality-ffmpeg')
    runs-on: ubuntu-24.04
    defaults:
      run:
        working-directory: services/call-quality-ffmpeg
    steps:
      - name: "Checkout the repository"
        uses: actions/checkout@v4

      - name: "Install mise"
        uses: jdx/mise-action@v2
        with:
          working_directory: services/call-quality-ffmpeg

      - name: "Install ffmpeg system libraries"
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \
            libavcodec-dev libavformat-dev libavutil-dev \
            libswresample-dev libswscale-dev pkg-config

      - name: "Lint"
        run: mise run lint

      - name: "Test"
        run: cargo test --workspace --include-ignored
  ```

  > `jdx/mise-action` reads `mise.toml`, installs rust 1.86 and ffmpeg, and puts them on `PATH`.
  > ffmpeg system dev libs are installed via `apt-get` for `ac-ffmpeg` to link against at compile time.

- [ ] **T03** — Create `bin/converter/Dockerfile`:

  Three stages: static FFmpeg build → Rust binary build → minimal runtime.

  ```dockerfile
  # ── Stage 1: Build static FFmpeg ─────────────────────────────────────────────
  FROM debian:bookworm-slim AS ffmpeg-static

  RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential nasm yasm wget pkg-config libopus-dev \
      && rm -rf /var/lib/apt/lists/*

  # Download and build FFmpeg with only the codecs/formats this service uses.
  # --disable-everything + selective --enable-* keeps the static lib set small.
  RUN wget -q https://ffmpeg.org/releases/ffmpeg-7.1.tar.gz \
      && tar xf ffmpeg-7.1.tar.gz \
      && cd ffmpeg-7.1 \
      && ./configure \
          --prefix=/opt/ffmpeg \
          --disable-shared \
          --enable-static \
          --disable-programs \
          --disable-doc \
          --disable-everything \
          --enable-decoder=opus \
          --enable-demuxer=ogg \
          --enable-parser=opus \
          --enable-encoder=aac \
          --enable-encoder=pcm_s16le \
          --enable-muxer=mp4 \
          --enable-muxer=wav \
          --enable-libopus \
      && make -j"$(nproc)" \
      && make install

  # ── Stage 2: Build Rust binary ────────────────────────────────────────────────
  FROM rust:1.86-slim-bookworm AS builder

  RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config build-essential \
      && rm -rf /var/lib/apt/lists/*

  # Copy static FFmpeg installation from stage 1
  COPY --from=ffmpeg-static /opt/ffmpeg /opt/ffmpeg

  # Tell pkg-config to use our static FFmpeg and prefer .a over .so
  ENV PKG_CONFIG_LIBDIR=/opt/ffmpeg/lib/pkgconfig
  ENV PKG_CONFIG_ALL_STATIC=1

  WORKDIR /build
  COPY . .
  RUN cargo build --release -p converter

  # ── Stage 3: Minimal runtime ──────────────────────────────────────────────────
  # No FFmpeg packages needed — all libav* code is embedded in the binary.
  FROM debian:bookworm-slim

  COPY --from=builder /build/target/release/converter /usr/local/bin/converter

  ENTRYPOINT ["converter"]
  ```

- [ ] **T04** — Create `bin/lambda_worker/Dockerfile`:

  Same three-stage static pattern; runtime is the AWS Lambda provided AL2023 image.

  ```dockerfile
  # ── Stage 1: Build static FFmpeg ─────────────────────────────────────────────
  FROM debian:bookworm-slim AS ffmpeg-static

  RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential nasm yasm wget pkg-config libopus-dev \
      && rm -rf /var/lib/apt/lists/*

  RUN wget -q https://ffmpeg.org/releases/ffmpeg-7.1.tar.gz \
      && tar xf ffmpeg-7.1.tar.gz \
      && cd ffmpeg-7.1 \
      && ./configure \
          --prefix=/opt/ffmpeg \
          --disable-shared \
          --enable-static \
          --disable-programs \
          --disable-doc \
          --disable-everything \
          --enable-decoder=opus \
          --enable-demuxer=ogg \
          --enable-parser=opus \
          --enable-encoder=aac \
          --enable-encoder=pcm_s16le \
          --enable-muxer=mp4 \
          --enable-muxer=wav \
          --enable-libopus \
      && make -j"$(nproc)" \
      && make install

  # ── Stage 2: Build Rust binary ────────────────────────────────────────────────
  FROM rust:1.86-slim-bookworm AS builder

  RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config build-essential \
      && rm -rf /var/lib/apt/lists/*

  COPY --from=ffmpeg-static /opt/ffmpeg /opt/ffmpeg

  ENV PKG_CONFIG_LIBDIR=/opt/ffmpeg/lib/pkgconfig
  ENV PKG_CONFIG_ALL_STATIC=1

  WORKDIR /build
  COPY . .
  RUN cargo build --release -p lambda_worker

  # ── Stage 3: Lambda runtime ───────────────────────────────────────────────────
  # provided:al2023 supplies the Lambda runtime interface (glibc-based AL2023).
  # No FFmpeg packages needed — all libav* code is embedded in the binary.
  FROM public.ecr.aws/lambda/provided:al2023

  # Lambda custom runtime expects the entrypoint binary to be named 'bootstrap'
  COPY --from=builder /build/target/release/lambda_worker ${LAMBDA_TASK_ROOT}/bootstrap

  CMD ["bootstrap"]
  ```

  > **On Alpine**: Alpine uses musl libc. The builder (`rust:1.86-slim-bookworm`) produces a
  > glibc-linked binary even with static FFmpeg — the C runtime (glibc) itself is still dynamic.
  > A glibc binary cannot run on Alpine. To target Alpine, add
  > `--target x86_64-unknown-linux-musl` to the cargo build and install `musl-tools` in the
  > builder. The `provided:al2023` runtime is glibc-based and is the simpler, recommended path.

- [ ] **T05** — Add Docker build tasks to `mise.toml` (already done in a prior commit):

  ```toml
  [tasks."docker:build:converter"]
  description = "Build the converter CLI Docker image"
  run = """
  docker build -f bin/converter/Dockerfile -t call-quality-ffmpeg/converter:local .
  """

  [tasks."docker:build:lambda"]
  description = "Build the lambda_worker Docker image"
  run = """
  docker build -f bin/lambda_worker/Dockerfile -t call-quality-ffmpeg/lambda-worker:local .
  """

  [tasks."docker:build"]
  description = "Build all Docker images"
  depends = ["docker:build:converter", "docker:build:lambda"]
  ```

---

## Technical Notes

### How Static Linking Works with ac-ffmpeg

`ac-ffmpeg` calls `pkg-config` via its build script to locate `libavcodec`, `libavformat`, etc.
Two environment variables control the linking mode:

| Variable | Value | Effect |
|---|---|---|
| `PKG_CONFIG_LIBDIR` | `/opt/ffmpeg/lib/pkgconfig` | Overrides the pkg-config search path to the static-only install |
| `PKG_CONFIG_ALL_STATIC` | `1` | Instructs pkg-config to emit `--libs` flags for `.a` files instead of `.so` |

When both are set, the Rust linker (`cc`) receives `-lavcodec -lavformat ...` flags that resolve
to `.a` archives in `/opt/ffmpeg/lib/`. The linker copies the required object code into the final
binary. The runtime container has no knowledge of FFmpeg.

No Cargo feature flags or `build.rs` changes are required — it is handled entirely through the
environment at Docker build time.

### Minimal FFmpeg Build

Building with `--disable-everything` then selectively enabling only what the service uses:

| Flag | Reason |
|---|---|
| `--enable-decoder=opus` | Decode Ogg/Opus input |
| `--enable-parser=opus` | Frame boundary detection for Opus |
| `--enable-demuxer=ogg` | Demux the Ogg container |
| `--enable-encoder=aac` | Encode MP4 output |
| `--enable-encoder=pcm_s16le` | Encode WAV output |
| `--enable-muxer=mp4` | Mux fragmented MP4 container |
| `--enable-muxer=wav` | Mux WAV container |
| `--enable-libopus` | Link against system libopus for higher-quality Opus decode |

This keeps the static library set to ~15–20 MB and the binary addition to ~5–10 MB.

### Build Context

Both Dockerfiles use the workspace root (`services/call-quality-ffmpeg/`) as the Docker build
context so the full `Cargo.toml` workspace is available. Run `docker build` from that directory,
which is what the mise tasks do.

### Lambda Binary Name

`lambda_runtime` requires the binary at `${LAMBDA_TASK_ROOT}/bootstrap`. The `CMD ["bootstrap"]`
instruction matches this name.

---

## Success Criteria

- [ ] `call-quality-ffmpeg-test` CI job runs on every PR touching `services/call-quality-ffmpeg/**`
- [ ] CI does not call `gen-fixtures` — the committed fixture is used as-is
- [ ] All `cargo test --workspace --include-ignored` tests pass in CI
- [ ] `mise run docker:build:converter` builds without error; `docker run --rm call-quality-ffmpeg/converter:local --help` succeeds
- [ ] `mise run docker:build:lambda` builds without error; runtime stage contains no `libavcodec` or `libavformat` packages
- [ ] `ldd` on the binary inside the runtime stage shows no `libav*` dependencies
- [ ] Dockerfiles live at `bin/converter/Dockerfile` and `bin/lambda_worker/Dockerfile`
