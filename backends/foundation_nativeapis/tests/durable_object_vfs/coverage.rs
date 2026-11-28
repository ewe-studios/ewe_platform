//! Coverage tests for the S3 and R2 object-store virtual file system (VFS).
//!
//! Each test drives the public VFS and object-store application programming
//! interfaces (APIs). No test uses a network.

use crate::support::{fixture, r2_fixture, stream, Fixture};
use core::time::Duration;
use foundation_core::durable::{
    ContentType, DurableStreamVfs, Expiry, ForkInfo, ProducerEpochState, ProducerId, StreamSeq,
    VfsError, VfsFuture, VfsMeta, VfsOffset, WriterState,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_nativeapis::native::vfs::s3::{S3Settings, S3Vfs};
use foundation_nativeapis::shared::vfs::durable::head::{
    head_key, is_segment_key, parse_head_key, segment_key, HeadManifest, SegmentEntry,
};
use foundation_nativeapis::shared::vfs::durable::{
    CasMode, MemoryFaults, ObjectLayout, ObjectStore, ObjectVfsConfig, Provider, PublishPolicy,
    StoreCapabilities,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn run<T>(future: VfsFuture<'_, T>) -> Result<T, VfsError> {
    block_on_future_scoped(future)
}

fn empty_meta(content_type: &str) -> VfsMeta {
    VfsMeta {
        content_type: ContentType::parse(content_type).expect("content type"),
        len: 0,
        closed: false,
        expires: None,
        writer: WriterState::default(),
        fork: None,
        last_access_ms: 0,
        trim_from: 0,
        version: 0,
    }
}

fn flush(
    fixture: &Fixture,
    name: &str,
) -> foundation_nativeapis::shared::vfs::durable::FlushReport {
    block_on_future_scoped(fixture.vfs.flush_stream(&stream(name))).expect("flush")
}

fn read(fixture: &Fixture, name: &str, at: u64, limit: usize) -> Vec<u8> {
    run(fixture.vfs.read_at(&stream(name), &VfsOffset(at), limit)).expect("read")
}

fn append(fixture: &Fixture, name: &str, bytes: &[u8]) {
    run(fixture.vfs.append(&stream(name), bytes)).expect("append");
}

fn create(fixture: &Fixture, name: &str) {
    run(fixture
        .vfs
        .create(&stream(name), empty_meta("application/octet-stream")))
    .expect("create");
}

fn segment_keys(fixture: &Fixture) -> Vec<String> {
    fixture
        .store
        .keys()
        .into_iter()
        .filter(|key| is_segment_key(key))
        .collect()
}

#[test]
fn one_put_per_batch() {
    let fixture = fixture("one_put", ObjectVfsConfig::default());
    let name = "batch/one";
    create(&fixture, name);
    append(&fixture, name, &[7_u8; 1000]);
    fixture.store.reset_stats();
    let report = flush(&fixture, name);
    assert_eq!(report.blocks_put, 1, "one local batch sends one object PUT");
    assert_eq!(
        segment_keys(&fixture).len(),
        1,
        "the store holds one segment for the batch"
    );
    assert_eq!(report.bytes_put, 1000);
    assert_eq!(read(&fixture, name, 0, 2000), vec![7_u8; 1000]);
    assert_eq!(
        run(fixture.vfs.meta(&stream(name))).expect("meta").len,
        1000
    );
}

#[test]
fn append_only_batches_locally() {
    let config = ObjectVfsConfig::default().with_layout(ObjectLayout::AppendOnly);
    let fixture = fixture("append_only_batch", config);
    let name = "batch/append";
    create(&fixture, name);
    append(&fixture, name, b"append-only bytes");
    fixture.store.reset_stats();
    let report = flush(&fixture, name);
    assert_eq!(report.blocks_put, 1);
    let keys = segment_keys(&fixture);
    assert_eq!(keys.len(), 1);
    assert!(
        keys[0].contains("/log/"),
        "the key uses the append-only layout"
    );
    assert_eq!(read(&fixture, name, 0, 64), b"append-only bytes");
}

#[test]
fn large_batch_uses_multipart() {
    let config = ObjectVfsConfig::default()
        .with_segment_target_bytes(4096)
        .with_max_block_bytes(4096)
        .with_max_single_put_bytes(64)
        .with_multipart_part_bytes(64);
    let fixture = fixture("multipart", config);
    let name = "batch/multipart";
    create(&fixture, name);
    append(&fixture, name, &[3_u8; 512]);
    fixture.store.reset_stats();
    let report = flush(&fixture, name);
    assert_eq!(report.blocks_put, 1);
    let stats = fixture.store.stats();
    assert!(stats.part_puts > 1, "a large batch uses multipart PUTs");
    assert!(stats.max_parts > 1);
    assert_eq!(read(&fixture, name, 0, 1024), vec![3_u8; 512]);
}

#[test]
fn read_spans_segments() {
    let config = ObjectVfsConfig::default()
        .with_segment_target_bytes(8)
        .with_max_block_bytes(8);
    let fixture = fixture("spans", config);
    let name = "read/spans";
    create(&fixture, name);
    append(&fixture, name, b"0123456789abcdefghij");
    let report = flush(&fixture, name);
    assert_eq!(report.blocks_put, 3, "the batch splits into three objects");
    fixture.store.reset_stats();
    assert_eq!(read(&fixture, name, 0, 64), b"0123456789abcdefghij");
    assert!(
        fixture.store.stats().range_gets > 1,
        "a cross-segment read uses range GETs"
    );
}

#[test]
fn read_is_bounded() {
    let fixture = fixture("bounded", ObjectVfsConfig::default());
    let name = "read/bounded";
    create(&fixture, name);
    append(&fixture, name, b"0123456789abcdefghij");
    flush(&fixture, name);
    for page in [1_usize, 3, 7] {
        let mut cursor = 0_u64;
        let mut out = Vec::new();
        for _ in 0..64 {
            let bytes = read(&fixture, name, cursor, page);
            assert!(bytes.len() <= page, "a page is bounded by {page} bytes");
            if bytes.is_empty() {
                break;
            }
            cursor = cursor.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            out.extend_from_slice(&bytes);
            if cursor >= 20 {
                break;
            }
        }
        assert_eq!(out, b"0123456789abcdefghij", "page {page} reassembles");
    }
}

#[test]
fn head_cache_is_refreshed() {
    let config = ObjectVfsConfig::default().with_head_cache_ms(60_000);
    let fixture = fixture("head_cache", config);
    let name = "cache/head";
    create(&fixture, name);
    append(&fixture, name, b"one");
    flush(&fixture, name);
    assert_eq!(read(&fixture, name, 0, 64), b"one");

    let second = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture
                .config
                .clone()
                .with_spool_root(fixture.temp.path().join("spool-b")),
            S3Settings::default(),
        )
        .expect("second vfs"),
    );
    run(second.append(&stream(name), b"two")).expect("second append");
    block_on_future_scoped(second.flush_stream(&stream(name))).expect("second flush");

    assert_eq!(
        read(&fixture, name, 0, 64),
        b"onetwo",
        "the read revalidates the head after the GET"
    );
}

