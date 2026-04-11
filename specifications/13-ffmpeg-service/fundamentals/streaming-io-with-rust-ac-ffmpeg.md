Streaming IO with rust-ac-ffmpeg

Fundamentals for Feature 00: Library Implementation (DE-4295)

---
1. Overview

Feature 00 specifies FileStore::get_stream() returning Box<dyn io::Read + Send> — a byte stream, not a file path. The current spec handles this
with a tempfile::NamedTempFile round-trip: dump the stream to disk, hand the path to ffmpeg-sys-next, then clean up. That works, but it
materialises the entire audio file on disk before a single frame is decoded.

rust-ac-ffmpeg eliminates that round-trip. It wraps FFmpeg's AVIOContext in a safe Rust API that accepts any impl Read (or impl Read + Seek)
directly as an ffmpeg input source, and any impl Write (or impl Write + Seek) as an ffmpeg output sink. The stream from get_stream() plugs in with
  zero intermediate files.

Approach comparison:

┌────────────────────────────┬──────────────────────────────┬─────────────────────────────┐
│                            │   Temp-file (current spec)   │     ac-ffmpeg streaming     │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ Input materialised on disk │ Yes — full file              │ Never                       │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ Requires real file path    │ Yes                          │ No                          │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ System library deps        │ ffmpeg-sys-next + libav*     │ ac-ffmpeg + same libav*     │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ Rust API safety            │ Unsafe C bindings            │ Safe wrapper                │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ Seekable input required    │ No (path is always seekable) │ Only for MP4/M4A containers │
├────────────────────────────┼──────────────────────────────┼─────────────────────────────┤
│ In-memory output           │ Requires second temp file    │ MemWriter built in          │
└────────────────────────────┴──────────────────────────────┴─────────────────────────────┘

When each is appropriate:

- Use ac-ffmpeg streaming when the source is a cloud stream (S3 byte range, GCS download) and the container is seekable-friendly (Ogg/Opus, MKV,
MPEG-TS). This is the primary case for DE-4295.
- Fall back to temp-file only if you encounter a container format that hard-requires seeking during probe and buffering into a Cursor<Vec<u8>> is
impractical (very large files from an unknown-length stream).

---
2. How rust-ac-ffmpeg Wraps AVIOContext

The IO<T> Type

ac_ffmpeg::io::IO<T> is the bridge between Rust IO traits and FFmpeg's AVIOContext. Internally it:

1. Allocates a 4 KiB C-side buffer (FFmpeg default)
2. Registers C callbacks — read_packet, write_packet, seek — that forward into your T
3. Heap-pins the Box<T> so the pointer passed to C remains stable across moves

Four constructors cover all combinations:

// Non-seekable input  — Box<dyn Read> works here
IO::from_read_stream(inner: T)             where T: Read

// Seekable input — required for MP4 source probing
IO::from_seekable_read_stream(inner: T)    where T: Read + Seek

// Non-seekable output — streaming upload to S3/GCS via put_object body
IO::from_write_stream(inner: T)            where T: Write

// Seekable output — required for MP4/M4A mux (writes moov atom at end)
IO::from_seekable_write_stream(inner: T)   where T: Write + Seek

Memory Safety Model

The Box<T> is pinned on the heap before the pointer is handed to C. IO<T> owns that Box for its lifetime. FFmpeg never outlives the IO<T> because
Demuxer / Muxer borrow or consume it — Rust's ownership model enforces the lifetime contract that C cannot express.

MemWriter

ac_ffmpeg::io::MemWriter is a Write + Seek backed by a Vec<u8>. Use it as the muxer output to capture the encoded container entirely in memory,
then hand the bytes to FileStore::upload via io::Cursor.

use ac_ffmpeg::io::MemWriter;

let writer = MemWriter::new();
// ... build muxer with IO::from_seekable_write_stream(writer) ...
// After muxer.write_trailer():
let bytes: Vec<u8> = muxer.into_io().into_inner().into_contents();
let reader = io::Cursor::new(bytes);
store.upload(dest_path, Box::new(reader))?;

---
3. Complete Streaming Decode Pipeline (Read → Frames)

This pipeline takes any Box<dyn io::Read + Send> and produces decoded AudioFrame values. It is the input half of OpusConverter::convert.

use std::io;
use ac_ffmpeg::{
    codec::audio::AudioDecoder,
    format::demuxer::Demuxer,
    io::IO,
    Error as FfmpegError,
};

