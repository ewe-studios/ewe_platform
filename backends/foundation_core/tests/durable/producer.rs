//! BDD scenarios for producer state and `Stream-Seq`.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendKind, AppendOutcome, AppendRequest, Chunk, CreateSpec, DurableDriver, DurableStream,
    DurableStreamStore, DurableStreams, MemoryVfs, Producer, ProducerId, ReadFrom, ReadLimit,
    ReadRequest, StoreError, StreamName, StreamSeq,
};
use proptest::prelude::*;
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

async fn create(streams: &DurableStreams<DurableStream>, name: &StreamName) {
    streams
        .stream(name.clone())
        .create(CreateSpec::default())
        .await
        .expect("create");
}

fn producer(id: &str, epoch: u64, seq: u64) -> Producer {
    Producer {
        id: ProducerId::parse(id).expect("producer id"),
        epoch,
        seq,
    }
}

fn append(
    data: &'static [u8],
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

async fn read_all(streams: &DurableStreams<DurableStream>, name: &StreamName) -> Vec<u8> {
    let page = streams
        .stream(name.clone())
        .read(ReadRequest {
            from: ReadFrom::Start,
            limit: ReadLimit::default(),
        })
        .await
        .expect("read");
    page.chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect()
}

#[test]
fn given_a_producer_when_it_retries_a_sequence_then_the_data_is_not_duplicated() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/retry").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name.clone());

        let first = handle
            .append(append(b"a", None, Some(producer("p", 0, 0))))
            .await
            .expect("first append");
        assert_eq!(first.kind, AppendKind::Created);

        let retry = handle
            .append(append(b"b", None, Some(producer("p", 0, 0))))
            .await
            .expect("retry");
        assert_eq!(retry.kind, AppendKind::Duplicate);
        assert_eq!(retry.producer_seq, Some(0));

        assert_eq!(read_all(&streams, &name).await, b"a");
    });
}

#[test]
fn given_a_producer_when_a_sequence_gap_appears_then_it_is_a_typed_error() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/gap").expect("valid");
        create(&streams, &name).await;

        let error = streams
            .stream(name)
            .append(append(b"a", None, Some(producer("p", 0, 1))))
            .await
            .expect_err("gap");

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
fn given_a_producer_when_an_old_epoch_writes_then_it_is_fenced() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/fenced").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", None, Some(producer("p", 0, 0))))
            .await
            .expect("epoch 0");
        handle
            .append(append(b"b", None, Some(producer("p", 1, 0))))
            .await
            .expect("epoch 1");

        let error = handle
            .append(append(b"c", None, Some(producer("p", 0, 1))))
            .await
            .expect_err("old epoch");

        assert!(matches!(error, StoreError::Fenced { current_epoch: 1 }));
    });
}

#[test]
fn given_a_new_epoch_when_seq_is_not_zero_then_it_is_invalid() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/epoch-seq").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", None, Some(producer("p", 0, 0))))
            .await
            .expect("epoch 0");

        let error = handle
            .append(append(b"b", None, Some(producer("p", 1, 1))))
            .await
            .expect_err("bad epoch start");

        assert!(matches!(error, StoreError::InvalidEpochSeq { seq: 1 }));
    });
}

#[test]
fn given_stream_seq_when_it_regresses_then_it_is_a_conflict() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/stream-seq").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", Some("2"), None))
            .await
            .expect("first");

        let error = handle
            .append(append(b"b", Some("10"), None))
            .await
            .expect_err("lexicographic regression");

        assert!(matches!(error, StoreError::StreamSeqConflict { .. }));
    });
}

#[test]
fn given_stream_seq_when_it_skips_forward_then_it_is_accepted() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/stream-seq-skip").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", Some("10"), None))
            .await
            .expect("first");
        handle
            .append(append(b"b", Some("2"), None))
            .await
            .expect("second");
    });
}

#[test]
fn given_a_producer_retry_when_stream_seq_is_older_then_it_is_deduplicated() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/retry-seq").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", Some("2"), Some(producer("p", 0, 0))))
            .await
            .expect("first");

        let retry = handle
            .append(append(b"b", Some("1"), Some(producer("p", 0, 0))))
            .await
            .expect("producer first");

        assert_eq!(retry.kind, AppendKind::Duplicate);
    });
}

#[test]
fn given_a_producer_when_the_stream_restarts_then_a_retry_is_still_deduplicated() {
    foundation_core::valtron::block_on_future_scoped(async {
        let vfs = Arc::new(MemoryVfs::new());
        let name = StreamName::parse("producer/restart").expect("valid");

        {
            let streams = DurableStreams::new(DurableStream::new(vfs.clone(), &Ctx::host()));
            create(&streams, &name).await;
            streams
                .stream(name.clone())
                .append(append(b"a", None, Some(producer("p", 0, 0))))
                .await
                .expect("first append");
        }

        let streams = DurableStreams::new(DurableStream::new(vfs, &Ctx::host()));
        let retry = streams
            .stream(name.clone())
            .append(append(b"b", None, Some(producer("p", 0, 0))))
            .await
            .expect("retry after restart");

        assert_eq!(retry.kind, AppendKind::Duplicate);
        assert_eq!(read_all(&streams, &name).await, b"a");
    });
}

#[test]
fn given_two_concurrent_producers_when_they_append_then_both_are_accepted() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/concurrent").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name.clone());

        let first = handle.append(append(b"a", None, Some(producer("a", 0, 0))));
        let second = handle.append(append(b"b", None, Some(producer("b", 0, 0))));
        let (first, second) = smol::future::zip(first, second).await;

        assert!(first.is_ok(), "first append: {first:?}");
        assert!(second.is_ok(), "second append: {second:?}");

        let bytes = read_all(&streams, &name).await;
        assert!(
            bytes == b"ab" || bytes == b"ba",
            "both bytes present: {bytes:?}"
        );
    });
}

