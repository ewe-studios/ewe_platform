//! BDD scenarios for sliding TTL with an injected clock.

use core::time::Duration;
use foundation_context::{Clock, Ctx};
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, Expiry, MemoryVfs, ReadFrom,
    ReadLimit, ReadRequest, StoreError, StreamName,
};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct TestClock {
    now_ms: Arc<AtomicI64>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            now_ms: Arc::new(AtomicI64::new(0)),
        }
    }

    fn set(&self, millis: i64) {
        self.now_ms.store(millis, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    #[expect(
        clippy::disallowed_methods,
        reason = "The test drives wall time only; it never reads the monotonic clock."
    )]
    fn now(&self) -> foundation_compact::Instant {
        foundation_compact::Instant::now()
    }

    fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    fn wall_now(&self) -> foundation_context::SystemTime {
        foundation_compact::time::from_unix_millis(self.now_ms.load(Ordering::SeqCst))
    }
}

#[test]
fn given_a_ttl_stream_when_reads_and_heads_happen_then_only_reads_slide_the_window() {
    let clock = TestClock::new();
    let stream = DurableStream::with_ctx(Arc::new(MemoryVfs::new()), &Ctx::fixed(Arc::new(clock.clone())));
    let streams = DurableStreams::new(stream);
    let name = StreamName::parse("ttl/sliding").expect("valid");
    let handle = streams.stream(name);

    foundation_core::valtron::block_on_future_scoped(async {
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
    foundation_core::valtron::block_on_future_scoped(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read at 5s");
    });

    clock.set(12_000);
    foundation_core::valtron::block_on_future_scoped(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read at 12s slides the window");
    });

    clock.set(16_000);
    foundation_core::valtron::block_on_future_scoped(async {
        handle.head().await.expect("head at 16s does not expire");
    });

    clock.set(23_000);
    foundation_core::valtron::block_on_future_scoped(async {
        let error = handle.head().await.expect_err("ttl expired");
        assert!(matches!(error, StoreError::NotFound));
    });
}

#[test]
fn given_an_expired_ttl_stream_when_it_is_used_then_it_reports_not_found() {
    let clock = TestClock::new();
    let streams = DurableStreams::new(DurableStream::with_ctx(
        Arc::new(MemoryVfs::new()),
        &Ctx::fixed(Arc::new(clock.clone())),
    ));
    let name = StreamName::parse("ttl/expired").expect("name");

    foundation_core::valtron::block_on_future_scoped(async {
        streams
            .stream(name.clone())
            .create(CreateSpec {
                expires: Some(Expiry::Ttl(Duration::from_secs(10))),
                initial: Some(Chunk::new(b"data".to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create");
    });

    clock.set(10_001);

    foundation_core::valtron::block_on_future_scoped(async {
        assert!(matches!(
            streams.stream(name.clone()).head().await,
            Err(StoreError::NotFound)
        ));
        // The entry is gone, so a later access stays not found.
        assert!(matches!(
            streams.stream(name.clone()).head().await,
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            streams
                .stream(name.clone())
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await,
            Err(StoreError::NotFound)
        ));
    });
}

#[test]
fn given_a_ttl_source_when_it_expires_then_a_living_fork_keeps_its_own_window() {
    let clock = TestClock::new();
    let streams = DurableStreams::new(DurableStream::with_ctx(
        Arc::new(MemoryVfs::new()),
        &Ctx::fixed(Arc::new(clock.clone())),
    ));
    let source = StreamName::parse("ttl/fork-src").expect("name");
    let fork = StreamName::parse("ttl/fork-child").expect("name");

    foundation_core::valtron::block_on_future_scoped(async {
        streams
            .stream(source.clone())
            .create(CreateSpec {
                expires: Some(Expiry::Ttl(Duration::from_secs(10))),
                initial: Some(Chunk::new(b"data".to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create source");
        streams
            .stream(fork.clone())
            .create(CreateSpec {
                fork: Some(foundation_core::durable::ForkSpec {
                    source: source.clone(),
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork");
    });

    clock.set(9_000);
    foundation_core::valtron::block_on_future_scoped(async {
        // A fork read slides the fork window only.
        streams
            .stream(fork.clone())
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("fork read");
    });

    clock.set(11_000);
    foundation_core::valtron::block_on_future_scoped(async {
        assert!(matches!(
            streams.stream(source.clone()).head().await,
            Err(StoreError::NotFound)
        ));
        let page = streams
            .stream(fork.clone())
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("fork survives");
        let bytes: Vec<u8> = page
            .chunks
            .into_iter()
            .flat_map(|chunk| chunk.as_bytes().to_vec())
            .collect();
        assert_eq!(bytes, b"data");
    });
}
