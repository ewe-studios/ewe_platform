//! Tests for the valtron producer task.

use foundation_context::Ctx;
use foundation_core::durable::{
    Chunk, CreateSpec, DurableStream, DurableStreams, MemoryVfs, ProducerId, ReadFrom, ReadLimit,
    ReadRequest, StreamName,
};
use foundation_core::durable::valtron::{producer_task, DurableStreamWriter};
use foundation_core::valtron::{valtron_test, TaskStatus};
use std::sync::Arc;

#[valtron_test(timeout = 5000)]
fn given_a_producer_task_when_it_runs_then_it_appends_in_order() {
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host());
    let name = StreamName::parse("producer/task").expect("valid name");

    foundation_core::valtron::block_on_future_scoped(async {
        DurableStreams::new(stream.clone())
            .stream(name.clone())
            .create(CreateSpec::default())
            .await
            .expect("create");
    });

    let writer = DurableStreamWriter::new(
        stream.clone(),
        name.clone(),
        ProducerId::parse("task-producer").expect("id"),
        0,
    );
    let task = producer_task(
        writer,
        vec![Chunk::new(b"a".to_vec()), Chunk::new(b"b".to_vec())],
    );

    let mut outcomes = Vec::new();
    for status in task {
        if let TaskStatus::Ready(result) = status {
            outcomes.push(result);
        }
    }

    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(Result::is_ok));

    let bytes = foundation_core::valtron::block_on_future_scoped(async {
        let page = DurableStreams::new(stream)
            .stream(name)
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");
        page.chunks
            .iter()
            .flat_map(|chunk| chunk.as_bytes().iter().copied())
            .collect::<Vec<u8>>()
    });

    assert_eq!(bytes, b"ab");
}
