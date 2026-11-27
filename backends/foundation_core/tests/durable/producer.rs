//! BDD scenarios for producer state and `Stream-Seq`.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendKind, AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, MemoryVfs,
    Producer, ProducerId, ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName, StreamSeq,
};
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
