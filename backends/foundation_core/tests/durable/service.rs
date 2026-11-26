//! Tests for the transport-neutral service.

use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableLimits, DurableService, DurableStream, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StreamName,
};
use std::sync::Arc;

#[test]
fn given_a_service_when_the_business_api_runs_then_it_uses_the_driver() {
    smol::block_on(async {
        let service = DurableService::with_limits(
            DurableStream::new(Arc::new(MemoryVfs::new())),
            DurableLimits::default(),
        );
        let name = StreamName::parse("service/basic").expect("valid");

        service
            .create(name.clone(), CreateSpec::default())
            .await
            .expect("create");
        service
            .append(&name, AppendRequest::new(Chunk::new(b"service".to_vec())))
            .await
            .expect("append");

        let page = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.as_bytes().iter().copied())
            .collect();

        assert_eq!(bytes, b"service");
        assert_eq!(service.limits().page_bytes, 1024 * 1024);
    });
}

#[test]
fn given_protocol_limits_when_built_then_each_value_is_configurable() {
    let limits = foundation_core::durable::DurableLimits::default()
        .with_page_bytes(4096)
        .with_page_chunks(8)
        .with_long_poll_timeout_ms(1_000)
        .with_sse_lifetime_ms(5_000);

    assert_eq!(limits.page_bytes, 4096);
    assert_eq!(limits.page_chunks, 8);
    assert_eq!(limits.long_poll_timeout_ms, 1_000);
    assert_eq!(limits.sse_lifetime_ms, 5_000);
}

#[test]
fn given_a_page_limit_when_a_read_runs_then_no_page_exceeds_it() {
    smol::block_on(async {
        use foundation_core::durable::{
            AppendRequest, Chunk, CreateSpec, DurableLimits, DurableService, DurableStream,
            MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StreamName,
        };
        use std::sync::Arc;

        let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
        let service = DurableService::with_limits(
            stream,
            DurableLimits::default().with_page_bytes(4),
        );
        let name = StreamName::parse("bounded/page").expect("name");
        service
            .create(name.clone(), CreateSpec::default())
            .await
            .expect("create");
        service
            .append(
                &name,
                AppendRequest::new(Chunk::new(b"0123456789".to_vec())),
            )
            .await
            .expect("append");

        let mut offset = ReadFrom::Start;
        let mut total = Vec::new();
        let mut pages = 0;
        loop {
            let page = service
                .read(
                    &name,
                    ReadRequest {
                        from: offset.clone(),
                        limit: ReadLimit::default(),
                    },
                )
                .await
                .expect("read");
            pages += 1;
            for chunk in &page.chunks {
                assert!(chunk.as_bytes().len() <= 4, "page exceeded the byte limit");
                total.extend_from_slice(chunk.as_bytes());
            }
            if page.cursor.up_to_date {
                break;
            }
            offset = ReadFrom::Offset(page.cursor.next.clone());
        }
        assert_eq!(total, b"0123456789");
        assert!(pages >= 3, "the limit forces several pages");
    });
}

#[test]
fn given_a_chunk_limit_when_a_json_read_runs_then_no_page_exceeds_it() {
    smol::block_on(async {
        use foundation_core::durable::{
            AppendRequest, Chunk, ContentType, CreateSpec, DurableLimits, DurableService,
            DurableStream, MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StreamName,
        };
        use std::sync::Arc;

        let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
        let service = DurableService::with_limits(
            stream,
            DurableLimits::default().with_page_chunks(2),
        );
        let name = StreamName::parse("bounded/chunks").expect("name");
        service
            .create(
                name.clone(),
                CreateSpec {
                    content_type: Some(ContentType::parse("application/json").expect("type")),
                    ..CreateSpec::default()
                },
            )
            .await
            .expect("create");
        for value in [r#"{"n":1}"#, r#"{"n":2}"#, r#"{"n":3}"#] {
            service
                .append(
                    &name,
                    AppendRequest {
                        chunk: Chunk::new(value.as_bytes().to_vec()),
                        content_type: None,
                        stream_seq: None,
                        producer: None,
                        close: false,
                    },
                )
                .await
                .expect("append");
        }

        let first = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        assert_eq!(first.chunks.len(), 2);
        assert!(!first.cursor.up_to_date);

        let second = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Offset(first.cursor.next.clone()),
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        assert_eq!(second.chunks.len(), 1);
        assert!(second.cursor.up_to_date);
    });
}

#[test]
fn given_a_byte_limit_when_a_json_read_runs_then_the_page_respects_it() {
    smol::block_on(async {
        use foundation_core::durable::{
            AppendRequest, Chunk, ContentType, CreateSpec, DurableLimits, DurableService,
            DurableStream, MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StreamName,
        };
        use std::sync::Arc;

        let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
        let service = DurableService::with_limits(
            stream,
            DurableLimits::default().with_page_bytes(4),
        );
        let name = StreamName::parse("bounded/json-bytes").expect("name");
        service
            .create(
                name.clone(),
                CreateSpec {
                    content_type: Some(ContentType::parse("application/json").expect("type")),
                    ..CreateSpec::default()
                },
            )
            .await
            .expect("create");
        for value in ["1", "2", "3"] {
            service
                .append(
                    &name,
                    AppendRequest {
                        chunk: Chunk::new(value.as_bytes().to_vec()),
                        content_type: None,
                        stream_seq: None,
                        producer: None,
                        close: false,
                    },
                )
                .await
                .expect("append");
        }

        let first = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        assert_eq!(first.chunks.len(), 2);
        assert!(!first.cursor.up_to_date);
    });
}

#[test]
fn given_a_message_larger_than_the_page_when_it_is_read_then_it_arrives_whole() {
    smol::block_on(async {
        use foundation_core::durable::{
            AppendRequest, Chunk, ContentType, CreateSpec, DurableLimits, DurableService,
            DurableStream, MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StreamName,
        };
        use std::sync::Arc;

        let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
        let service = DurableService::with_limits(
            stream,
            DurableLimits::default().with_page_bytes(4),
        );
        let name = StreamName::parse("bounded/large-message").expect("name");
        service
            .create(
                name.clone(),
                CreateSpec {
                    content_type: Some(ContentType::parse("application/json").expect("type")),
                    ..CreateSpec::default()
                },
            )
            .await
            .expect("create");
        let message = br#"{"text":"a message longer than four bytes"}"#;
        service
            .append(
                &name,
                AppendRequest {
                    chunk: Chunk::new(message.to_vec()),
                    content_type: None,
                    stream_seq: None,
                    producer: None,
                    close: false,
                },
            )
            .await
            .expect("append");

        let page = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        assert_eq!(page.chunks.len(), 1);
        assert_eq!(page.chunks[0].as_bytes(), message);
        assert!(page.cursor.up_to_date);
    });
}
