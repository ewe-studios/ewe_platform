//! Malformed-input fuzz oracle.
//!
//! The property test feeds random offsets, `Stream-Seq` values, JSON bodies and header fields
//! to the public protocol API. It asserts that no input panics and that an invalid input
//! returns a typed [`StoreError`].

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, BadRequestKind, Chunk, ContentType, CreateSpec, DurableStream, DurableStreams,
    Expiry, MemoryVfs, Producer, ProducerId, ReadFrom, ReadLimit, ReadRequest, StoreError,
    StreamName, StreamOffset, StreamSeq, wire,
};
use proptest::prelude::*;
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

fn name(raw: &str) -> StreamName {
    StreamName::parse(raw).expect("valid test name")
}

fn json_spec() -> CreateSpec {
    CreateSpec {
        content_type: Some(ContentType::parse("application/json").expect("valid type")),
        ..CreateSpec::default()
    }
}

/// The random header and field values one case uses.
struct Inputs {
    offset_bytes: Vec<u8>,
    seq_bytes: Vec<u8>,
    body: Vec<u8>,
    content_type: Option<String>,
    stream_name: String,
    producer_id: Option<String>,
    ttl: Option<u64>,
    expires_at: Option<i64>,
    epoch: u64,
    producer_seq: u64,
    close: bool,
}

/// True for the protocol refusals an append may return.
fn is_append_refusal(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::NotFound
            | StoreError::Gone
            | StoreError::Trimmed
            | StoreError::BadRequest(_)
            | StoreError::ContentTypeMismatch { .. }
            | StoreError::SequenceGap { .. }
            | StoreError::Fenced { .. }
            | StoreError::InvalidEpochSeq { .. }
            | StoreError::StreamSeqConflict { .. }
            | StoreError::Closed { .. }
            | StoreError::ConfigConflict
            | StoreError::Unsupported
    )
}

async fn check(inputs: &Inputs) -> Result<(), String> {
    check_offsets(inputs).await?;
    check_stream_seq(inputs).await?;
    check_json_body(inputs).await?;
    check_headers(inputs).await
}

/// Random bytes as offsets. The wire decoder returns `None` for bad text; the
/// store returns a typed error for an offset that is not a cursor.
async fn check_offsets(inputs: &Inputs) -> Result<(), String> {
    let text = String::from_utf8_lossy(&inputs.offset_bytes);
    if let Some(offset) = wire::parse_offset(&text) {
        let round = wire::parse_offset(&wire::offset_text(&offset));
        if round.as_ref() != Some(&offset) {
            return Err(format!("offset text did not round trip: {offset:?}"));
        }
    }
    let streams = streams();
    let stream = streams.stream(name("fuzz/offset"));
    stream
        .create(json_spec())
        .await
        .map_err(|error| format!("offset stream create failed: {error:?}"))?;
    stream
        .append(AppendRequest::new(Chunk::new(b"{\"a\":1}".to_vec())))
        .await
        .map_err(|error| format!("offset stream append failed: {error:?}"))?;
    let read = stream
        .read(ReadRequest {
            from: ReadFrom::Offset(StreamOffset::from_bytes(inputs.offset_bytes.clone())),
            limit: ReadLimit::default(),
        })
        .await;
    match read {
        Ok(_) | Err(StoreError::BadRequest(BadRequestKind::Json)) => Ok(()),
        Err(other) => Err(format!(
            "random offset returned an untyped error: {other:?}"
        )),
    }
}

/// Random `Stream-Seq` values. The first value is accepted; the same value
/// again is a typed conflict.
async fn check_stream_seq(inputs: &Inputs) -> Result<(), String> {
    let streams = streams();
    let stream = streams.stream(name("fuzz/stream-seq"));
    stream
        .create(CreateSpec::default())
        .await
        .map_err(|error| format!("seq stream create failed: {error:?}"))?;
    let seq = StreamSeq::from_bytes(inputs.seq_bytes.clone());
    stream
        .append(AppendRequest {
            stream_seq: Some(seq.clone()),
            ..AppendRequest::new(Chunk::new(b"x".to_vec()))
        })
        .await
        .map_err(|error| format!("first Stream-Seq append failed: {error:?}"))?;
    let repeat = stream
        .append(AppendRequest {
            stream_seq: Some(seq),
            ..AppendRequest::new(Chunk::new(b"y".to_vec()))
        })
        .await;
    match repeat {
        Err(StoreError::StreamSeqConflict { .. }) => Ok(()),
        Ok(_) => Err("a repeated Stream-Seq was accepted".into()),
        Err(other) => Err(format!("a repeated Stream-Seq gave {other:?}")),
    }
}