fn decode_opus_stream(
    stream: Box<dyn io::Read + Send>,
) -> Result<Vec<ac_ffmpeg::codec::audio::AudioFrame>, FfmpegError> {
    // Step 1: Wrap the io::Read in an AVIOContext.
    // from_read_stream is sufficient for Ogg/Opus — the container
    // does not require seeking during format probe.
    let io = IO::from_read_stream(stream);

    // Step 2: Build a Demuxer and probe stream info.
    // find_stream_info reads enough packets to populate codec parameters
    // on every stream. The None argument means "no timeout".
    let mut demuxer = Demuxer::builder()
        .build(io)?
        .find_stream_info(None)
        .map_err(|(_, err)| err)?;

    // Step 3: Locate the first audio stream.
    // streams() returns metadata only — no packets consumed here.
    let (stream_index, audio_stream) = demuxer
        .streams()
        .iter()
        .enumerate()
        .find(|(_, s)| s.codec_parameters().is_audio_codec())
        .ok_or_else(|| FfmpegError::new("no audio stream found in input"))?;

    // Step 4: Build an AudioDecoder from the stream's codec parameters.
    // This opens the codec context with the parameters found during probe.
    let mut decoder = AudioDecoder::from_stream(audio_stream)?.build()?;

    let mut frames = Vec::new();

    // Step 5: Packet loop — demux → decode → collect frames.
    // demuxer.take() returns Ok(None) at EOF.
    while let Some(packet) = demuxer.take()? {
        // Skip packets from non-audio streams (e.g. video, subtitles).
        if packet.stream_index() != stream_index {
            continue;
        }

        // Push the compressed packet into the decoder.
        decoder.push(packet)?;

        // Drain all decoded frames the decoder is ready to yield.
        // A single packet can produce multiple frames (e.g. AAC 1024-sample blocks).
        while let Some(frame) = decoder.take()? {
            frames.push(frame);
        }
    }

    // Step 6: Flush — signal EOF to the decoder so it emits buffered frames.
    // Omitting flush silently drops the last few frames.
    decoder.flush()?;
    while let Some(frame) = decoder.take()? {
        frames.push(frame);
    }

    Ok(frames)
}

Key points:
- from_read_stream is correct for Ogg/Opus sources — no seeking needed
- find_stream_info consumes some packets internally; the demuxer replays them correctly
- The flush step is mandatory — decoders with lookahead (AAC, MP3, Opus) hold back frames until flushed

---
4. Complete Re-mux / Transcode Pipeline (Read → Write)

This is the full convert pipeline: stream in → decode → encode → mux → stream out.

use std::io;
use ac_ffmpeg::{
    codec::audio::{AudioDecoder, AudioEncoder, ChannelLayout, SampleFormat},
    format::{
        demuxer::Demuxer,
        muxer::{Muxer, OutputFormat},
    },
    io::{IO, MemWriter},
    Error as FfmpegError,
};

/// Transcode an Opus stream to the target container/codec.
/// Returns the encoded bytes ready for upload.
fn transcode(
    input: Box<dyn io::Read + Send>,
    output_format_name: &str, // "mp4" or "wav"
    codec_name: &str,         // "aac" for MP4, "pcm_s16le" for WAV
) -> Result<Vec<u8>, FfmpegError> {
    // ── INPUT SIDE ──────────────────────────────────────────────────────────

    let io_in = IO::from_read_stream(input);

    let mut demuxer = Demuxer::builder()
        .build(io_in)?
        .find_stream_info(None)
        .map_err(|(_, err)| err)?;

    let (stream_index, audio_stream) = demuxer
        .streams()
        .iter()
        .enumerate()
        .find(|(_, s)| s.codec_parameters().is_audio_codec())
        .ok_or_else(|| FfmpegError::new("no audio stream found"))?;

    let mut decoder = AudioDecoder::from_stream(audio_stream)?.build()?;

    // Read sample rate and channel layout from the source stream
    // so the encoder matches the input characteristics.
    let codec_params = audio_stream.codec_parameters();

    // ── OUTPUT SIDE ─────────────────────────────────────────────────────────

    // Build encoder for the target codec.
    // Use PCM s16le for WAV (lossless), AAC for MP4.
    let mut encoder = AudioEncoder::builder(codec_name)?
        .sample_rate(codec_params.sample_rate())?
        .channel_layout(codec_params.channel_layout().clone())?
        .sample_format(SampleFormat::find_by_name("fltp").unwrap())?
        .build()?;

    // Resolve the output container format by short name.
    let output_format = OutputFormat::find_by_name(output_format_name)
        .ok_or_else(|| FfmpegError::new("output format not found"))?;

    // MemWriter is Write + Seek — required for MP4 (moov atom rewrite at end).
    // For WAV (sequential write), IO::from_write_stream would suffice, but
    // from_seekable_write_stream works for both.
    let mem_writer = MemWriter::new();
    let io_out = IO::from_seekable_write_stream(mem_writer);

    let mut muxer = Muxer::builder()
        .add_stream(encoder.codec_parameters())?
        .build(io_out, output_format)?;

    muxer.write_header(None)?;

    // ── TRANSCODE LOOP ───────────────────────────────────────────────────────

    while let Some(packet) = demuxer.take()? {
        if packet.stream_index() != stream_index {
            continue;
        }

        decoder.push(packet)?;

        while let Some(frame) = decoder.take()? {
            encoder.push(frame)?;

            while let Some(encoded_packet) = encoder.take()? {
                muxer.write(encoded_packet)?;
            }
        }
    }

    // Flush decoder
    decoder.flush()?;
    while let Some(frame) = decoder.take()? {
        encoder.push(frame)?;
        while let Some(encoded_packet) = encoder.take()? {
            muxer.write(encoded_packet)?;
        }
    }

    // Flush encoder
    encoder.flush()?;
    while let Some(encoded_packet) = encoder.take()? {
        muxer.write(encoded_packet)?;
    }

    // Write the container trailer (moov atom for MP4, data chunk size for WAV).
    // For MP4 this seeks back to the start of the file — MemWriter handles it.
    muxer.write_trailer()?;

    // Recover the in-memory bytes.
    let bytes = muxer.into_io().into_inner().into_contents();
    Ok(bytes)
}

