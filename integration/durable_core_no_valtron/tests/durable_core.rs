//! The durable protocol core conforms without the valtron task engine.
//!
//! This crate depends on `foundation_core` with `default-features = false` and the `std` and
//! `alloc` features only. It never enables `valtron`, `test-clock`, `multi` or `vfs-testkit`.
//! The successful build therefore proves that `foundation_core::durable` has no valtron type,
//! trait or executor on the protocol path.
//!
//! The tests drive the protocol with the native async API and `futures-lite`. They use no HTTP
//! adapter, no socket and no valtron runtime. The suite covers the rules of spec 71 section 2:
//! create, append, read pages, close, fork, producer, `Stream-Seq`, time to live (TTL) and
//! soft-delete.

use core::time::Duration;
use foundation_context::{Clock, Ctx, Instant, SystemTime};
use foundation_core::durable::{
    AppendKind, AppendRequest, Chunk, CloseRequest, CreateSpec, DurableLimits, DurableService,
    DurableStream, DurableStreams, Expiry, ForkSpec, MemoryVfs, Producer, ProducerId, ReadFrom,
    ReadLimit, ReadPage, ReadRequest, StoreError, StreamName, StreamOffset, StreamSeq,
};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

fn service(limits: DurableLimits) -> DurableService<DurableStream> {
    DurableService::with_limits(
        DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()),
        limits,
    )
}

fn name(raw: &str) -> StreamName {
    StreamName::parse(raw).expect("valid stream name")
}

fn offset(value: u64) -> StreamOffset {
    StreamOffset::from_bytes(value.to_be_bytes())
}

fn producer(id: &str, epoch: u64, seq: u64) -> Producer {
    Producer {
        id: ProducerId::parse(id).expect("valid producer identifier"),
        epoch,
        seq,
    }
}

fn append_request(
    data: &[u8],
    stream_seq: Option<&str>,
    producer: Option<Producer>,
) -> AppendRequest {
    AppendRequest {
        chunk: Chunk::new(data.to_vec()),
        content_type: None,
        stream_seq: stream_seq.map(|seq| StreamSeq::from_bytes(seq.as_bytes().to_vec())),
        producer,
        close: false,
    }
}

fn page_bytes(page: &ReadPage) -> Vec<u8> {
    page.chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect()
}

async fn read_all(streams: &DurableStreams<DurableStream>, stream: &StreamName) -> Vec<u8> {
    let page = streams
        .stream(stream.clone())
        .read(ReadRequest {
            from: ReadFrom::Start,
            limit: ReadLimit::default(),
        })
        .await
        .expect("read");
    page_bytes(&page)
}

async fn create(streams: &DurableStreams<DurableStream>, stream: &StreamName) {
    streams
        .stream(stream.clone())
        .create(CreateSpec::default())
        .await
        .expect("create");
}

async fn append(streams: &DurableStreams<DurableStream>, stream: &StreamName, data: &[u8]) {
    streams
        .stream(stream.clone())
        .append(AppendRequest::new(Chunk::new(data.to_vec())))
        .await
        .expect("append");
}

#[test]
fn given_an_empty_driver_when_create_runs_twice_then_the_second_call_is_idempotent() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/create");
        let handle = streams.stream(stream.clone());

        let first = handle
            .create(CreateSpec::default())
            .await
            .expect("first create");
        assert!(first.created, "the first create makes the stream");

        let second = handle
            .create(CreateSpec::default())
            .await
            .expect("second create");
        assert!(!second.created, "the second create is idempotent");
        assert_eq!(second.head.tail, offset(0));
    });
}

#[test]
fn given_a_stream_when_append_and_read_run_then_the_bytes_are_exact() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/bytes");
        create(&streams, &stream).await;
        append(&streams, &stream, b"hello ").await;
        append(&streams, &stream, b"without valtron").await;

        let page = streams
            .stream(stream)
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(page_bytes(&page), b"hello without valtron");
        assert!(page.cursor.up_to_date, "the page reaches the stream tail");
        assert!(!page.cursor.closed, "the stream is open");
    });
}

