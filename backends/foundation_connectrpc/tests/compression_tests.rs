//! Compression + buffer-pool tests (spec-41 F14 / Decision 06): gzip round-trip,
//! size limits (compress_min / send_max / read_max bomb guard), negotiation, the
//! reset-able streaming form, and the origin-return buffer pool.

use std::sync::Arc;
use std::thread;

use foundation_connectrpc::shared::compression::GzipStreamCompressor;
use foundation_connectrpc::{
    negotiate_compression, BufferPool, CompressionError, CompressionRegistry, Compressor,
    GzipCompressor, SizeLimits,
};

// ── gzip ──────────────────────────────────────────────────────────────────────

#[test]
fn gzip_roundtrip() {
    let gz = GzipCompressor::default();
    let data = b"the quick brown fox jumps over the lazy dog".repeat(20);
    let compressed = gz.compress(&data).unwrap();
    assert!(compressed.len() < data.len(), "should shrink repetitive data");
    let back = gz.decompress(&compressed, 0).unwrap();
    assert_eq!(back, data);
    assert_eq!(gz.name(), "gzip");
}

#[test]
fn gzip_decompress_bomb_is_bounded_by_read_max() {
    let gz = GzipCompressor::default();
    // 1 MiB of zeros compresses to a tiny payload — a classic bomb shape.
    let bomb = gz.compress(&vec![0u8; 1_000_000]).unwrap();
    let err = gz.decompress(&bomb, 4096).unwrap_err();
    assert!(matches!(err, CompressionError::TooLarge { limit: 4096, .. }));
    // Unlimited (0) decompresses fully.
    assert_eq!(gz.decompress(&bomb, 0).unwrap().len(), 1_000_000);
}

#[test]
fn gzip_stream_compressor_reused_across_messages() {
    let gz = GzipCompressor::default();
    let mut stream = GzipStreamCompressor::default();

    stream.write(b"first message").unwrap();
    let a = stream.finish_message().unwrap();
    stream.write(b"second message").unwrap();
    let b = stream.finish_message().unwrap();

    assert_eq!(gz.decompress(&a, 0).unwrap(), b"first message");
    assert_eq!(gz.decompress(&b, 0).unwrap(), b"second message");
}

// ── SizeLimits (H10) ──────────────────────────────────────────────────────────

#[test]
fn size_limits_compress_min_and_send_max() {
    let limits = SizeLimits {
        read_max_bytes: 0,
        send_max_bytes: 100,
        compress_min_bytes: 50,
    };
    assert!(!limits.should_compress(10), "below compress_min: skip");
    assert!(limits.should_compress(50), "at compress_min: compress");

    assert!(limits.check_send(100).is_ok());
    let err = limits.check_send(101).err().expect("over send_max");
    assert_eq!(
        err.current_context().code(),
        foundation_connectrpc::Code::ResourceExhausted
    );

    // Defaults are unlimited (connect-go parity).
    let unlimited = SizeLimits::default();
    assert!(unlimited.should_compress(0));
    assert!(unlimited.check_send(usize::MAX).is_ok());
}

// ── Registry + negotiation ────────────────────────────────────────────────────

#[test]
fn registry_defaults_to_gzip() {
    let reg = CompressionRegistry::new();
    assert!(reg.get("gzip").is_some());
    assert!(reg.supports("identity"));
    assert!(reg.supports("gzip"));
    assert!(!reg.supports("br"));
    assert_eq!(reg.accept_encoding_header(), "gzip");
}

#[test]
fn registry_register_and_remove() {
    let mut reg = CompressionRegistry::empty();
    assert_eq!(reg.accept_encoding_header(), "");
    reg.register(Arc::new(GzipCompressor::default()));
    assert_eq!(reg.accept_encoding_header(), "gzip");
    reg.remove("gzip");
    assert!(reg.get("gzip").is_none());
}

#[test]
fn negotiate_request_and_response() {
    let reg = CompressionRegistry::new();

    // Request declares gzip → we get a decompressor; client accepts gzip → compressor.
    let n = negotiate_compression(&reg, Some("gzip"), Some("gzip, identity")).unwrap();
    assert!(n.request_decompressor.is_some());
    assert!(n.response_compressor.is_some());

    // No encodings → identity both ways.
    let n = negotiate_compression(&reg, None, None).unwrap();
    assert!(n.request_decompressor.is_none());
    assert!(n.response_compressor.is_none());

    // Unknown accept token falls back to the request encoding.
    let n = negotiate_compression(&reg, Some("gzip"), Some("br")).unwrap();
    assert!(n.response_compressor.is_some());

    // Unsupported request encoding is an error (Unimplemented).
    let err = negotiate_compression(&reg, Some("br"), None).err().expect("unsupported");
    assert_eq!(
        err.current_context().code(),
        foundation_connectrpc::Code::Unimplemented
    );
}

// ── Buffer pool ───────────────────────────────────────────────────────────────

#[test]
fn buffer_pool_recycles_locally() {
    let mut pool = BufferPool::new();
    let mut buf = pool.get();
    buf.extend_from_slice(&[1, 2, 3]);
    pool.put(buf);
    assert_eq!(pool.free_list_len(), 1);
    // Next get reuses it, cleared.
    let reused = pool.get();
    assert!(reused.is_empty());
    assert_eq!(pool.free_list_len(), 0);
}

#[test]
fn frozen_frame_final_drop_returns_to_origin_pool_across_threads() {
    let mut pool = BufferPool::new();

    // Fill a buffer past the fresh-allocation capacity, then freeze it.
    let mut buf = pool.get();
    buf.extend_from_slice(&[7u8; 4096]);
    let frozen = pool.freeze(buf);
    assert_eq!(&frozen[..8], &[7u8; 8]);
    assert_eq!(pool.pending_returns(), 0, "still alive, not yet returned");

    // Drop the last reference on ANOTHER thread — the allocation must still come
    // home to the origin pool's return lane.
    let handle = thread::spawn(move || {
        let _f = frozen; // dropped at end of the closure, on this worker
    });
    handle.join().unwrap();

    assert_eq!(pool.pending_returns(), 1, "final drop returned to origin lane");

    // get() drains the lane and hands back the recycled allocation.
    let recycled = pool.get();
    assert!(
        recycled.capacity() >= 4096,
        "recycled the original allocation (cap {}), not a fresh 512-byte buffer",
        recycled.capacity()
    );
    assert_eq!(pool.pending_returns(), 0);
}