Fragmented MP4 note: If you want progressive output (drain bytes before the full file is ready — useful for streaming upload), set the movflags
option to frag_keyframe+empty_moov when writing the header. This eliminates the seek-back on trailer and lets you use IO::from_write_stream
instead of seekable:

use ac_ffmpeg::format::muxer::MuxerOptions;

let mut opts = MuxerOptions::new();
opts.set("movflags", "frag_keyframe+empty_moov")?;
muxer.write_header(Some(&opts))?;
// Now io_out need not be seekable — compatible with streaming S3 put_object body

---
5. Integration with the FileStore Trait

Input: get_stream() → IO::from_read_stream

FileStore::get_stream returns error_stack::Result<Box<dyn io::Read + Send>, ConverterError>. That Box<dyn io::Read + Send> satisfies the T: Read
bound on IO::from_read_stream directly:

let raw_stream: Box<dyn io::Read + Send> = self.store
    .get_stream(source_path)
    .change_context(ConverterError::ReadFailed { path: source_path.into() })?;

// No temp file. Plugs in directly.
let io = IO::from_read_stream(raw_stream);

Output: MemWriter → upload

After muxer.write_trailer(), recover the bytes and wrap in io::Cursor to satisfy Box<dyn io::Read + Send>:

let encoded_bytes = muxer.into_io().into_inner().into_contents();
let upload_reader: Box<dyn io::Read + Send> = Box::new(io::Cursor::new(encoded_bytes));

let final_url = self.store
    .upload(output_path, upload_reader)
    .change_context(ConverterError::WriteFailed { destination: output_path.into() })
    .attach_printable("check bucket permissions and available storage quota")?;

Full OpusConverter::convert Sketch

This replaces the temp-file implementation described in Feature 00's T07:

impl<S: FileStore> OpusConverter<S> {
    pub fn convert(
        &self,
        source_path: &str,
        output_path: &str,
        format: ConversionFormat,
    ) -> error_stack::Result<String, ConverterError> {
        // 1. Download as stream — no disk write
        let stream = self.store
            .get_stream(source_path)
            .change_context(ConverterError::ReadFailed { path: source_path.into() })
            .attach_printable("ensure the source path exists and is readable")?;

        // 2. Wrap in AVIOContext — zero-copy bridge into FFmpeg
        let io = IO::from_read_stream(stream);

        // 3. Demux + probe
        let mut demuxer = Demuxer::builder()
            .build(io)
            .and_then(|d| d.find_stream_info(None).map_err(|(_, e)| e))
            .change_context(ConverterError::EncodingFailed {
                input: source_path.into(),
                exit_code: -1,
            })
            .attach_printable("ffmpeg failed to probe stream info")?;

        let (stream_index, audio_stream) = demuxer
            .streams()
            .iter()
            .enumerate()
            .find(|(_, s)| s.codec_parameters().is_audio_codec())
            .ok_or_else(|| {
                error_stack::report!(ConverterError::EncodingFailed {
                    input: source_path.into(),
                    exit_code: -2,
                })
            })
            .attach_printable("no audio stream found in input")?;

        let mut decoder = AudioDecoder::from_stream(audio_stream)
            .and_then(|b| b.build())
            .change_context(ConverterError::EncodingFailed {
                input: source_path.into(),
                exit_code: -3,
            })?;

        // 4. Build encoder + muxer for target format
        let (codec_name, container_name) = format.ac_ffmpeg_names();

        let mut encoder = AudioEncoder::builder(codec_name)
            .and_then(|b| {
                b.sample_rate(audio_stream.codec_parameters().sample_rate())?
                  .channel_layout(audio_stream.codec_parameters().channel_layout().clone())?
                  .sample_format(SampleFormat::find_by_name("fltp").unwrap())?
                  .build()
            })
            .change_context(ConverterError::EncodingFailed {
                input: source_path.into(),
                exit_code: -4,
            })
            .attach_printable_lazy(|| format!("codec: {codec_name}"))?;

        let output_format = OutputFormat::find_by_name(container_name)
            .ok_or_else(|| {
                error_stack::report!(ConverterError::UnsupportedFormat {
                    format: format.extension().into(),
                })
            })?;

        let io_out = IO::from_seekable_write_stream(MemWriter::new());

        let mut muxer = Muxer::builder()
            .add_stream(encoder.codec_parameters())
            .and_then(|b| b.build(io_out, output_format))
            .change_context(ConverterError::EncodingFailed {
                input: source_path.into(),
                exit_code: -5,
            })?;

        muxer.write_header(None)
            .change_context(ConverterError::EncodingFailed {
                input: source_path.into(),
                exit_code: -6,
            })?;

        // 5. Transcode loop
        while let Some(packet) = demuxer.take()
            .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -7 })?
        {
            if packet.stream_index() != stream_index { continue; }
            decoder.push(packet)
                .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -8 })?;
            while let Some(frame) = decoder.take()
                .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -9 })?
            {
                encoder.push(frame)
                    .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -10 })?;
                while let Some(pkt) = encoder.take()
                    .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -11 })?
                {
                    muxer.write(pkt)
                        .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -12 })?;
                }
            }
        }

        // Flush decoder → encoder → muxer
        decoder.flush().ok();
        while let Some(frame) = decoder.take().ok().flatten() {
            encoder.push(frame).ok();
            while let Some(pkt) = encoder.take().ok().flatten() { muxer.write(pkt).ok(); }
        }
        encoder.flush().ok();
        while let Some(pkt) = encoder.take().ok().flatten() { muxer.write(pkt).ok(); }

        muxer.write_trailer()
            .change_context(ConverterError::EncodingFailed { input: source_path.into(), exit_code: -13 })?;

        // 6. Recover bytes and upload — no temp file ever touched disk
        let bytes = muxer.into_io().into_inner().into_contents();
        let reader: Box<dyn io::Read + Send> = Box::new(io::Cursor::new(bytes));

        self.store
            .upload(output_path, reader)
            .change_context(ConverterError::WriteFailed { destination: output_path.into() })
            .attach_printable("check bucket permissions and available storage quota")
    }
}

You also need a helper on ConversionFormat to resolve ac-ffmpeg names:

impl ConversionFormat {
    pub fn ac_ffmpeg_names(&self) -> (&'static str, &'static str) {
        match self {
            Self::MP4  => ("aac",       "mp4"),
            Self::Wave => ("pcm_s16le", "wav"),
        }
    }
}

---
6. Seekable vs Non-Seekable Sources — Decision Guide

Not all containers tolerate non-seekable input. The rule: the container format determines whether seeking is required during probe, not the codec.

┌────────────────────┬─────────────────────┬──────────────────────┬────────────────────────────────────────────────────────┐
│     Container      │ Format probe seeks? │ from_read_stream OK? │                         Notes                          │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ Ogg (Opus, Vorbis) │ No                  │ Yes                  │ Sequential by design                                   │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ Matroska / WebM    │ No (usually)        │ Yes                  │ EBML is sequential; some muxers add seek tables at end │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ MPEG-TS            │ No                  │ Yes                  │ Live-stream native format                              │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ WAV / AIFF         │ No                  │ Yes                  │ Simple RIFF headers                                    │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ MP4 / M4A / MOV    │ Yes                 │ No                   │ moov atom may be at file end; probe must seek          │
├────────────────────┼─────────────────────┼──────────────────────┼────────────────────────────────────────────────────────┤
│ MP3 / FLAC         │ Sometimes           │ Usually              │ ID3 tags can force seeks                               │
└────────────────────┴─────────────────────┴──────────────────────┴────────────────────────────────────────────────────────┘