#[test]
fn given_a_page_limit_when_read_runs_then_no_page_exceeds_it() {
    futures_lite::future::block_on(async {
        let service = service(DurableLimits::default().with_page_bytes(4));
        let stream = name("conformance/bounded");
        service
            .create(stream.clone(), CreateSpec::default())
            .await
            .expect("create");
        service
            .append(
                &stream,
                AppendRequest::new(Chunk::new(b"0123456789".to_vec())),
            )
            .await
            .expect("append");

        let mut from = ReadFrom::Start;
        let mut total = Vec::new();
        let mut pages = 0_usize;
        loop {
            let page = service
                .read(
                    &stream,
                    ReadRequest {
                        from,
                        limit: ReadLimit::default(),
                    },
                )
                .await
                .expect("read");
            pages += 1;
            for chunk in &page.chunks {
                assert!(
                    chunk.as_bytes().len() <= 4,
                    "a page stays inside the byte limit"
                );
            }
            total.extend(page_bytes(&page));
            let up_to_date = page.cursor.up_to_date;
            let next = page.cursor.next;
            if up_to_date {
                break;
            }
            from = ReadFrom::Offset(next);
        }

        assert_eq!(total, b"0123456789");
        assert!(pages >= 3, "the limit forces several pages, got {pages}");
    });
}

#[test]
fn given_a_closed_stream_when_append_runs_then_it_is_refused_and_the_data_stays_readable() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/closed");
        create(&streams, &stream).await;
        append(&streams, &stream, b"abc").await;
        streams
            .stream(stream.clone())
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let error = streams
            .stream(stream.clone())
            .append(AppendRequest::new(Chunk::new(b"late".to_vec())))
            .await
            .expect_err("a closed stream refuses an append");
        match error {
            StoreError::Closed { tail } => assert_eq!(tail, offset(3)),
            other => panic!("unexpected refusal: {other:?}"),
        }

        let page = streams
            .stream(stream)
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");
        assert_eq!(page_bytes(&page), b"abc");
        assert!(page.cursor.up_to_date);
        assert!(page.cursor.closed, "the page reaches the closed tail");
    });
}

#[test]
fn given_a_source_when_a_fork_reads_then_it_spans_the_prefix_and_diverges() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let source = name("conformance/fork-source");
        let target = name("conformance/fork-target");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        let fork = streams
            .stream(target.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");
        assert!(fork.created);

        assert_eq!(read_all(&streams, &target).await, b"ab");
        append(&streams, &target, b"X").await;
        assert_eq!(read_all(&streams, &target).await, b"abX");

        // The fork owns only its tail. A later source append stays out of it.
        append(&streams, &source, b"d").await;
        assert_eq!(read_all(&streams, &target).await, b"abX");
        assert_eq!(read_all(&streams, &source).await, b"abcd");
    });
}

#[test]
fn given_a_producer_when_it_retries_a_sequence_then_the_data_is_not_duplicated() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/producer-retry");
        create(&streams, &stream).await;
        let handle = streams.stream(stream.clone());

        let first = handle
            .append(append_request(b"a", None, Some(producer("p", 0, 0))))
            .await
            .expect("first append");
        assert_eq!(first.kind, AppendKind::Created);

        let retry = handle
            .append(append_request(b"b", None, Some(producer("p", 0, 0))))
            .await
            .expect("retry");
        assert_eq!(retry.kind, AppendKind::Duplicate);
        assert_eq!(retry.producer_seq, Some(0));

        assert_eq!(read_all(&streams, &stream).await, b"a");
    });
}

#[test]
fn given_a_producer_when_an_old_epoch_writes_then_it_is_fenced() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/producer-fenced");
        create(&streams, &stream).await;
        let handle = streams.stream(stream);

        handle
            .append(append_request(b"a", None, Some(producer("p", 0, 0))))
            .await
            .expect("epoch 0");
        handle
            .append(append_request(b"b", None, Some(producer("p", 1, 0))))
            .await
            .expect("epoch 1");

        let error = handle
            .append(append_request(b"c", None, Some(producer("p", 0, 1))))
            .await
            .expect_err("a stale epoch is fenced");

        assert!(matches!(error, StoreError::Fenced { current_epoch: 1 }));
    });
}

#[test]
fn given_a_producer_when_a_sequence_gap_appears_then_it_is_a_typed_error() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/producer-gap");
        create(&streams, &stream).await;

        let error = streams
            .stream(stream)
            .append(append_request(b"a", None, Some(producer("p", 0, 1))))
            .await
            .expect_err("a sequence gap is refused");

        assert!(matches!(
            error,
            StoreError::SequenceGap {
                expected: 0,
                received: 1
            }
        ));
    });
}

