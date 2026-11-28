//! Span tests for the Durable Streams protocol core.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableLimits, DurableService, DurableStream, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StreamName,
};
use std::sync::Arc;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::{global_buf, MockWriter};

/// Prove that `append` and `read` each open a span that names the stream.
///
/// The operations run on one thread under a `tracing` subscriber. The
/// subscriber writes the span-open events to the buffer of the
/// `tracing-test` dev-dependency. The test then reads that buffer.
#[test]
fn given_append_and_read_when_the_service_runs_then_each_opens_a_span_with_the_stream() {
    global_buf().lock().expect("the traced log buffer").clear();

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(MockWriter::new(global_buf()))
        .with_span_events(FmtSpan::NEW)
        .with_ansi(false)
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        // The span callsites cache their interest process-wide. Re-evaluate it
        // now that this subscriber is the current one.
        tracing::callsite::rebuild_interest_cache();
        foundation_core::valtron::block_on_future_scoped(async {
            let service = DurableService::with_limits(
                DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()),
                DurableLimits::default(),
            );
            let name = StreamName::parse("spans/append-read").expect("a valid stream name");

            service
                .create(name.clone(), CreateSpec::default())
                .await
                .expect("create");
            service
                .append(&name, AppendRequest::new(Chunk::new(b"span".to_vec())))
                .await
                .expect("append");
            service
                .read(
                    &name,
                    ReadRequest {
                        from: ReadFrom::Start,
                        limit: ReadLimit::default(),
                    },
                )
                .await
                .expect("read");
        });
    });

    let logs = String::from_utf8(global_buf().lock().expect("the traced log buffer").clone())
        .expect("the traced output is UTF-8");
    for operation in ["append", "read"] {
        assert!(
            logs.contains(&format!("{operation}{{stream=spans/append-read ")),
            "the {operation} operation must open a span with the stream name; saw:\n{logs}"
        );
    }
}
