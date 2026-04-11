Plan: Streaming IO Fundamentals Documentation for rust-ac-ffmpeg

Context

The call-quality-ffmpeg service (DE-4295) implements an opus audio converter. Feature 00 (library implementation) defines a FileStore trait
whose get_stream method returns Box<dyn io::Read + Send> — a stream of bytes, not a file path. The current spec notes that ffmpeg-sys-next (raw
C bindings) works best with real file paths, and falls back to tempfile::NamedTempFile as a workaround.

The user wants foundational documentation on how to use io::Read streams directly as ffmpeg input via rust-ac-ffmpeg
(https://github.com/angelcam/rust-ac-ffmpeg), which wraps AVIOContext / AVIO callbacks in a safe Rust API. This document will help the
engineering team understand the streaming-first design and decide whether to use ac-ffmpeg in place of (or alongside) ffmpeg-sys-next.

---
Output

Single file to create:
specifications/DE-4295-ffmpeg-opus-converter/fundamentals/streaming-io-with-ac-ffmpeg.md

(The requirements.md front matter has has_fundamentals: false — this document creates the fundamentals section for the ticket.)

---
Document Structure

The document will be structured in 7 sections:

1. Overview

- Purpose: explain why streaming from io::Read avoids the temp-file round-trip
- Two approaches compared: temp-file (current spec) vs direct streaming (this doc)
- When each approach is appropriate

2. How rust-ac-ffmpeg Wraps AVIOContext

- IO<T> type: wraps any Read/Write/Seek into an FFmpeg AVIOContext
- Four constructors: from_read_stream, from_seekable_read_stream, from_write_stream, from_seekable_write_stream
- Internal mechanism: 4 KiB buffer, C callbacks (read_packet, write_packet, seek), heap-pinned Box<T>
- Memory safety model: why Box<T> keeps the pointer stable
- MemWriter for in-memory output

3. Complete Streaming Decode Pipeline (Read → Frames)

Step-by-step with annotated code:
1. Wrap a Box<dyn Read> in IO::from_read_stream
2. Build Demuxer → find_stream_info
3. Locate the audio stream, AudioDecoder::from_stream
4. Packet loop: demuxer.take() → decoder.push() → decoder.take()
5. Flush decoder at EOF

4. Complete Re-mux / Transcode Pipeline (Read → Write)

Step-by-step with annotated code:
1. Demux + decode from IO<impl Read> (from FileStore::get_stream)
2. Encode to target codec (AAC for MP4, PCM for WAV)
3. Mux to IO<MemWriter> (in-memory) or IO<impl Write> (streaming upload)
4. Close muxer, recover bytes, pipe to FileStore::upload
5. Fragmented MP4 flag for progressive draining

5. Integration with the FileStore Trait

- How FileStore::get_stream() -> Box<dyn io::Read + Send> plugs directly into IO::from_read_stream
- How FileStore::upload(dest, reader) can accept output from a MemWriter via Cursor<Vec<u8>>
- Full end-to-end OpusConverter::convert sketch using ac-ffmpeg instead of temp files

6. Seekable vs Non-Seekable Sources — Decision Guide

- When opus/ogg/matroska/ts formats can use non-seekable input
- When MP4/M4A containers require seeking during probe (must buffer or use Cursor)
- Strategy: Box<dyn Read + Seek> variant of FileStore for seekable backends (local FS, GCS signed URLs with range requests)
- Fallback pattern: buffer into Cursor<Vec<u8>> if stream size is known and bounded

7. Cargo Dependencies and Compile Notes

- Add ac-ffmpeg to packages/ffmpeg-converter/Cargo.toml
- System library requirements (same as ffmpeg-sys-next)
- Feature flags: none required (all features on by default)
- ac-ffmpeg uses it internally; both may coexist but using ac-ffmpeg should cover the full conversion pipeline
without dropping to raw C

---
Key Sources Used

┌──────────────────────────────────────────────────────────────────┬───────────────────────────────────────────────────────────────────────────
┐
│                              Source                              │                               Key findings
│
├──────────────────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────────────────────
┤
│ specifications/.../features/00-library-implementation/feature.md │ FileStore::get_stream returns Box<dyn io::Read + Send>; temp file
│
│                                                                  │ strategy currently planned
│
├──────────────────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────────────────────
┤
│ specifications/.../requirements.md                               │ Architecture overview, error handling strategy
│
├──────────────────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────────────────────
┤
│ packages/ffmpeg-converter/src/lib.rs                             │ Scaffold only — no implementations yet
│
├──────────────────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────────────────────
┤
│ rust-ac-ffmpeg GitHub + source                                   │ Full IO<T> API, MemWriter, decoder/encoder/muxer patterns, examples
│
└──────────────────────────────────────────────────────────────────┴───────────────────────────────────────────────────────────────────────────
┘

---
Verification

After implementation:
1. Engineer reads fundamentals/streaming-io-with-ac-ffmpeg.md — all code snippets must be self-contained and directly usable
2. Code snippets must compile if dropped into a Rust file with ac-ffmpeg in scope
3. Update requirements.md front matter: has_fundamentals: true

---
Files to Create / Modify

┌──────────────────────────────────────────────────────────────────────────────────────────┬─────────────────────────────────────┐
│                                           File                                           │               Action                │
├──────────────────────────────────────────────────────────────────────────────────────────┼─────────────────────────────────────┤
│ specifications/DE-4295-ffmpeg-opus-converter/fundamentals/streaming-io-with-ac-ffmpeg.md │ Create — the fundamentals document  │
├──────────────────────────────────────────────────────────────────────────────────────────┼─────────────────────────────────────┤
│ specifications/DE-4295-ffmpeg-opus-converter/requirements.md                             │ Update — set has_fundamentals: true │
└──────────────────────────────────────────────────────────────────────────────────────────┴─────────────────────────────────────┘