#[test]
fn cache_fill_is_deduplicated() {
    let fixture = fixture("cache_dedup", ObjectVfsConfig::default());
    let name = "cache/dedup";
    create(&fixture, name);
    append(&fixture, name, &[9_u8; 100]);
    flush(&fixture, name);
    let first = read(&fixture, name, 10, 20);
    let after_first = fixture.store.stats().range_gets;
    let second = read(&fixture, name, 10, 20);
    let after_second = fixture.store.stats().range_gets;
    assert_eq!(first, second);
    assert_eq!(
        after_first, after_second,
        "a repeated range is served from the cache"
    );
}

#[test]
fn missing_object_is_an_error() {
    let fixture = fixture("missing", ObjectVfsConfig::default());
    let name = "missing/segment";
    create(&fixture, name);
    append(&fixture, name, b"durable bytes");
    flush(&fixture, name);
    let key = segment_keys(&fixture).remove(0);
    block_on_future_scoped(fixture.store.delete(&key)).expect("delete segment");
    let error = run(fixture.vfs.read_at(&stream(name), &VfsOffset(0), 64));
    assert!(
        matches!(error, Err(VfsError::Io(_))),
        "a missing object for a live head is an error, got {error:?}"
    );
}

#[test]
fn two_writers_one_cas_wins() {
    let fixture = fixture("two_writers", ObjectVfsConfig::default());
    let name = "writers/two";
    create(&fixture, name);
    append(&fixture, name, b"aaa");
    flush(&fixture, name);

    let second = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture
                .config
                .clone()
                .with_spool_root(fixture.temp.path().join("spool-b")),
            S3Settings::default(),
        )
        .expect("second vfs"),
    );
    run(second.append(&stream(name), b"bbb")).expect("append b");
    fixture
        .store
        .set_fault(|faults| faults.fail_next_put_if_match = true);
    let report = block_on_future_scoped(second.flush_stream(&stream(name))).expect("flush b");
    assert_eq!(report.retries, 1, "the losing CAS retries");
    assert_eq!(read(&fixture, name, 0, 64), b"aaabbb");
    assert_eq!(segment_keys(&fixture).len(), 2, "both segments are kept");
}

