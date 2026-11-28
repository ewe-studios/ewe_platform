//! Tests for the S3 block tier of the SQL virtual file system (VFS).
//!
//! A recording transport stands in for the object service. It stores every PUT
//! and serves a byte range from the stored object, so the tests prove the
//! request shaping and the exact range read without a live service.

#![cfg(feature = "vfs-object-http")]

use crate::sql_support::LibsqlExecutor;
use foundation_context::Ctx;
use foundation_core::durable::sql::{SqlVfs, SqlVfsConfig};
use foundation_core::durable::{
    ContentType, DurableStreamVfs, StreamName, VfsMeta, VfsOffset, WriterState,
};
use foundation_core::valtron::{block_on_future_scoped, SimulatedClock};
use foundation_nativeapis::native::vfs::object_http::{
    HttpMethod, HttpObjectStore, ObjectRequest, ObjectResponse, ObjectTransport, S3HttpConfig,
};
use foundation_nativeapis::native::vfs::s3_blocks::S3BlockStore;
use foundation_nativeapis::shared::vfs::durable::ObjectFuture;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One recorded PUT: the request path and the body.
type Puts = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
/// One recorded GET: the request path and its `Range` header, when present.
type Gets = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// A transport that behaves like a small object service.
#[derive(Clone, Default)]
struct ObjectMap {
    objects: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    puts: Puts,
    gets: Gets,
}

fn path_of(request: &ObjectRequest) -> String {
    request
        .url
        .split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|index| rest[index..].to_string()))
        .unwrap_or_else(|| request.url.clone())
}

fn header<'a>(request: &'a ObjectRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn parse_range(value: &str) -> Option<(usize, usize)> {
    let value = value.strip_prefix("bytes=")?;
    let (start, end) = value.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?))
}

impl ObjectTransport for ObjectMap {
    fn send(&self, request: ObjectRequest) -> ObjectFuture<'_, ObjectResponse> {
        let objects = Arc::clone(&self.objects);
        let puts = Arc::clone(&self.puts);
        let gets = Arc::clone(&self.gets);
        Box::pin(async move {
            let path = path_of(&request);
            match request.method {
                HttpMethod::Put => {
                    puts.lock()
                        .expect("put log")
                        .push((path.clone(), request.body.clone()));
                    objects.lock().expect("objects").insert(path, request.body);
                    Ok(ObjectResponse {
                        status: 200,
                        headers: vec![("etag".to_string(), "\"v1\"".to_string())],
                        body: Vec::new(),
                    })
                }
                HttpMethod::Get => {
                    let range = header(&request, "range").map(str::to_string);
                    gets.lock()
                        .expect("get log")
                        .push((path.clone(), range.clone()));
                    let stored = objects.lock().expect("objects").get(&path).cloned();
                    let Some(stored) = stored else {
                        return Ok(ObjectResponse {
                            status: 404,
                            headers: Vec::new(),
                            body: Vec::new(),
                        });
                    };
                    let Some(range) = range else {
                        return Ok(ObjectResponse {
                            status: 200,
                            headers: vec![("content-length".to_string(), stored.len().to_string())],
                            body: stored,
                        });
                    };
                    let Some((start, end)) = parse_range(&range) else {
                        return Ok(ObjectResponse {
                            status: 400,
                            headers: Vec::new(),
                            body: Vec::new(),
                        });
                    };
                    let end = end.saturating_add(1).min(stored.len());
                    let start = start.min(stored.len());
                    let slice = stored[start..end].to_vec();
                    Ok(ObjectResponse {
                        status: 206,
                        headers: vec![("content-length".to_string(), slice.len().to_string())],
                        body: slice,
                    })
                }
                HttpMethod::Head => {
                    let stored = objects.lock().expect("objects").get(&path).cloned();
                    match stored {
                        Some(stored) => Ok(ObjectResponse {
                            status: 200,
                            headers: vec![
                                ("content-length".to_string(), stored.len().to_string()),
                                ("etag".to_string(), "\"v1\"".to_string()),
                            ],
                            body: Vec::new(),
                        }),
                        None => Ok(ObjectResponse {
                            status: 404,
                            headers: Vec::new(),
                            body: Vec::new(),
                        }),
                    }
                }
                HttpMethod::Delete => {
                    objects.lock().expect("objects").remove(&path);
                    Ok(ObjectResponse {
                        status: 204,
                        headers: Vec::new(),
                        body: Vec::new(),
                    })
                }
                HttpMethod::Post => Ok(ObjectResponse {
                    status: 200,
                    headers: Vec::new(),
                    body: Vec::new(),
                }),
            }
        })
    }
}

fn block_store(objects: &ObjectMap, ctx: &Ctx) -> S3BlockStore<ObjectMap> {
    let config = S3HttpConfig {
        endpoint: "http://s3.test".to_string(),
        bucket: "blocks".to_string(),
        path_style: true,
        ..S3HttpConfig::default()
    };
    S3BlockStore::new(HttpObjectStore::new(objects.clone(), config, ctx))
}

fn stream_name(text: &str) -> StreamName {
    StreamName::parse(text).expect("valid stream name")
}

fn meta() -> VfsMeta {
    VfsMeta {
        content_type: ContentType::parse("application/octet-stream").expect("content type"),
        len: 0,
        closed: false,
        expires: None,
        writer: WriterState::default(),
        fork: None,
        last_access_ms: 0,
        trim_from: 0,
        version: 0,
    }
}

fn zero_age() -> SqlVfsConfig {
    SqlVfsConfig::default()
        .with_batch_age(Duration::ZERO)
        .with_max_block_bytes(1024)
        .with_vacuum_threshold(1)
}

#[test]
fn sql_vfs_flushes_to_s3() {
    block_on_future_scoped(async {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock);
        let objects = ObjectMap::default();
        let executor = Arc::new(LibsqlExecutor::open().await);
        let blocks = Arc::new(block_store(&objects, &ctx));
        let vfs = SqlVfs::new(executor, blocks, &ctx, zero_age());
        let name = stream_name("sql/blocks");

        vfs.create(&name, meta()).await.expect("create");
        vfs.append(&name, b"hello ").await.expect("append");
        vfs.append(&name, b"world").await.expect("append");
        let report = vfs.flush().await.expect("flush");
        assert_eq!(report.blocks_put, 1, "one block reaches the object store");

        let puts = objects.puts.lock().expect("put log");
        let (path, block) = puts.first().expect("one PUT");
        assert!(path.ends_with("/0-11"), "the key names the covered range");
        assert_eq!(&block[..8], b"DSBLK001", "the block carries its magic");
    });
}

#[test]
fn sql_vfs_reads_a_block_range_from_s3() {
    block_on_future_scoped(async {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock);
        let objects = ObjectMap::default();
        let executor = Arc::new(LibsqlExecutor::open().await);
        let blocks = Arc::new(block_store(&objects, &ctx));
        let vfs = SqlVfs::new(executor, blocks, &ctx, zero_age());
        let name = stream_name("sql/blocks-range");

        vfs.create(&name, meta()).await.expect("create");
        vfs.append(&name, b"hello ").await.expect("append");
        vfs.append(&name, b"world").await.expect("append");
        vfs.flush().await.expect("flush");
        vfs.vacuum().await.expect("vacuum");

        let bytes = vfs
            .read_at(&name, &VfsOffset(6), 5)
            .await
            .expect("range read");
        assert_eq!(bytes, b"world");

        let gets = objects.gets.lock().expect("get log");
        assert!(
            gets.iter().any(|(_, range)| range.is_some()),
            "the vacuumed read asks the object store for a byte range"
        );
    });
}
