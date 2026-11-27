//! BDD scenarios for sliding TTL with an injected clock.

use core::time::Duration;
use foundation_context::{Clock, Ctx};
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, Expiry, ForkSpec, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

fn ttl(seconds: u64) -> Expiry {
    Expiry::Ttl(Duration::from_secs(seconds))
}

fn at(millis: i64) -> Expiry {
    Expiry::Absolute(millis)
}

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
    let stream = DurableStream::with_ctx(
        Arc::new(MemoryVfs::new()),
        &Ctx::fixed(Arc::new(clock.clone())),
    );
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

fn ttl_streams(clock: &TestClock) -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::with_ctx(
        Arc::new(MemoryVfs::new()),
        &Ctx::fixed(Arc::new(clock.clone())),
    ))
}

#[test]
fn ttl_slides_on_read_and_write() {
    let clock = TestClock::new();
    let streams = ttl_streams(&clock);
    let name = StreamName::parse("ttl/slide").expect("name");

    foundation_core::valtron::block_on_future_scoped(async {
        let handle = streams.stream(name.clone());
        handle
            .create(CreateSpec {
                expires: Some(ttl(10)),
                ..CreateSpec::default()
            })
            .await
            .expect("create");
        clock.set(9_000);
        handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read slides to 9s");

        clock.set(15_000);
        handle
            .append(AppendRequest::new(Chunk::new(b"x".to_vec())))
            .await
            .expect("append slides to 15s");

        clock.set(24_999);
        handle.head().await.expect("alive at 24.999s");
        clock.set(25_000);
        assert!(matches!(handle.head().await, Err(StoreError::NotFound)));
    });
}

#[test]
fn head_does_not_reset_ttl() {
    let clock = TestClock::new();
    let streams = ttl_streams(&clock);
    let name = StreamName::parse("ttl/head").expect("name");

    foundation_core::valtron::block_on_future_scoped(async {
        let handle = streams.stream(name.clone());
        handle
            .create(CreateSpec {
                expires: Some(ttl(10)),
                ..CreateSpec::default()
            })
            .await
            .expect("create");

        clock.set(9_000);
        handle.head().await.expect("head at 9s");
        clock.set(9_999);
        handle.head().await.expect("head at 9.999s");

        // The window still ends at 10s: HEAD did not slide it.
        clock.set(10_000);
        assert!(matches!(handle.head().await, Err(StoreError::NotFound)));
    });
}

#[test]
fn fork_ttl_follows_table() {
    // Section 2.1 of feature 04: (source, fork request, fork result).
    let rows = [
        (None, None, None),
        (None, Some(ttl(10)), Some(ttl(10))),
        (None, Some(at(100_000)), Some(at(100_000))),
        (Some(ttl(10)), None, Some(ttl(10))),
        (Some(ttl(10)), Some(ttl(20)), Some(ttl(20))),
        (Some(ttl(10)), Some(at(100_000)), Some(at(100_000))),
        (Some(at(100_000)), None, Some(at(100_000))),
        (Some(at(100_000)), Some(ttl(10)), Some(ttl(10))),
        (Some(at(100_000)), Some(at(200_000)), Some(at(200_000))),
    ];

    for (index, (source_expiry, fork_expiry, expected)) in rows.into_iter().enumerate() {
        let clock = TestClock::new();
        let streams = ttl_streams(&clock);
        let source = StreamName::parse(&format!("ttl/table-source-{index}")).expect("name");
        let fork = StreamName::parse(&format!("ttl/table-fork-{index}")).expect("name");

        foundation_core::valtron::block_on_future_scoped(async {
            streams
                .stream(source.clone())
                .create(CreateSpec {
                    expires: source_expiry,
                    initial: Some(Chunk::new(b"data".to_vec())),
                    ..CreateSpec::default()
                })
                .await
                .expect("create source");
            let head = streams
                .stream(fork)
                .create(CreateSpec {
                    expires: fork_expiry,
                    fork: Some(ForkSpec {
                        source,
                        at: None,
                        sub_offset: 0,
                    }),
                    ..CreateSpec::default()
                })
                .await
                .expect("fork")
                .head;
            assert_eq!(head.expires, expected, "row {index}");
        });
    }
}