#[test]
fn cas_retry_preserves_both_segments() {
    let fixture = fixture("cas_retry", ObjectVfsConfig::default());
    let name = "writers/retry";
    create(&fixture, name);
    append(&fixture, name, b"first");
    flush(&fixture, name);
    let second = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture
                .config
                .clone()
                .with_spool_root(fixture.temp.path().join("spool-b")),
            S3Settings::default(),
        )
        .expect("second vfs"),
    );
    run(second.append(&stream(name), b"-second")).expect("append b");
    fixture
        .store
        .set_fault(|faults| faults.fail_next_put_if_match = true);
    block_on_future_scoped(second.flush_stream(&stream(name))).expect("flush b");
    assert_eq!(read(&fixture, name, 0, 64), b"first-second");
}

#[test]
fn spool_survives_crash() {
    let fixture = fixture("crash", ObjectVfsConfig::default());
    let name = "crash/spool";
    create(&fixture, name);
    append(&fixture, name, b"acknowledged");
    // A new VFS over the same spool and store sees the acknowledged bytes.
    let restarted = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture.config.clone(),
            S3Settings::default(),
        )
        .expect("restarted vfs"),
    );
    assert_eq!(read(&fixture, name, 0, 64), b"acknowledged");
    let meta = run(restarted.meta(&stream(name))).expect("meta");
    assert_eq!(meta.len, 12);
    assert_eq!(
        run(restarted.read_at(&stream(name), &VfsOffset(0), 64)).expect("restart read"),
        b"acknowledged"
    );
}

#[test]
fn restart_after_segment_put() {
    let fixture = fixture("restart_put", ObjectVfsConfig::default());
    let name = "crash/segment";
    create(&fixture, name);
    append(&fixture, name, &[5_u8; 100]);
    fixture
        .store
        .set_fault(|faults| faults.fail_next_head_cas = true);
    let failed = block_on_future_scoped(fixture.vfs.flush_stream(&stream(name)));
    assert!(failed.is_err(), "the head CAS failed after the segment PUT");
    let keys_after_put = segment_keys(&fixture);
    assert_eq!(
        keys_after_put.len(),
        1,
        "the sealed segment is on the store"
    );
    assert_eq!(
        fixture
            .vfs
            .inner()
            .spool()
            .pending_len(&stream(name))
            .expect("pending"),
        100,
        "the spool keeps the acknowledged bytes"
    );

    let restarted = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture.config.clone(),
            S3Settings::default(),
        )
        .expect("restarted vfs"),
    );
    let report = block_on_future_scoped(restarted.flush_stream(&stream(name))).expect("replay");
    assert_eq!(report.blocks_put, 1);
    assert_eq!(
        segment_keys(&fixture).len(),
        1,
        "the replay reuses the same content-addressed key"
    );
    assert_eq!(read(&fixture, name, 0, 200), vec![5_u8; 100]);
}

