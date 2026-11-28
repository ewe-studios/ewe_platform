//! Deterministic simulation testing (DST) for the durable core.
//!
//! WHY: one seed must replay every observation. A failure is then repeatable.
//! WHAT: a small random scenario creates a source and a fork, then appends,
//! retries a producer, reads, moves a time-to-live (TTL) clock and expires the
//! source while the fork still reads it.
//! HOW: the seed builds a `Ctx` with a seeded RNG. A `SimulatedClock` moves only
//! when the scenario moves it. The memory VFS is deterministic.

use core::time::Duration;
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, Expiry, ForkSpec, MemoryVfs,
    Producer, ProducerId, ReadFrom, ReadLimit, ReadRequest, StreamName,
};
use foundation_core::valtron::{block_on_future_scoped, SimulatedClock};
use std::sync::Arc;

/// The number of random steps in one run.
const STEPS: u32 = 24;

/// Draw one value below `modulo` from the seeded context RNG.
fn draw(ctx: &Ctx, modulo: u32) -> u32 {
    let rng = ctx.rng();
    let mut rng = rng.lock().expect("the seeded RNG");
    rng.next_u32() % modulo
}

/// Run one scenario and return its observation log.
#[expect(
    clippy::too_many_lines,
    reason = "One deterministic scenario reads best in one place."
)]
fn run(seed: u64) -> Vec<String> {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(clock.clone(), seed);
    let streams = DurableStreams::new(DurableStream::with_ctx(Arc::new(MemoryVfs::new()), &ctx));
    let source = StreamName::parse("dst/source").expect("valid source");
    let fork = StreamName::parse("dst/fork").expect("valid fork");
    let producer = ProducerId::parse("dst-producer").expect("valid producer");
    let mut log = Vec::new();

    block_on_future_scoped(async {
        let source_handle = streams.stream(source.clone());
        source_handle
            .create(CreateSpec {
                expires: Some(Expiry::Ttl(Duration::from_secs(300))),
                initial: Some(Chunk::new(b"seed".to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create source");
        log.push("create source".to_string());

        let fork_handle = streams.stream(fork.clone());
        fork_handle
            .create(CreateSpec {
                expires: Some(Expiry::Ttl(Duration::from_secs(3_600))),
                fork: Some(ForkSpec {
                    source: source.clone(),
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("create fork");
        log.push("create fork".to_string());

        for step in 0..STEPS {
            match draw(&ctx, 6) {
                0 => {
                    let byte = u8::try_from(draw(&ctx, 0x80)).expect("below 128");
                    let result = source_handle
                        .append(AppendRequest::new(Chunk::new(vec![byte])))
                        .await;
                    log.push(format!("append source {step} {byte:02x} -> {result:?}"));
                }
                1 => {
                    let seq = u64::from(draw(&ctx, 4));
                    let result = source_handle
                        .append(AppendRequest {
                            chunk: Chunk::new(vec![b'p']),
                            content_type: None,
                            stream_seq: None,
                            producer: Some(Producer {
                                id: producer.clone(),
                                epoch: 0,
                                seq,
                            }),
                            close: false,
                        })
                        .await;
                    log.push(format!("append producer {step} seq {seq} -> {result:?}"));
                }
                2 => {
                    let result = source_handle
                        .read(ReadRequest {
                            from: ReadFrom::Start,
                            limit: ReadLimit::default(),
                        })
                        .await;
                    log.push(format!("read source {step} -> {result:?}"));
                }
                3 => {
                    let byte = u8::try_from(draw(&ctx, 0x80)).expect("below 128");
                    let result = fork_handle
                        .append(AppendRequest::new(Chunk::new(vec![byte])))
                        .await;
                    log.push(format!("append fork {step} {byte:02x} -> {result:?}"));
                }
                4 => {
                    let seconds = u64::from(draw(&ctx, 5)) + 1;
                    clock.advance_wall(Duration::from_secs(seconds));
                    log.push(format!("advance {step} {seconds}s"));
                }
                _ => {
                    let result = fork_handle.head().await;
                    log.push(format!("head fork {step} -> {result:?}"));
                }
            }
        }

        let final_source = source_handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await;
        log.push(format!("final source -> {final_source:?}"));
        let final_fork = fork_handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await;
        log.push(format!("final fork -> {final_fork:?}"));

        // Expiry with a living fork soft-deletes the source. The first access
        // reports `NotFound`; the second reports `Gone`; the fork still reads.
        clock.advance_wall(Duration::from_secs(3_601));
        let first = source_handle.head().await;
        log.push(format!("expired source -> {first:?}"));
        let second = source_handle.head().await;
        log.push(format!("soft-deleted source -> {second:?}"));
        let after = fork_handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await;
        log.push(format!("fork after expiry -> {after:?}"));
    });

    log
}

#[test]
fn seeded_reader_replay_is_byte_exact() {
    let first = run(7);
    let replay = run(7);

    assert_eq!(first, replay, "one seed must replay the run exactly");
    assert!(
        first.len() > usize::try_from(STEPS).expect("steps fit"),
        "the scenario records every step"
    );
    assert_ne!(run(8), first, "a different seed gives a different run");
}