#[test]
fn expired_source_with_fork_is_soft_deleted() {
    let clock = TestClock::new();
    let streams = ttl_streams(&clock);
    let source = StreamName::parse("ttl/soft-source").expect("name");
    let fork = StreamName::parse("ttl/soft-fork").expect("name");

    foundation_core::valtron::block_on_future_scoped(async {
        streams
            .stream(source.clone())
            .create(CreateSpec {
                expires: Some(ttl(10)),
                initial: Some(Chunk::new(b"data".to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create source");
        // The fork gets its own longer window, so only the source expires.
        streams
            .stream(fork.clone())
            .create(CreateSpec {
                expires: Some(ttl(60)),
                fork: Some(ForkSpec {
                    source: source.clone(),
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork");
    });

    clock.set(10_001);
    foundation_core::valtron::block_on_future_scoped(async {
        // The first access expires the source; a fork holds a reference, so the
        // source is soft-deleted rather than removed.
        assert!(matches!(
            streams.stream(source.clone()).head().await,
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            streams.stream(source.clone()).head().await,
            Err(StoreError::Gone)
        ));

        // The living fork still reads the source bytes.
        let page = streams
            .stream(fork.clone())
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("fork keeps reading");
        let bytes: Vec<u8> = page
            .chunks
            .into_iter()
            .flat_map(|chunk| chunk.as_bytes().to_vec())
            .collect();
        assert_eq!(bytes, b"data");

        // Deleting the last fork cascades cleanup to the soft-deleted source.
        streams
            .stream(fork.clone())
            .delete()
            .await
            .expect("delete fork");
        assert!(matches!(
            streams.stream(source).head().await,
            Err(StoreError::NotFound)
        ));
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Access {
    Ok,
    NotFound,
}

fn access<T>(result: Result<T, StoreError>) -> Access {
    match result {
        Ok(_) => Access::Ok,
        Err(StoreError::NotFound) => Access::NotFound,
        Err(other) => panic!("unexpected result: {other:?}"),
    }
}

/// The small time-to-live model: a read or write slides the window, `HEAD` does
/// not, and the first access after the deadline removes the stream.
struct TtlModel {
    now: i64,
    last_access: i64,
    removed: bool,
    written: usize,
}

impl TtlModel {
    const WINDOW_MS: i64 = 10_000;

    fn new() -> Self {
        Self {
            now: 0,
            last_access: 0,
            removed: false,
            written: 0,
        }
    }

    fn step(&mut self, delta_ms: i64, action: u8) -> Access {
        self.now += delta_ms;
        if self.removed {
            return Access::NotFound;
        }
        if self.now >= self.last_access + Self::WINDOW_MS {
            self.removed = true;
            return Access::NotFound;
        }
        if action != 2 {
            // A read or write slides the window.
            self.last_access = self.now;
        }
        if action == 1 {
            self.written += 1;
        }
        Access::Ok
    }
}

proptest! {
    #[test]
    fn ttl_model_matches(ops in prop::collection::vec((0_i64..3_000, 0_u8..3), 1..12)) {
        let clock = TestClock::new();
        let streams = ttl_streams(&clock);
        let name = StreamName::parse("ttl/model").expect("name");

        let (actual, expected, model_written) = foundation_core::valtron::block_on_future_scoped(async {
            streams
                .stream(name.clone())
                .create(CreateSpec {
                    expires: Some(ttl(10)),
                    ..CreateSpec::default()
                })
                .await
                .expect("create");
            let handle = streams.stream(name.clone());

            let mut model = TtlModel::new();
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            for (delta_ms, action) in ops {
                let want = model.step(delta_ms, action);
                clock.set(model.now);
                let got = match action {
                    0 => access(
                        handle
                            .read(ReadRequest {
                                from: ReadFrom::Start,
                                limit: ReadLimit::default(),
                            })
                            .await,
                    ),
                    1 => access(
                        handle
                            .append(AppendRequest::new(Chunk::new(b"x".to_vec())))
                            .await,
                    ),
                    _ => access(handle.head().await),
                };
                actual.push(got);
                expected.push(want);
            }
            (actual, expected, model.written)
        });

        prop_assert_eq!(actual, expected);
        if model_written > 0 {
            // The model equals the stored bytes whenever the stream still exists.
            let bytes = foundation_core::valtron::block_on_future_scoped(async {
                match streams
                    .stream(name)
                    .read(ReadRequest {
                        from: ReadFrom::Start,
                        limit: ReadLimit::default(),
                    })
                    .await
                {
                    Ok(page) => Some(
                        page.chunks
                            .into_iter()
                            .flat_map(|chunk| chunk.as_bytes().to_vec())
                            .collect::<Vec<u8>>(),
                    ),
                    Err(StoreError::NotFound) => None,
                    Err(other) => panic!("unexpected read: {other:?}"),
                }
            });
            if let Some(bytes) = bytes {
                prop_assert_eq!(bytes.len(), model_written);
            }
        }
    }
}