#[test]
fn restart_from_objects() {
    let fixture = fixture("restart_objects", ObjectVfsConfig::default());
    let name = "restart/objects";
    create(&fixture, name);
    append(&fixture, name, b"from objects");
    flush(&fixture, name);
    // A fresh spool forces the read path to fetch the segment objects.
    let restarted = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture
                .config
                .clone()
                .with_spool_root(fixture.temp.path().join("fresh-spool")),
            S3Settings::default(),
        )
        .expect("restarted vfs"),
    );
    assert_eq!(
        run(restarted.read_at(&stream(name), &VfsOffset(0), 64)).expect("read"),
        b"from objects"
    );
}

#[test]
fn compaction_keeps_bytes_exact() {
    let config = ObjectVfsConfig::default()
        .with_segment_target_bytes(32)
        .with_max_block_bytes(32);
    let fixture = fixture("compact", config);
    let name = "compact/bytes";
    create(&fixture, name);
    for chunk in [b"0123".as_slice(), b"4567", b"89ab", b"cdef"] {
        append(&fixture, name, chunk);
        flush(&fixture, name);
    }
    let before = read(&fixture, name, 0, 64);
    let report = block_on_future_scoped(fixture.vfs.compact(&stream(name))).expect("compact");
    assert!(report.merged > 0, "small adjacent segments merge");
    assert!(report.objects_written > 0);
    assert_eq!(
        read(&fixture, name, 0, 64),
        before,
        "compaction is byte exact"
    );
}

#[test]
fn retention_drops_old_segments() {
    let config = ObjectVfsConfig::default()
        .with_segment_target_bytes(8)
        .with_max_block_bytes(8);
    let fixture = fixture("retention", config);
    let name = "retention/segments";
    create(&fixture, name);
    append(&fixture, name, b"0123456789abcdefghijklmnop");
    flush(&fixture, name);
    run(fixture.vfs.trim(&stream(name), VfsOffset(8))).expect("trim");
    let old_keys = segment_keys(&fixture).len();
    assert_eq!(old_keys, 4);
    fixture.clock.advance_wall(Duration::from_hours(25));
    let report = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name)))).expect("gc");
    assert!(report.deleted >= 1, "the trimmed segment is removed");
    assert_eq!(read(&fixture, name, 8, 64), b"89abcdefghijklmnop");
    assert!(matches!(
        run(fixture.vfs.read_at(&stream(name), &VfsOffset(0), 8)),
        Err(VfsError::Trimmed)
    ));
}

#[test]
fn orphan_segment_is_removed() {
    let fixture = fixture("orphan", ObjectVfsConfig::default());
    let name = "orphan/segment";
    create(&fixture, name);
    append(&fixture, name, b"live");
    flush(&fixture, name);
    let key = format!("{}/seg/orphan.log", prefix_of(name));
    block_on_future_scoped(fixture.store.put(&key, b"orphan".to_vec())).expect("orphan put");
    fixture.clock.advance_wall(Duration::from_hours(25));
    let report = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name)))).expect("gc");
    assert_eq!(report.deleted, 1);
    assert!(!fixture.store.contains(&key), "the orphan is removed");
    assert_eq!(
        read(&fixture, name, 0, 64),
        b"live",
        "the live segment stays"
    );
}