#[test]
fn given_stream_seq_when_it_regresses_then_it_is_a_conflict_and_a_higher_value_is_accepted() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let stream = name("conformance/stream-seq");
        create(&streams, &stream).await;
        let handle = streams.stream(stream.clone());

        handle
            .append(append_request(b"a", Some("2"), None))
            .await
            .expect("the first value is accepted");

        // Byte order puts "10" before "2", so "10" regresses.
        let lower = handle
            .append(append_request(b"b", Some("10"), None))
            .await
            .expect_err("a lower value conflicts");
        assert!(matches!(lower, StoreError::StreamSeqConflict { .. }));

        let equal = handle
            .append(append_request(b"c", Some("2"), None))
            .await
            .expect_err("an equal value conflicts");
        assert!(matches!(equal, StoreError::StreamSeqConflict { .. }));

        handle
            .append(append_request(b"d", Some("3"), None))
            .await
            .expect("a higher value is accepted");

        assert_eq!(read_all(&streams, &stream).await, b"ad");
    });
}

/// A hand-written clock for the TTL scenario.
///
/// The wall clock is the test-controlled millisecond value. The monotonic clock stays fixed,
/// because the TTL rule reads wall time only. This type has no valtron dependency.
#[derive(Clone)]
struct TestClock {
    origin: Instant,
    now_ms: Arc<AtomicI64>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            now_ms: Arc::new(AtomicI64::new(0)),
        }
    }

    fn set(&self, millis: i64) {
        self.now_ms.store(millis, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.origin
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wall_now(&self) -> SystemTime {
        let millis = self.now_ms.load(Ordering::SeqCst);
        let forward = Duration::from_millis(millis.unsigned_abs());
        if millis >= 0 {
            SystemTime::UNIX_EPOCH + forward
        } else {
            SystemTime::UNIX_EPOCH - forward
        }
    }
}

fn streams_at(clock: &TestClock) -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(
        Arc::new(MemoryVfs::new()),
        &Ctx::fixed(Arc::new(clock.clone())),
    ))
}

#[test]
fn given_a_ttl_stream_when_time_passes_then_reads_slide_the_window_and_it_expires() {
    let clock = TestClock::new();
    let streams = streams_at(&clock);
    let stream = name("conformance/ttl");
    let handle = streams.stream(stream);

    futures_lite::future::block_on(async {
        handle
            .create(CreateSpec {
                expires: Some(Expiry::Ttl(Duration::from_secs(10))),
                ..CreateSpec::default()
            })
            .await
            .expect("create");
        handle
            .append(AppendRequest::new(Chunk::new(b"data".to_vec())))
            .await
            .expect("append");
    });

    clock.set(5_000);
    futures_lite::future::block_on(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("a read at 5 s is inside the window");
    });

    // The read at 5 s moves the deadline to 15 s. A read at 12 s moves it to 22 s.
    clock.set(12_000);
    futures_lite::future::block_on(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("a read at 12 s slides the window");
    });

    clock.set(16_000);
    futures_lite::future::block_on(async {
        handle
            .head()
            .await
            .expect("head at 16 s is inside the window");
    });

    clock.set(23_000);
    futures_lite::future::block_on(async {
        assert!(matches!(handle.head().await, Err(StoreError::NotFound)));
    });
}

#[test]
fn given_a_source_with_a_fork_when_the_source_is_deleted_then_the_fork_reads_and_cascade_cleans_up()
{
    futures_lite::future::block_on(async {
        let streams = streams();
        let source = name("conformance/soft-source");
        let target = name("conformance/soft-target");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;
        streams
            .stream(target.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");

        let deleted = streams
            .stream(source.clone())
            .delete()
            .await
            .expect("delete the source");
        assert!(deleted.soft_deleted, "a source with a fork is soft-deleted");
        assert!(matches!(
            streams.stream(source.clone()).head().await,
            Err(StoreError::Gone)
        ));
        assert!(matches!(
            streams
                .stream(source.clone())
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await,
            Err(StoreError::Gone)
        ));

        // The fork still reads the inherited prefix of the soft-deleted source.
        assert_eq!(read_all(&streams, &target).await, b"ab");

        // Deleting the last fork cascades to the soft-deleted source.
        streams
            .stream(target)
            .delete()
            .await
            .expect("delete the fork");
        assert!(matches!(
            streams.stream(source).head().await,
            Err(StoreError::NotFound)
        ));
    });
}
