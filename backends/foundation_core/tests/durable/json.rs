//! BDD scenarios for JSON mode.

use foundation_core::durable::{
    AppendRequest, BadRequestKind, Chunk, ContentType, CreateSpec, DurableStream, DurableStreams,
    MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName,
};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new())))
}

fn json_spec() -> CreateSpec {
    CreateSpec {
        content_type: Some(ContentType::parse("application/json").expect("valid")),
        ..CreateSpec::default()
    }
}

fn chunk(data: &'static str) -> Chunk {
    Chunk::new(data.as_bytes().to_vec())
}

async fn create(streams: &DurableStreams<DurableStream>, name: &StreamName, spec: CreateSpec) {
    streams
        .stream(name.clone())
        .create(spec)
        .await
        .expect("create");
}

async fn read_chunks(streams: &DurableStreams<DurableStream>, name: &StreamName) -> Vec<String> {
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
        .map(|chunk| String::from_utf8(chunk.as_bytes().to_vec()).expect("utf8"))
        .collect()
}

#[test]
fn given_a_json_stream_when_an_array_appends_then_it_flattens_one_level() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/flatten").expect("valid");
        create(&streams, &name, json_spec()).await;

        streams
            .stream(name.clone())
            .append(AppendRequest::new(chunk(r#"[{"a":1},{"b":2}]"#)))
            .await
            .expect("append");

        assert_eq!(
            read_chunks(&streams, &name).await,
            vec![r#"{"a":1}"#, r#"{"b":2}"#]
        );
    });
}

#[test]
fn given_a_json_stream_when_a_single_value_appends_then_it_is_one_message() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/single").expect("valid");
        create(&streams, &name, json_spec()).await;

        streams
            .stream(name.clone())
            .append(AppendRequest::new(chunk(r#"{"x":1}"#)))
            .await
            .expect("append");

        assert_eq!(read_chunks(&streams, &name).await, vec![r#"{"x":1}"#]);
    });
}

#[test]
fn given_a_json_stream_when_nested_arrays_append_then_only_one_level_flattens() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/nested").expect("valid");
        create(&streams, &name, json_spec()).await;

        streams
            .stream(name.clone())
            .append(AppendRequest::new(chunk("[[1,2],[3,4]]")))
            .await
            .expect("append");
        assert_eq!(read_chunks(&streams, &name).await, vec!["[1,2]", "[3,4]"]);

        streams
            .stream(name.clone())
            .append(AppendRequest::new(chunk("[[[1,2,3]]]")))
            .await
            .expect("append");
        assert_eq!(
            read_chunks(&streams, &name).await,
            vec!["[1,2]", "[3,4]", "[[1,2,3]]"]
        );
    });
}

#[test]
fn given_a_json_stream_when_the_append_is_an_empty_array_then_it_is_refused() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/empty-append").expect("valid");
        create(&streams, &name, json_spec()).await;

        let error = streams
            .stream(name)
            .append(AppendRequest::new(chunk("[]")))
            .await
            .expect_err("empty array");

        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Json)
        ));
    });
}

#[test]
fn given_a_json_stream_when_it_is_created_with_an_empty_array_then_it_is_valid() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/empty-create").expect("valid");
        create(
            &streams,
            &name,
            CreateSpec {
                initial: Some(chunk("[]")),
                ..json_spec()
            },
        )
        .await;

        assert!(read_chunks(&streams, &name).await.is_empty());
    });
}

#[test]
fn given_a_json_stream_when_the_append_is_invalid_then_it_is_refused() {
    smol::block_on(async {
        let streams = streams();
        let name = StreamName::parse("json/invalid").expect("valid");
        create(&streams, &name, json_spec()).await;

        let error = streams
            .stream(name)
            .append(AppendRequest::new(chunk("{not json")))
            .await
            .expect_err("invalid json");

        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Json)
        ));
    });
}

#[test]
fn given_an_offset_inside_a_json_message_when_a_read_runs_then_it_is_a_bad_request() {
    smol::block_on(async {
        use foundation_core::durable::{
            AppendRequest, Chunk, ContentType, CreateSpec, DurableStream, DurableStreams,
            MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName, StreamOffset,
        };
        use std::sync::Arc;

        let streams = DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new())));
        let stream = streams.stream(StreamName::parse("json/mid-offset").expect("name"));
        stream
            .create(CreateSpec {
                content_type: Some(ContentType::parse("application/json").expect("type")),
                ..CreateSpec::default()
            })
            .await
            .expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"{\"a\":1}".to_vec())))
            .await
            .expect("append");

        // Offset 4 lands inside the message. Only a cursor offset is valid.
        let error = stream
            .read(ReadRequest {
                from: ReadFrom::Offset(StreamOffset::from_bytes(4_u64.to_be_bytes().to_vec())),
                limit: ReadLimit::default(),
            })
            .await
            .expect_err("mid-message offset");
        assert!(matches!(
            error,
            StoreError::BadRequest(foundation_core::durable::BadRequestKind::Json)
        ));
    });
}