fn prefix_of(name: &str) -> String {
    use core::fmt::Write as _;
    let mut hex = String::new();
    for byte in name.as_bytes() {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("durable/{hex}")
}

#[test]
fn orphan_of_a_deleted_stream_is_removed() {
    let fixture = fixture("orphan_deleted", ObjectVfsConfig::default());
    let name = "orphan/deleted";
    create(&fixture, name);
    append(&fixture, name, b"orphan bytes");
    flush(&fixture, name);
    let head = head_key("durable", &stream(name));
    block_on_future_scoped(fixture.store.delete(&head)).expect("delete head");
    let keys = segment_keys(&fixture);
    assert_eq!(keys.len(), 1);
    fixture.clock.advance_wall(Duration::from_hours(25));
    let report = block_on_future_scoped(fixture.vfs.gc(None)).expect("gc");
    assert!(report.deleted >= 1);
    assert!(!fixture.store.contains(&keys[0]));
}

#[test]
fn never_delete_a_live_object() {
    let fixture = fixture("live", ObjectVfsConfig::default());
    let name = "lifecycle/live";
    create(&fixture, name);
    append(&fixture, name, b"live bytes");
    flush(&fixture, name);
    let keys = segment_keys(&fixture);
    fixture.clock.advance_wall(Duration::from_hours(25));
    let report = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name)))).expect("gc");
    assert_eq!(report.deleted, 0, "a live object is never deleted");
    assert_eq!(report.live, keys.len());
    for key in &keys {
        assert!(fixture.store.contains(key));
    }
    assert_eq!(read(&fixture, name, 0, 64), b"live bytes");
}

#[test]
fn retired_object_waits_for_grace() {
    let fixture = fixture("retired", ObjectVfsConfig::default());
    let name = "lifecycle/retired";
    create(&fixture, name);
    append(&fixture, name, b"retired bytes");
    flush(&fixture, name);
    let keys = segment_keys(&fixture);
    run(fixture.vfs.remove(&stream(name))).expect("remove");
    let first = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name)))).expect("gc");
    assert_eq!(
        first.deleted, 0,
        "a retired object waits for the grace window"
    );
    assert_eq!(first.retired, keys.len());
    assert_eq!(
        block_on_future_scoped(fixture.vfs.inner().lifecycle(&stream(name), &keys[0]))
            .expect("lifecycle"),
        foundation_nativeapis::shared::vfs::durable::ObjectLifecycle::Retired
    );
    fixture.clock.advance_wall(Duration::from_hours(25));
    let second = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name)))).expect("gc");
    assert_eq!(second.deleted, keys.len());
    assert_eq!(
        block_on_future_scoped(fixture.vfs.inner().lifecycle(&stream(name), &keys[0]))
            .expect("lifecycle"),
        foundation_nativeapis::shared::vfs::durable::ObjectLifecycle::Deleted
    );
}

#[test]
fn partial_delete_error_is_reported() {
    let fixture = fixture("delete_error", ObjectVfsConfig::default());
    let name = "lifecycle/delete";
    create(&fixture, name);
    append(&fixture, name, b"delete me");
    flush(&fixture, name);
    let key = segment_keys(&fixture).remove(0);
    let mut faults = MemoryFaults::default();
    faults.fail_delete_keys.insert(key.clone());
    fixture.store.set_faults(faults);
    run(fixture.vfs.remove(&stream(name))).expect("remove");
    fixture.clock.advance_wall(Duration::from_hours(25));
    let error = block_on_future_scoped(fixture.vfs.gc(Some(&stream(name))));
    assert!(
        matches!(error, Err(VfsError::Io(_))),
        "a partial delete fails the pass"
    );
    assert!(fixture.store.contains(&key), "the failed key remains");
}

