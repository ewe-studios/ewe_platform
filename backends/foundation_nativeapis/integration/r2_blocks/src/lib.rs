//! A Worker fixture that proves the Cloudflare R2 block store on a real local
//! R2 binding.
//!
//! WHY: the R2 binding only exists inside the Workers runtime. A native test
//! cannot call it.
//! WHAT: `wrangler dev --local` starts workerd with a real local R2 bucket and a
//! local D1 database. A `GET /r2-blocks` request runs the block scenarios.
//! HOW: the route asserts the conditional-write semantics with the bare R2
//! etag, then runs a block put, an idempotent retry, an exact range read, and
//! the SQL virtual file system (VFS) flush and vacuumed read.

use foundation_context::Ctx;
use foundation_core::durable::sql::{
    BlockStore, SqlExecutor, SqlStatement, SqlVfs, SqlVfsConfig,
};
use foundation_core::durable::{
    ContentType, DurableStreamVfs, StreamName, VfsMeta, VfsOffset, WriterState,
};
use foundation_db::wasm::bindgen::{R2Bucket, R2Object};
use foundation_db::D1Executor;
use foundation_nativeapis::wasm::vfs::r2_blocks::R2BlockStore;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::{Context, Env, Request, Response, Result, event};

/// Read the environment object as a JavaScript value.
fn env_value(env: Env) -> JsValue {
    env.into()
}

fn bytes(value: &[u8]) -> js_sys::Uint8Array {
    let data = js_sys::Uint8Array::new_with_length(u32::try_from(value.len()).unwrap_or(u32::MAX));
    data.copy_from(value);
    data
}

/// Put one object with no condition.
async fn put(bucket: &R2Bucket, key: &str, value: &str) -> std::result::Result<(), String> {
    let body = bytes(value.as_bytes());
    JsFuture::from(bucket.put(key, &body.into()))
        .await
        .map_err(|error| format!("R2 put {key} failed: {error:?}"))?;
    Ok(())
}

/// Put one object only when its current etag matches `etag`.
///
/// Returns true when the write happened. A failed condition resolves to `null`.
async fn put_if_etag(
    bucket: &R2Bucket,
    key: &str,
    value: &str,
    etag: &str,
) -> std::result::Result<bool, String> {
    let body = bytes(value.as_bytes());
    let options = js_sys::JSON::parse(&format!(
        "{{\"onlyIf\":{{\"etagMatches\":\"{etag}\"}}}}"
    ))
    .map_err(|error| format!("R2 conditional options failed: {error:?}"))?;
    let result = JsFuture::from(bucket.put_with_options(key, &body.into(), &options))
        .await
        .map_err(|error| format!("R2 conditional put {key} failed: {error:?}"))?;
    Ok(!(result.is_null() || result.is_undefined()))
}

/// Read one object as text.
async fn get(bucket: &R2Bucket, key: &str) -> std::result::Result<Option<String>, String> {
    let result = JsFuture::from(bucket.get(key))
        .await
        .map_err(|error| format!("R2 get {key} failed: {error:?}"))?;
    if result.is_null() || result.is_undefined() {
        return Ok(None);
    }
    let object: R2Object = result.unchecked_into();
    let buffer = JsFuture::from(object.array_buffer())
        .await
        .map_err(|error| format!("R2 arrayBuffer {key} failed: {error:?}"))?;
    let buffer = buffer
        .dyn_into::<js_sys::ArrayBuffer>()
        .map_err(|_| String::from("R2 arrayBuffer returned a value that is not an ArrayBuffer"))?;
    let text = String::from_utf8(js_sys::Uint8Array::new(&buffer).to_vec())
        .map_err(|error| format!("R2 object {key} is not text: {error}"))?;
    Ok(Some(text))
}