Decision tree for FileStore implementations:

Source URL scheme
    ├── gs:// or s3://
    │     ├── Container is Ogg/TS/MKV/WAV  →  from_read_stream  (no range requests needed)
    │     └── Container is MP4/M4A         →  buffer into Cursor<Vec<u8>>
    │                                          (fetch the full object, wrap in Cursor,
    │                                           use from_seekable_read_stream)
    └── local path
          →  File::open gives Read + Seek  →  from_seekable_read_stream  (always safe)

Buffering strategy for MP4 sources from cloud:

// When you know the content is MP4 and the store is cloud-backed:
let mut buf = Vec::new();
self.store
    .get_stream(source_path)?
    .read_to_end(&mut buf)
    .change_context(ConverterError::ReadFailed { path: source_path.into() })?;

let io = IO::from_seekable_read_stream(io::Cursor::new(buf));

Only do this when the source is known to be MP4 and the file size is bounded and acceptable to hold in memory. For DE-4295 the primary source is
Opus (Ogg container), so from_read_stream is the default path and buffering is the fallback.

FileStore trait extension (optional):

If you later need to support seekable sources generically, add an optional method with a default impl:

pub trait FileStore {
    fn get_stream(&self, path: &str)
        -> error_stack::Result<Box<dyn io::Read + Send>, ConverterError>;

    fn get_seekable_stream(&self, path: &str)
        -> error_stack::Result<Box<dyn io::Read + io::Seek + Send>, ConverterError>
    {
        // Default: buffer the whole thing — works for any backend
        let mut buf = Vec::new();
        self.get_stream(path)?.read_to_end(&mut buf)
            .change_context(ConverterError::ReadFailed { path: path.into() })?;
        Ok(Box::new(io::Cursor::new(buf)))
    }

    fn upload(&self, dest_path: &str, data: Box<dyn io::Read + Send>)
        -> error_stack::Result<String, ConverterError>;
}

---
7. Cargo Dependencies and Compile Notes

Add to packages/ffmpeg-converter/Cargo.toml

[dependencies]
# Replace or augment ffmpeg-sys-next with ac-ffmpeg.
# ac-ffmpeg uses ffmpeg-sys-next internally — both can coexist
# during a migration, but ac-ffmpeg covers the full pipeline
# without raw unsafe C.
ac-ffmpeg = "0.18"          # check crates.io for latest

# Keep tempfile only if retaining the temp-file fallback path
# tempfile = "3"

# Other deps unchanged from Feature 00 spec
derive_more   = { version = "1", features = ["display", "error", "from"] }
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"
error-stack   = "0.5"
tokio         = { version = "1", features = ["full"] }
aws-sdk-s3    = { version = "1" }
google-cloud-storage = { version = "0.20" }

System Library Requirements

ac-ffmpeg links against the same system libraries as ffmpeg-sys-next. On the build host:

# macOS
brew install ffmpeg

# Ubuntu / Debian
apt-get install -y \
  libavcodec-dev libavformat-dev libavutil-dev \
  libswresample-dev libswscale-dev pkg-config

The Docker build image for Lambda must include these. Add to the Dockerfile:

RUN apt-get install -y \
  libavcodec-dev libavformat-dev libavutil-dev \
  libswresample-dev libswscale-dev pkg-config

Feature Flags

ac-ffmpeg has no mandatory feature flags — all codec and format support is enabled by default and is determined by what FFmpeg was compiled with
on the host.

Coexistence with ffmpeg-sys-next

Both crates link against the same libav* system libraries. They can coexist in the same binary. During migration you can use ac-ffmpeg for the
streaming pipeline (decode → encode → mux) and keep any existing ffmpeg-sys-next code for operations not yet migrated. Linking will not conflict
as long as both crates see the same installed FFmpeg version via pkg-config.

Verification Checklist

After implementing Feature 00 with ac-ffmpeg:

- cargo build -p ffmpeg-converter compiles with no warnings
- cargo clippy -p ffmpeg-converter -- -D warnings passes
- Code snippets in this document compile if pasted into the crate with use ac_ffmpeg::* in scope
- OpusConverter::convert runs against a real .opus file (Ogg container) without touching disk
- OpusConverter::convert produces a valid .mp4 and .wav output
- Update requirements.md front matter: has_fundamentals: true