#[test]
fn provider_capabilities_are_recorded() {
    let fixture = fixture("capabilities", ObjectVfsConfig::default());
    let capabilities = fixture.vfs.provider_capabilities();
    assert_eq!(capabilities.provider, Provider::S3);
    assert_eq!(capabilities.cas, CasMode::ConditionalPut);
    assert!(capabilities.store.conditional_write);
    assert_eq!(capabilities.settings.region, "us-east-1");

    let probe = block_on_future_scoped(fixture.vfs.probe()).expect("probe");
    assert!(probe.conditional_write);

    let weak = StoreCapabilities {
        conditional_write: false,
        ..StoreCapabilities::default()
    };
    fixture.store.set_capabilities(weak);
    fixture
        .store
        .set_fault(|faults| faults.ignore_if_match = true);
    let probe = block_on_future_scoped(fixture.vfs.probe()).expect("probe");
    assert!(!probe.conditional_write);
    assert_eq!(fixture.vfs.provider_capabilities().cas, CasMode::Lease);
}

#[test]
fn weak_cas_fallback_is_documented() {
    let fixture = fixture("weak_cas", ObjectVfsConfig::default());
    let weak = StoreCapabilities {
        conditional_write: false,
        ..StoreCapabilities::default()
    };
    fixture.store.set_capabilities(weak);
    fixture
        .store
        .set_fault(|faults| faults.ignore_if_match = true);
    let name = "weak/cas";
    create(&fixture, name);
    append(&fixture, name, b"aaa");
    flush(&fixture, name);
    assert_eq!(
        fixture.vfs.provider_capabilities().cas,
        CasMode::Lease,
        "a store without conditional writes uses the lease fallback"
    );
    let second = Arc::new(
        S3Vfs::new(
            fixture.store.clone(),
            &fixture.ctx,
            fixture
                .config
                .clone()
                .with_spool_root(fixture.temp.path().join("spool-b")),
            S3Settings::default(),
        )
        .expect("second vfs"),
    );
    run(second.append(&stream(name), b"bbb")).expect("append b");
    block_on_future_scoped(second.flush_stream(&stream(name))).expect("flush b");
    assert_eq!(read(&fixture, name, 0, 64), b"aaabbb");
}

#[test]
fn r2_uses_auto_region() {
    let (r2, _, _temp) = r2_fixture("r2_region");
    let settings = r2.settings();
    assert_eq!(settings.region, "auto");
    assert!(settings.path_style);
    assert!(settings.crc32);
    let capabilities = r2.provider_capabilities();
    assert_eq!(capabilities.provider, Provider::R2);
    assert_eq!(capabilities.settings.region, "auto");
    assert!(capabilities.settings.path_style);
    assert!(capabilities.settings.crc32);
}

#[test]
fn r2_reports_conditional_write_support() {
    let (r2, store, _temp) = r2_fixture("r2_conditional");
    let probe = block_on_future_scoped(r2.probe()).expect("probe");
    assert!(probe.conditional_write);
    assert_eq!(r2.provider_capabilities().cas, CasMode::ConditionalPut);

    let weak = StoreCapabilities {
        conditional_write: false,
        ..StoreCapabilities::default()
    };
    store.set_capabilities(weak);
    store.set_fault(|faults| faults.ignore_if_match = true);
    let probe = block_on_future_scoped(r2.probe()).expect("probe");
    assert!(!probe.conditional_write);
    assert_eq!(r2.provider_capabilities().cas, CasMode::Lease);
}

#[test]
fn head_manifest_round_trips() {
    let mut producers = BTreeMap::new();
    producers.insert(
        ProducerId::parse("producer-1").expect("producer"),
        ProducerEpochState {
            epoch: 2,
            last_seq: 7,
        },
    );
    let manifest = HeadManifest {
        content_type: Some(ContentType::parse("application/json").expect("content type")),
        len: 42,
        closed: true,
        expires: Some(Expiry::Ttl(Duration::from_millis(1500))),
        last_access_ms: 9,
        trim_from: 3,
        version: 4,
        ref_count: 1,
        soft_deleted: false,
        fork: Some(ForkInfo {
            source: stream("manifest/source"),
            at: 5,
        }),
        producers,
        stream_seq: Some(StreamSeq::from_bytes(vec![1, 2])),
        segments: vec![SegmentEntry {
            at: 5,
            len: 10,
            key: "k".to_string(),
            batch: "b".to_string(),
        }],
        published_own: 47,
    };
    let text = manifest.render();
    let parsed = HeadManifest::parse(&text).expect("parse");
    assert_eq!(parsed, manifest);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        &lines[..9],
        &[
            "content_type\tapplication/json",
            "len\t42",
            "closed\t1",
            "expires\tttl:1500",
            "last_access_ms\t9",
            "trim_from\t3",
            "version\t4",
            "ref_count\t1",
            "soft_deleted\t0",
        ],
        "the first nine lines use the file VFS metadata format"
    );
    assert!(text.contains("format\ts3-vfs-1"));
}