/// The bare hexadecimal etag of one object.
async fn head_etag(bucket: &R2Bucket, key: &str) -> std::result::Result<String, String> {
    let result = JsFuture::from(bucket.head(key))
        .await
        .map_err(|error| format!("R2 head {key} failed: {error:?}"))?;
    if result.is_null() || result.is_undefined() {
        return Err(format!("R2 object {key} is missing"));
    }
    let object: R2Object = result.unchecked_into();
    Ok(object.etag())
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

/// Run every block scenario and return the observations.
async fn run(env: Env) -> std::result::Result<Value, String> {
    let env_js = env_value(env);
    let bucket = R2Bucket::from_env(&env_js, "BUCKET").map_err(|error| format!("{error:?}"))?;

    // The conditional-write semantics, with the bare R2 etag (not the quoted
    // HTTP form). A matching etag writes; a stale etag does not.
    put(&bucket, "conditional", "first").await?;
    let etag = head_etag(&bucket, "conditional").await?;
    if !put_if_etag(&bucket, "conditional", "second", &etag).await? {
        return Err("a conditional put with the current etag did not write".to_string());
    }
    if put_if_etag(&bucket, "conditional", "third", "0000000000000000").await? {
        return Err("a conditional put with a stale etag wrote".to_string());
    }
    let after = get(&bucket, "conditional")
        .await?
        .ok_or_else(|| "the conditional object is missing".to_string())?;
    if after != "second" {
        return Err(format!("the conditional put left the value {after}"));
    }

    // The block store: a put, an idempotent retry, and an exact range read.
    let store = R2BlockStore::new(bucket, "spec71/feature11/r2-blocks");
    let block = b"0123456789".to_vec();
    store
        .put("retry", block.clone())
        .await
        .map_err(|error| error.to_string())?;
    store
        .put("retry", block)
        .await
        .map_err(|error| error.to_string())?;
    let range = store
        .get_range("retry", 3, 4)
        .await
        .map_err(|error| error.to_string())?;
    if range != b"3456" {
        return Err(format!(
            "the R2 range read returned {:?}, not \"3456\"",
            String::from_utf8_lossy(&range)
        ));
    }

    // The SQL VFS on local D1, with R2 as the block tier.
    let executor = D1Executor::from_env(&env_js, "DB").map_err(|error| format!("{error:?}"))?;
    for statement in [
        "DROP TABLE IF EXISTS pending_data",
        "DROP TABLE IF EXISTS stream_meta",
    ] {
        executor
            .execute(SqlStatement::plain(statement))
            .await
            .map_err(|error| error.to_string())?;
    }
    let ctx = Ctx::host();
    let config = SqlVfsConfig::default()
        .with_batch_age(Duration::ZERO)
        .with_max_block_bytes(1024)
        .with_vacuum_threshold(1)
        .with_block_prefix("spec71/feature11/sql-blocks");
    let vfs = SqlVfs::new(Arc::new(executor), Arc::new(store), &ctx, config);
    let name = StreamName::parse("r2/blocks").map_err(|error| error.to_string())?;
    vfs.create(&name, meta())
        .await
        .map_err(|error| error.to_string())?;
    vfs.append(&name, b"hello ")
        .await
        .map_err(|error| error.to_string())?;
    vfs.append(&name, b"world")
        .await
        .map_err(|error| error.to_string())?;
    let report = vfs.flush().await.map_err(|error| error.to_string())?;
    if report.blocks_put != 1 {
        return Err(format!(
            "the SQL VFS put {} blocks, not one",
            report.blocks_put
        ));
    }
    vfs.vacuum().await.map_err(|error| error.to_string())?;
    let sql_read = vfs
        .read_at(&name, &VfsOffset(6), 5)
        .await
        .map_err(|error| error.to_string())?;
    if sql_read != b"world" {
        return Err(format!(
            "the vacuumed SQL read returned {:?}",
            String::from_utf8_lossy(&sql_read)
        ));
    }

    Ok(json!({
        "ok": true,
        "conditional_put": true,
        "retry_same_bytes": true,
        "range": String::from_utf8_lossy(&range),
        "sql_read": String::from_utf8_lossy(&sql_read),
    }))
}

/// Route `GET /r2-blocks` to the block scenarios.
#[event(fetch)]
async fn main(request: Request, env: Env, _ctx: Context) -> Result<Response> {
    if request.path() != "/r2-blocks" {
        return Response::ok("GET /r2-blocks runs the R2 block store scenarios");
    }
    match run(env).await {
        Ok(value) => Response::from_json(&value),
        Err(error) => Ok(Response::from_json(&json!({
            "ok": false,
            "error": error,
        }))?
        .with_status(500)),
    }
}