/// Random JSON bodies. A valid body is stored; a malformed body is a typed
/// JSON refusal.
async fn check_json_body(inputs: &Inputs) -> Result<(), String> {
    let streams = streams();
    let stream = streams.stream(name("fuzz/json"));
    stream
        .create(json_spec())
        .await
        .map_err(|error| format!("json stream create failed: {error:?}"))?;
    match stream
        .append(AppendRequest::new(Chunk::new(inputs.body.clone())))
        .await
    {
        Ok(_) | Err(StoreError::BadRequest(BadRequestKind::Json)) => Ok(()),
        Err(other) => Err(format!("random JSON body gave {other:?}")),
    }
}

/// Random header and field combinations. Each parser returns a typed refusal
/// or a value; the combined request returns a protocol refusal.
async fn check_headers(inputs: &Inputs) -> Result<(), String> {
    if let Some(raw) = &inputs.content_type {
        if let Err(error) = ContentType::parse(raw) {
            if !matches!(error, StoreError::BadRequest(BadRequestKind::Header)) {
                return Err(format!("content type gave {error:?}"));
            }
        }
    }
    if let Err(error) = StreamName::parse(&inputs.stream_name) {
        if !matches!(error, StoreError::BadRequest(BadRequestKind::Header)) {
            return Err(format!("stream name gave {error:?}"));
        }
    }
    if let Some(raw) = &inputs.producer_id {
        if let Err(error) = ProducerId::parse(raw) {
            if !matches!(error, StoreError::BadRequest(BadRequestKind::Producer)) {
                return Err(format!("producer id gave {error:?}"));
            }
        }
    }
    let expiry = Expiry::from_headers(
        inputs.ttl.map(core::time::Duration::from_secs),
        inputs.expires_at,
    );
    let expiry = match expiry {
        Ok(expiry) => expiry,
        Err(StoreError::BadRequest(BadRequestKind::Header)) => None,
        Err(other) => return Err(format!("expiry headers gave {other:?}")),
    };

    let streams = streams();
    let stream = streams.stream(name("fuzz/headers"));
    stream
        .create(CreateSpec {
            content_type: inputs
                .content_type
                .as_deref()
                .and_then(|raw| ContentType::parse(raw).ok()),
            expires: expiry,
            ..CreateSpec::default()
        })
        .await
        .map_err(|error| format!("header stream create failed: {error:?}"))?;
    let producer = inputs
        .producer_id
        .as_deref()
        .and_then(|raw| ProducerId::parse(raw).ok())
        .map(|id| Producer {
            id,
            epoch: inputs.epoch,
            seq: inputs.producer_seq,
        });
    let append = stream
        .append(AppendRequest {
            chunk: Chunk::new(inputs.body.clone()),
            content_type: inputs
                .content_type
                .as_deref()
                .and_then(|raw| ContentType::parse(raw).ok()),
            stream_seq: Some(StreamSeq::from_bytes(inputs.seq_bytes.clone())),
            producer,
            close: inputs.close,
        })
        .await;
    match append {
        Ok(_) => Ok(()),
        Err(error) if is_append_refusal(&error) => Ok(()),
        Err(other) => Err(format!("combined fields gave {other:?}")),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })]

    #[test]
    fn malformed_input_never_panics(
        offset_bytes in prop::collection::vec(any::<u8>(), 0..=32),
        seq_bytes in prop::collection::vec(any::<u8>(), 0..=32),
        body in prop::collection::vec(any::<u8>(), 0..=96),
        content_type in prop::option::of(prop::collection::vec(any::<char>(), 0..=16)),
        stream_name in prop::collection::vec(any::<char>(), 0..=16),
        producer_id in prop::option::of(prop::collection::vec(any::<char>(), 0..=16)),
        ttl in prop::option::of(0u64..=1_000_000_000),
        expires_at in prop::option::of(any::<i64>()),
        epoch in any::<u64>(),
        producer_seq in any::<u64>(),
        close in any::<bool>(),
    ) {
        let inputs = Inputs {
            offset_bytes,
            seq_bytes,
            body,
            content_type: content_type.map(|chars| chars.into_iter().collect()),
            stream_name: stream_name.into_iter().collect(),
            producer_id: producer_id.map(|chars| chars.into_iter().collect()),
            ttl,
            expires_at,
            epoch,
            producer_seq,
            close,
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            foundation_core::valtron::block_on_future_scoped(check(&inputs))
        }));
        let Ok(checked) = outcome else {
            panic!("malformed input panicked");
        };
        assert!(
            checked.is_ok(),
            "{}",
            checked.err().unwrap_or_default()
        );
    }
}