#[test]
fn both_layouts_have_the_same_keys() {
    let name = "layout/keys";
    let segmented = fixture("keys_seg", ObjectVfsConfig::default());
    create(&segmented, name);
    append(&segmented, name, b"layout bytes");
    flush(&segmented, name);
    let append_fixture = fixture(
        "keys_append",
        ObjectVfsConfig::default().with_layout(ObjectLayout::AppendOnly),
    );
    create(&append_fixture, name);
    append(&append_fixture, name, b"layout bytes");
    flush(&append_fixture, name);

    let head = head_key("durable", &stream(name));
    assert!(segmented.store.contains(&head));
    assert!(
        append_fixture.store.contains(&head),
        "the head key is shared"
    );
    assert!(segment_keys(&segmented)[0].contains("/seg/"));
    assert!(segment_keys(&append_fixture)[0].contains("/log/"));
    assert_eq!(
        read(&segmented, name, 0, 64),
        read(&append_fixture, name, 0, 64)
    );
    assert_eq!(parse_head_key("durable", &head), Some(stream(name)));
    assert!(
        segment_key("durable", &stream(name), ObjectLayout::Segmented, 0, 5, "b").contains("/seg/")
    );
}

#[test]
fn flush_measures_put_count_and_throughput() {
    let defaults = ObjectVfsConfig::default();
    assert_eq!(defaults.segment_target_bytes, 256 * 1024);
    assert_eq!(defaults.max_block_bytes, 1024 * 1024);
    assert_eq!(defaults.batch_age, Duration::from_secs(5));
    assert_eq!(defaults.parallel_puts, 4);
    assert_eq!(defaults.retry_bound, 3);
    assert_eq!(defaults.publish_policy, PublishPolicy::Batched);
    assert_eq!(defaults.layout, ObjectLayout::Segmented);

    let fixture = fixture("throughput", defaults);
    let name = "perf/batch";
    create(&fixture, name);
    let bytes = vec![1_u8; 256 * 1024];
    append(&fixture, name, &bytes);
    let report = flush(&fixture, name);
    assert_eq!(report.blocks_put, 1, "a 256 KiB batch is one object");
    assert_eq!(report.bytes_put, 256 * 1024);
    assert_eq!(read(&fixture, name, 0, 256 * 1024), bytes);
}

#[test]
fn batched_flush_waits_for_the_age() {
    let config = ObjectVfsConfig::default()
        .with_batch_age(Duration::from_secs(5))
        .with_publish_policy(PublishPolicy::Batched);
    let fixture = fixture("batch_age", config);
    let name = "batch/age";
    create(&fixture, name);
    append(&fixture, name, b"waits for the age");
    let first = block_on_future_scoped(fixture.vfs.flush()).expect("flush");
    assert_eq!(first.blocks_put, 0, "a young batch waits");
    assert_eq!(segment_keys(&fixture).len(), 0);
    fixture.clock.advance_wall(Duration::from_secs(5));
    let second = block_on_future_scoped(fixture.vfs.flush()).expect("flush");
    assert_eq!(second.blocks_put, 1, "the age makes the batch eligible");
    assert_eq!(second.streams_flushed, 1);
    assert_eq!(read(&fixture, name, 0, 64), b"waits for the age");
}

