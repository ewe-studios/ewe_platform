//! BDD scenarios for sliding TTL with an injected clock.

use core::time::Duration;
use foundation_context::Clock;
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
    let stream = DurableStream::with_clock(Arc::new(MemoryVfs::new()), Arc::new(clock.clone()));
    let streams = DurableStreams::new(stream);
    let name = StreamName::parse("ttl/sliding").expect("valid");
    let handle = streams.stream(name);

    smol::block_on(async {
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
    smol::block_on(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read at 5s");
    });

    clock.set(12_000);
    smol::block_on(async {
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read at 12s slides the window");
    });

    clock.set(16_000);
    smol::block_on(async {
        handle.head().await.expect("head at 16s does not expire");
    });

    clock.set(23_000);
    smol::block_on(async {
        let error = handle.head().await.expect_err("ttl expired");
        assert!(matches!(error, StoreError::NotFound));
    });
}