#[test]
fn given_two_producers_when_they_are_independent_then_each_keeps_its_state() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("producer/multi").expect("valid");
        create(&streams, &name).await;
        let handle = streams.stream(name);

        handle
            .append(append(b"a", None, Some(producer("a", 0, 0))))
            .await
            .expect("a0");
        handle
            .append(append(b"b", None, Some(producer("b", 0, 0))))
            .await
            .expect("b0");
        handle
            .append(append(b"c", None, Some(producer("a", 0, 1))))
            .await
            .expect("a1");
    });
}

/// The decision a producer ladder step makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Created,
    Duplicate(u64),
    Fenced(u64),
    InvalidEpochSeq(u64),
    Gap(u64, u64),
}

/// Reduce a producer append result to its decision.
fn decision(result: Result<AppendOutcome, StoreError>) -> Decision {
    match result {
        Ok(outcome) => match outcome.kind {
            AppendKind::Created => Decision::Created,
            AppendKind::Duplicate => {
                Decision::Duplicate(outcome.producer_seq.expect("a duplicate has a sequence"))
            }
        },
        Err(StoreError::Fenced { current_epoch }) => Decision::Fenced(current_epoch),
        Err(StoreError::InvalidEpochSeq { seq }) => Decision::InvalidEpochSeq(seq),
        Err(StoreError::SequenceGap { expected, received }) => Decision::Gap(expected, received),
        Err(other) => panic!("unexpected producer error: {other:?}"),
    }
}

/// The small state model for one producer identifier.
#[derive(Default)]
struct ProducerModel {
    state: Option<(u64, u64)>,
}

impl ProducerModel {
    fn step(&mut self, epoch: u64, seq: u64) -> Decision {
        match self.state {
            None => {
                if seq != 0 {
                    return Decision::Gap(0, seq);
                }
                self.state = Some((epoch, 0));
                Decision::Created
            }
            Some((current_epoch, last_seq)) => {
                if epoch < current_epoch {
                    Decision::Fenced(current_epoch)
                } else if epoch > current_epoch {
                    if seq != 0 {
                        return Decision::InvalidEpochSeq(seq);
                    }
                    self.state = Some((epoch, 0));
                    Decision::Created
                } else if seq <= last_seq {
                    Decision::Duplicate(last_seq)
                } else if seq == last_seq + 1 {
                    self.state = Some((epoch, seq));
                    Decision::Created
                } else {
                    Decision::Gap(last_seq + 1, seq)
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn producer_model_matches(ops in prop::collection::vec((0_u64..3, 0_u64..4), 1..16)) {
        let (actual, expected, created, read) = foundation_core::valtron::block_on_future_scoped(async {
            let streams = streams();
            let name = StreamName::parse("producer/model").expect("valid");
            create(&streams, &name).await;
            let handle = streams.stream(name.clone());

            let mut model = ProducerModel::default();
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            let mut created = 0_usize;
            for (epoch, seq) in ops {
                let result = handle
                    .append(append(b"x", None, Some(producer("p", epoch, seq))))
                    .await;
                let decided = decision(result);
                if decided == Decision::Created {
                    created += 1;
                }
                actual.push(decided);
                expected.push(model.step(epoch, seq));
            }
            let read = read_all(&streams, &name).await.len();
            (actual, expected, created, read)
        });

        prop_assert_eq!(actual, expected);
        prop_assert_eq!(read, created);
    }
}

proptest! {
    #[test]
    fn stream_seq_is_monotonic(values in prop::collection::vec(1_u64..1_000, 1..10)) {
        foundation_core::valtron::block_on_future_scoped(async {
            let streams = streams();
            let name = StreamName::parse("seq/monotonic").expect("valid");
            create(&streams, &name).await;
            let handle = streams.stream(name);

            // `StreamSeq` orders by bytes, so pad each value to one width.
            let mut ordered: Vec<String> =
                values.iter().map(|value| format!("{value:04}")).collect();
            ordered.sort_unstable();
            ordered.dedup();

            let mut last_accepted: Option<u64> = None;
            for value in &ordered {
                handle
                    .append(append(b"x", Some(value.as_str()), None))
                    .await
                    .expect("an increasing value is accepted");
                let parsed = value.parse::<u64>().expect("decimal");
                assert!(
                    last_accepted.is_none_or(|last| parsed > last),
                    "the accepted value never decreases"
                );
                last_accepted = Some(parsed);

                let error = handle
                    .append(append(b"x", Some(value.as_str()), None))
                    .await
                    .expect_err("a repeated value is a conflict");
                assert!(matches!(error, StoreError::StreamSeqConflict { .. }));
            }
        });
    }
}

#[test]
fn drivers_agree() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let direct = StreamName::parse("producer/direct").expect("valid");
        let business = StreamName::parse("producer/business").expect("valid");
        create(&streams, &direct).await;
        create(&streams, &business).await;

        // The store-trait path and the business-API path must give one result.
        for seq in 0..4 {
            let request = append(b"x", None, Some(producer("p", 0, seq)));
            let by_store = streams
                .driver()
                .store()
                .append(&direct, request.clone())
                .await
                .expect("store append");
            let by_business = streams
                .stream(business.clone())
                .append(request)
                .await
                .expect("business append");

            assert_eq!(by_store.kind, by_business.kind);
            assert_eq!(by_store.producer_seq, by_business.producer_seq);
            assert_eq!(by_store.cursor, by_business.cursor);
        }

        assert_eq!(
            read_all(&streams, &direct).await,
            read_all(&streams, &business).await
        );
    });
}