#[test]
fn flush_publishes_several_streams() {
    let config = ObjectVfsConfig::default()
        .with_batch_age(Duration::ZERO)
        .with_parallel_puts(2);
    let fixture = fixture("parallel", config);
    for index in 0..5 {
        let name = format!("parallel/{index}");
        create(&fixture, &name);
        append(&fixture, &name, format!("stream-{index}").as_bytes());
    }
    let report = block_on_future_scoped(fixture.vfs.flush()).expect("flush");
    assert_eq!(report.streams_flushed, 5);
    assert_eq!(report.blocks_put, 5);
    for index in 0..5 {
        let name = format!("parallel/{index}");
        assert_eq!(
            read(&fixture, &name, 0, 64),
            format!("stream-{index}").as_bytes()
        );
    }
}

#[test]
fn s3_vfs_coverage() {
    let fixture = fixture("s3_coverage", ObjectVfsConfig::default());
    exercise(fixture.vfs.as_ref());
}

#[test]
fn r2_vfs_coverage() {
    let (r2, _store, _temp) = r2_fixture("r2_coverage");
    exercise(r2.as_ref());
}

fn exercise(vfs: &dyn DurableStreamVfs) {
    let name = stream("coverage/all");
    let meta = empty_meta("text/plain");
    assert!(run(vfs.create(&name, meta.clone())).expect("create"));
    assert!(!run(vfs.create(&name, meta.clone())).expect("idempotent create"));
    let current = run(vfs.meta(&name)).expect("meta");
    assert_eq!(
        run(vfs.append_with_meta(&name, b"abc", meta.clone(), current.version)).expect("append"),
        VfsOffset(3)
    );
    assert_eq!(
        run(vfs.read_at(&name, &VfsOffset(1), 2)).expect("read"),
        b"bc"
    );
    run(vfs.touch(&name, 5)).expect("touch");
    assert_eq!(run(vfs.meta(&name)).expect("meta").last_access_ms, 5);
    run(vfs.trim(&name, VfsOffset(1))).expect("trim");
    assert!(matches!(
        run(vfs.read_at(&name, &VfsOffset(0), 2)),
        Err(VfsError::Trimmed)
    ));
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();
    let notifier = vfs.notifier(&name).expect("notifier");
    notifier.register(Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    run(vfs.append(&name, b"d")).expect("append");
    assert!(seen.load(Ordering::SeqCst) >= 1, "the notifier fires");
    run(vfs.close(&name)).expect("close");
    assert!(run(vfs.meta(&name)).expect("meta").closed);
    let listed = run(vfs.list(&stream("coverage"))).expect("list");
    assert!(listed.iter().any(|entry| entry.name == name));
    assert!(!run(vfs.remove(&name)).expect("remove"));
    assert!(matches!(run(vfs.meta(&name)), Err(VfsError::NotFound)));
    // A fork create needs a live source.
    let source = stream("coverage/source");
    assert!(run(vfs.create(&source, meta.clone())).expect("source"));
    run(vfs.append(&source, b"0123456789")).expect("source append");
    let mut child_meta = empty_meta("text/plain");
    child_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    child_meta.len = 4;
    let child = stream("coverage/child");
    assert!(run(vfs.create(&child, child_meta)).expect("fork create"));
    assert_eq!(
        run(vfs.read_at(&child, &VfsOffset(0), 8)).expect("fork read"),
        b"0123"
    );
    assert!(run(vfs.remove(&source)).expect("soft delete"));
    assert!(matches!(run(vfs.meta(&source)), Err(VfsError::Gone)));
    assert_eq!(
        run(vfs.read_at(&child, &VfsOffset(0), 8)).expect("fork read"),
        b"0123"
    );
    assert!(!run(vfs.remove(&child)).expect("hard delete"));
    assert!(matches!(run(vfs.meta(&source)), Err(VfsError::NotFound)));
}
