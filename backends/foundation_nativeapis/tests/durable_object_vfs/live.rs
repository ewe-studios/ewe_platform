//! Live conformance tests against an S3-compatible service.
//!
//! The tests read `EWE_S3_ENDPOINT`, `EWE_S3_ACCESS_KEY`, `EWE_S3_SECRET_KEY`
//! and `EWE_S3_BUCKET`. When a variable is absent, a local run prints one reason
//! line and returns. When `EWE_REQUIRE_LIVE_S3=1` is set, an absent variable
//! fails the test, so continuous integration (CI) never skips in silence.
//!
//! The object store signs each request with the host clock. The virtual file
//! system (VFS) reads its clock from a fixed simulated clock, so the
//! time-to-live (TTL) scenario moves time by hand.
//!
//! The tests create the bucket through the same transport, under a unique object
//! root, and delete the objects at the end.

#![cfg(feature = "vfs-object-netio")]

use crate::sql_support::LibsqlExecutor;
use crate::support::{stream, TempDir};
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use foundation_core::durable::sql::{SqlVfs, SqlVfsConfig};
use foundation_core::durable::vfs_testkit::{
    scenario_append, scenario_bounded_pages, scenario_create, scenario_fork_append,
    scenario_fork_prefix, scenario_read_byte_exact, scenario_soft_delete_cascade,
    scenario_ttl_slide, scenario_version_conflict, ScenarioObservation, StoreFactory, SCENARIOS,
};
use foundation_core::durable::{
    ContentType, DurableStream, DurableStreamVfs, VfsMeta, VfsOffset, WriterState,
};
use foundation_core::valtron::{valtron_test, SimulatedClock};
use foundation_nativeapis::native::vfs::object_http::{
    sign_v4, HttpMethod, HttpObjectStore, ObjectRequest, ObjectTransport, S3Credentials,
    S3HttpConfig,
};
use foundation_nativeapis::native::vfs::object_netio::NetioObjectTransport;
use foundation_nativeapis::native::vfs::s3::S3Settings;
use foundation_nativeapis::native::vfs::s3_blocks::S3BlockStore;
use foundation_nativeapis::shared::vfs::durable::{
    ObjectLayout, ObjectStore, ObjectVfsConfig, PublishPolicy,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The live service settings from the environment.
#[derive(Clone, Debug)]
struct LiveEnv {
    endpoint: String,
    access_key: String,
    secret_key: String,
    bucket: String,
}

impl LiveEnv {
    fn credentials(&self) -> S3Credentials {
        S3Credentials {
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            session_token: None,
        }
    }
}

const VARIABLES: [&str; 4] = [
    "EWE_S3_ENDPOINT",
    "EWE_S3_ACCESS_KEY",
    "EWE_S3_SECRET_KEY",
    "EWE_S3_BUCKET",
];

/// Read the live settings, or return `None` with one reason line.
///
/// # Panics
///
/// Panics when `EWE_REQUIRE_LIVE_S3=1` is set and a variable is missing.
fn live_env() -> Option<LiveEnv> {
    let missing: Vec<&str> = VARIABLES
        .iter()
        .copied()
        .filter(|name| std::env::var(name).is_err())
        .collect();
    if !missing.is_empty() {
        assert!(
            std::env::var("EWE_REQUIRE_LIVE_S3").as_deref() != Ok("1"),
            "EWE_REQUIRE_LIVE_S3=1 but these variables are missing: {}",
            missing.join(", ")
        );
        eprintln!(
            "live S3 tests skipped: missing {} (owner: spec 71 feature 11)",
            missing.join(", ")
        );
        return None;
    }
    Some(LiveEnv {
        endpoint: std::env::var("EWE_S3_ENDPOINT").expect("endpoint"),
        access_key: std::env::var("EWE_S3_ACCESS_KEY").expect("access key"),
        secret_key: std::env::var("EWE_S3_SECRET_KEY").expect("secret key"),
        bucket: std::env::var("EWE_S3_BUCKET").expect("bucket"),
    })
}

fn store_config(env: &LiveEnv) -> S3HttpConfig {
    S3HttpConfig {
        endpoint: env.endpoint.clone(),
        bucket: env.bucket.clone(),
        region: "us-east-1".to_string(),
        path_style: true,
        crc32: false,
        credentials: Some(env.credentials()),
        ..S3HttpConfig::default()
    }
}

/// Build the transport. A literal endpoint address skips Domain Name System
/// (DNS) resolution, so a local service does not wait on a resolver.
fn live_transport(env: &LiveEnv) -> NetioObjectTransport {
    let mut builder = foundation_netio::HttpClientBuilder::new();
    if let Some(addr) = literal_socket_addr(&env.endpoint) {
        builder = builder.resolve_to(addr);
    }
    NetioObjectTransport::with_client(builder.build())
}

fn literal_socket_addr(endpoint: &str) -> Option<std::net::SocketAddr> {
    let rest = endpoint.split_once("://").map_or(endpoint, |(_, rest)| rest);
    let authority = rest.split('/').next()?;
    authority.parse().ok()
}

/// Build the live object store. Signing uses the real host clock.
///
/// Every store in one test shares the transport's connection pool.
fn live_store(
    env: &LiveEnv,
    ctx: &Ctx,
    transport: &NetioObjectTransport,
) -> HttpObjectStore<NetioObjectTransport> {
    HttpObjectStore::new(transport.clone(), store_config(env), ctx)
}

/// The bucket URL for a path-style endpoint.
fn bucket_url(env: &LiveEnv) -> String {
    format!(
        "{}/{}",
        env.endpoint.trim_end_matches('/'),
        env.bucket
    )
}

/// Create the bucket when it does not exist.
///
/// The request is signed and sent through the same transport that the store
/// uses.
async fn ensure_bucket(env: &LiveEnv, ctx: &Ctx, transport: &NetioObjectTransport) {
    let url = bucket_url(env);
    let host = url.split_once("://").map_or_else(
        || url.clone(),
        |(_, rest)| rest.split('/').next().unwrap_or(rest).to_string(),
    );
    let mut request = ObjectRequest {
        method: HttpMethod::Put,
        url,
        query: Vec::new(),
        headers: vec![
            ("host".to_string(), host),
            ("content-length".to_string(), "0".to_string()),
        ],
        body: Vec::new(),
        max_response_bytes: 8 * 1024,
    };
    sign_v4(
        &mut request,
        &env.credentials(),
        "us-east-1",
        unix_millis(ctx.clock().wall_now()),
    );
    let response = transport.send(request).await.expect("create bucket request");
    assert!(
        response.status == 200 || response.status == 409,
        "create bucket returned status {}",
        response.status
    );
}

fn unique_root(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_nanos();
    format!(
        "spec71/feature11/{tag}-{}-{nanos}",
        std::process::id()
    )
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

fn live_vfs(
    env: &LiveEnv,
    ctx: &Ctx,
    transport: &NetioObjectTransport,
    spool: PathBuf,
    root: String,
    policy: PublishPolicy,
) -> Arc<foundation_nativeapis::native::vfs::s3::S3Vfs> {
    let host = Ctx::host();
    let store = live_store(env, &host, transport);
    let config = ObjectVfsConfig::default()
        .with_spool_root(spool)
        .with_root(root)
        .with_layout(ObjectLayout::Segmented)
        .with_publish_policy(policy);
    Arc::new(
        foundation_nativeapis::native::vfs::s3::S3Vfs::new(
            Arc::new(store),
            ctx,
            config,
            S3Settings::default().with_path_style(true),
        )
        .expect("live S3 VFS"),
    )
}

async fn cleanup(env: &LiveEnv, transport: &NetioObjectTransport, root: &str) {
    let host = Ctx::host();
    let store = live_store(env, &host, transport);
    let Ok(entries) = store.list(root).await else {
        return;
    };
    let keys: Vec<String> = entries.into_iter().map(|entry| entry.key).collect();
    if !keys.is_empty() {
        let _ = store.delete_many(keys).await;
    }
}

fn fresh_clock() -> (Arc<SimulatedClock>, Ctx) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let as_dyn: Arc<dyn foundation_context::Clock> = clock.clone();
    (clock, Ctx::fixed(as_dyn))
}

/// Run every shared scenario on a factory that the executor can move.
///
/// This mirrors [`foundation_core::durable::vfs_testkit::run_all`]. The shared
/// helper borrows a `&dyn StoreFactory`, so its future is not `Send` and cannot
/// run on the valtron pool that the network client needs. This version owns the
/// factory, so the future is `Send`.
async fn run_all_send<F>(factory: F) -> Vec<ScenarioObservation>
where
    F: StoreFactory + Send + Sync + 'static,
{
    let mut results = Vec::with_capacity(SCENARIOS.len());
    results.push(scenario_create(&factory.build().0).await);
    results.push(scenario_append(&factory.build().0).await);
    results.push(scenario_read_byte_exact(&factory.build().0).await);
    results.push(scenario_bounded_pages(&factory.build().0).await);
    results.push(scenario_fork_prefix(&factory.build().0).await);
    results.push(scenario_fork_append(&factory.build().0).await);
    results.push(scenario_soft_delete_cascade(&factory.build().0).await);
    {
        let (stream, clock) = factory.build();
        results.push(scenario_ttl_slide(&stream, &clock).await);
    }
    results.push(scenario_version_conflict(&factory.build().0).await);
    results
}

#[valtron_test]
async fn live_s3_conformance() {
    let Some(env) = live_env() else { return };
    let host = Ctx::host();
    let transport = live_transport(&env);
    ensure_bucket(&env, &host, &transport).await;
    let root = unique_root("conformance");
    let temp = TempDir::new("live_s3_conformance");
    let counter = AtomicUsize::new(0);
    let spool = temp.path().to_path_buf();
    let factory = {
        let env = env.clone();
        let root = root.clone();
        let transport = transport.clone();
        move || {
            let index = counter.fetch_add(1, Ordering::SeqCst);
            let (clock, ctx) = fresh_clock();
            let vfs = live_vfs(
                &env,
                &ctx,
                &transport,
                spool.join(format!("store-{index}")),
                format!("{root}/{index}"),
                PublishPolicy::Immediate,
            );
            (DurableStream::with_ctx(vfs, &ctx), clock)
        }
    };
    let report = run_all_send(factory).await;
    let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();
    assert_eq!(names, SCENARIOS, "the live S3 VFS runs every scenario");
    assert!(
        report.iter().all(|scenario| !scenario.values.is_empty()),
        "every live S3 scenario observes a value"
    );
    cleanup(&env, &transport, &root).await;
}

#[valtron_test]
async fn live_s3_two_writers_one_cas_wins() {
    let Some(env) = live_env() else { return };
    let host = Ctx::host();
    let transport = live_transport(&env);
    ensure_bucket(&env, &host, &transport).await;
    let root = unique_root("two-writers");
    let temp = TempDir::new("live_s3_two_writers");
    let (clock_a, ctx_a) = fresh_clock();
    let (_clock_b, ctx_b) = fresh_clock();
    let writer_a = live_vfs(
        &env,
        &ctx_a,
        &transport,
        temp.path().join("writer-a"),
        root.clone(),
        PublishPolicy::Immediate,
    );
    let writer_b = live_vfs(
        &env,
        &ctx_b,
        &transport,
        temp.path().join("writer-b"),
        root.clone(),
        PublishPolicy::Immediate,
    );
    let _ = clock_a;
    let name = stream("live/two-writers");
    writer_a.create(&name, meta()).await.expect("create");

    let (first, second) = futures_lite::future::zip(
        writer_a.append(&name, b"alpha"),
        writer_b.append(&name, b"beta"),
    )
    .await;
    first.expect("writer A append");
    second.expect("writer B append");

    let (reader_clock, reader_ctx) = fresh_clock();
    let _ = reader_clock;
    let reader = live_vfs(
        &env,
        &reader_ctx,
        &transport,
        temp.path().join("reader"),
        root.clone(),
        PublishPolicy::Immediate,
    );
    let bytes = reader
        .read_at(&name, &VfsOffset(0), 1024)
        .await
        .expect("read");
    assert_eq!(bytes.len(), 9, "both batches reach the store");
    assert!(
        bytes == b"alphabeta" || bytes == b"betaalpha",
        "each writer's batch appears exactly once, in CAS order"
    );
    let stored = reader.meta(&name).await.expect("metadata");
    assert_eq!(stored.len, 9, "no acknowledged batch is lost");
    cleanup(&env, &transport, &root).await;
}

#[valtron_test]
async fn live_s3_restart_replays_the_spool() {
    let Some(env) = live_env() else { return };
    let host = Ctx::host();
    let transport = live_transport(&env);
    ensure_bucket(&env, &host, &transport).await;
    let root = unique_root("restart");
    let temp = TempDir::new("live_s3_restart");
    let spool = temp.path().join("writer");
    let (clock, ctx) = fresh_clock();
    let _ = clock;
    let name = stream("live/restart");

    let first = live_vfs(
        &env,
        &ctx,
        &transport,
        spool.clone(),
        root.clone(),
        PublishPolicy::Batched,
    );
    first.create(&name, meta()).await.expect("create");
    first
        .append(&name, b"acknowledged")
        .await
        .expect("append returns after the spool fsync");
    drop(first);

    // A new process opens the same spool. The replay publishes the pending
    // batch to the object store.
    let second = live_vfs(
        &env,
        &ctx,
        &transport,
        spool,
        root.clone(),
        PublishPolicy::Batched,
    );
    let flushed = second.flush_stream(&name).await.expect("replay flush");
    assert_eq!(flushed.blocks_put, 1, "the replay publishes the spool batch");

    let (_reader_clock, reader_ctx) = fresh_clock();
    let reader = live_vfs(
        &env,
        &reader_ctx,
        &transport,
        temp.path().join("reader"),
        root.clone(),
        PublishPolicy::Batched,
    );
    let bytes = reader
        .read_at(&name, &VfsOffset(0), 1024)
        .await
        .expect("read after restart");
    assert_eq!(
        bytes, b"acknowledged",
        "the restart loses no acknowledged data"
    );
    cleanup(&env, &transport, &root).await;
}

#[valtron_test]
async fn live_sql_vfs_flushes_to_s3() {
    let Some(env) = live_env() else { return };
    let host = Ctx::host();
    let transport = live_transport(&env);
    ensure_bucket(&env, &host, &transport).await;
    let root = unique_root("sql-blocks");
    let (_clock, ctx) = fresh_clock();
    let name = stream("live/sql-blocks");
    let store = live_store(&env, &host, &transport);
    let blocks = Arc::new(S3BlockStore::new(store));
    let executor = Arc::new(LibsqlExecutor::open().await);
    let config = SqlVfsConfig::default()
        .with_batch_age(Duration::ZERO)
        .with_max_block_bytes(1024)
        .with_vacuum_threshold(1)
        .with_block_prefix(root.clone());
    let vfs = SqlVfs::new(executor, blocks, &ctx, config);
    vfs.create(&name, meta()).await.expect("create");
    vfs.append(&name, b"hello ").await.expect("append");
    vfs.append(&name, b"world").await.expect("append");
    let report = vfs.flush().await.expect("flush to S3");
    assert_eq!(report.blocks_put, 1, "the SQL VFS flushes one block to S3");
    vfs.vacuum().await.expect("vacuum");
    let bytes = vfs
        .read_at(&name, &VfsOffset(6), 5)
        .await
        .expect("range read from S3");
    assert_eq!(bytes, b"world", "the block range read is byte-exact");
    cleanup(&env, &transport, &root).await;
}
