//! Check the confirmed Todo records through the application and the database.
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_db::{KeyValueStore, LibsqlStore, QueryStore};
use foundation_ui_runtime::action::{invoke_action, Origin};
use foundation_ui_runtime::dispatch::{ReplayError, ReplayFeed};
use foundation_ui_wire::{
    writer::{ClientCaps, WriterConfig},
    ActionInputT, ActionRequestT, MountSurface,
};
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::path::Path;
use ui_todo::{server_registry, TodoStore};

#[path = "server_store/process.rs"]
mod process;

const FIRST: &str = "00000000-0000-4000-8000-000000000001";
const SECOND: &str = "00000000-0000-4000-8000-000000000002";

async fn open(path: &Path) -> (Ctx, LiveChannel) {
    open_with_writer(path, WriterConfig::default()).await
}

async fn open_with_writer(path: &Path, writer: WriterConfig) -> (Ctx, LiveChannel) {
    let host = Ctx::host();
    let store = TodoStore::open(&host, path).await.unwrap();
    let feed = LiveChannel::resume(1, store.position(), BacklogConfig::default(), writer);
    let app = ui_todo::server::app_with_store(store, feed.clone()).unwrap();
    (app.context().clone(), feed)
}

async fn action(ctx: &Ctx, name: &str, input: ActionInputT) -> Result<(), String> {
    let registry = server_registry().unwrap();
    let entry = registry
        .get(Origin::Server, name, MountSurface::Web)
        .unwrap();
    let mut req = ActionRequestT::default();
    req.input = Some(Box::new(input));
    invoke_action(ctx, entry, MountSurface::Web, None, req)
        .await
        .map(|_| ())
        .map_err(|err| err.to_string())
}

async fn add(ctx: &Ctx, id: &str, title: &str) -> Result<(), String> {
    let mut input = ActionInputT::default();
    input.json =
        Some(serde_json::to_vec(&serde_json::json!({ "id": id, "title": title })).unwrap());
    action(ctx, "add_todo", input).await
}

async fn change(ctx: &Ctx, name: &str, id: &str) -> Result<(), String> {
    let mut input = ActionInputT::default();
    input.value = Some(id.into());
    action(ctx, name, input).await
}

#[valtron_test]
async fn restart_preserves_input_receipts_and_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Todo ü.sqlite3");
    {
        let (ctx, feed) = open(&path).await;
        let disconnected = feed
            .listen(&ClientCaps::web_current(), Box::new(|_| false))
            .unwrap();
        add(&ctx, FIRST, " first ").await.unwrap();
        add(&ctx, SECOND, "second").await.unwrap();
        change(&ctx, "toggle_todo", FIRST).await.unwrap();
        assert_eq!(feed.last_sequence(), 3);
        assert_eq!(feed.conn_count(), 0);
        drop(disconnected);
        assert!(TodoStore::open(&ctx, &path).await.is_err());
    }
    {
        let (ctx, feed) = open(&path).await;
        let store = ctx.get::<TodoStore>().unwrap();
        assert_eq!(
            store
                .snapshot()
                .items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        let first = store.get(FIRST.parse().unwrap()).await.unwrap().unwrap();
        assert!(first.done);
        add(&ctx, FIRST, " first ").await.unwrap();
        assert!(add(&ctx, FIRST, "first").await.is_err());
        assert_eq!(feed.last_sequence(), 3);
        assert_eq!(feed.backlog_len(), 0);
        let caps = ClientCaps::web_current();
        assert!(matches!(feed.replay(&caps, 2), Err(ReplayError::Expired)));
        assert_eq!(
            feed.replay(&caps, 3).unwrap(),
            [] as [std::sync::Arc<[u8]>; 0]
        );
        change(&ctx, "confirm_delete", FIRST).await.unwrap();
    }
    let (ctx, feed) = open(&path).await;
    add(&ctx, FIRST, " first ").await.unwrap();
    change(&ctx, "confirm_delete", FIRST).await.unwrap();
    let store = ctx.get::<TodoStore>().unwrap();
    assert!(store.get(FIRST.parse().unwrap()).await.unwrap().is_none());
    assert_eq!(store.snapshot().items.len(), 1);
    assert_eq!(feed.last_sequence(), 4);
}

#[valtron_test]
async fn failed_write_preserves_live_and_durable_state_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todos.sqlite3");
    let (ctx, feed) = open(&path).await;
    add(&ctx, FIRST, "first").await.unwrap();
    let db = LibsqlStore::new_kv(path.to_str().unwrap(), None, ctx.clone())
        .await
        .unwrap();
    db.execute_batch("CREATE TRIGGER test_failure BEFORE INSERT ON kv_store BEGIN SELECT RAISE(ABORT, 'forced write failure'); END;").await.unwrap();
    let err = add(&ctx, SECOND, "second").await.unwrap_err();
    assert!(err.contains("forced write failure"), "{err}");
    assert_eq!(feed.last_sequence(), 1);
    assert_eq!(ctx.get::<TodoStore>().unwrap().snapshot().items.len(), 1);
    db.execute_batch("DROP TRIGGER test_failure").await.unwrap();
    let (retry, duplicate) =
        futures_lite::future::zip(add(&ctx, SECOND, "second"), add(&ctx, SECOND, "second")).await;
    retry.unwrap();
    duplicate.unwrap();
    assert_eq!(feed.last_sequence(), 2);
    drop(db);
    drop(ctx);
    let (ctx, restored) = open(&path).await;
    assert_eq!(restored.last_sequence(), 2);
    assert_eq!(ctx.get::<TodoStore>().unwrap().snapshot().items.len(), 2);
}

#[valtron_test]
async fn malformed_store_preserves_the_source_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.sqlite3");
    let contents = b"malformed Todo database";
    std::fs::write(&path, contents).unwrap();
    assert!(TodoStore::open(&Ctx::host(), &path).await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), contents);
}

#[valtron_test]
async fn disconnected_caller_and_concurrent_writer_both_commit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todos.sqlite3");
    let (ctx, feed) = open(&path).await;
    // Hold the first publication so the caller must disconnect before it can get a reply.
    let (release, first_frame) = std::sync::mpsc::channel();
    let mut first_frame = Some(first_frame);
    let reader = feed
        .listen(
            &ClientCaps::web_current(),
            Box::new(move |_| {
                if let Some(frame) = first_frame.take() {
                    return frame
                        .recv_timeout(std::time::Duration::from_secs(10))
                        .is_ok();
                }
                true
            }),
        )
        .unwrap();
    assert!(futures_lite::future::poll_once(add(&ctx, FIRST, "first"))
        .await
        .is_none());
    release.send(()).unwrap();
    // A second request changes the first row, which requires the queued create to finish first.
    change(&ctx, "toggle_todo", FIRST).await.unwrap();
    add(&ctx, SECOND, "second").await.unwrap();
    assert_eq!(feed.last_sequence(), 3);
    let (first, second) = futures_lite::future::zip(
        change(&ctx, "toggle_todo", FIRST),
        change(&ctx, "toggle_todo", SECOND),
    )
    .await;
    first.unwrap();
    second.unwrap();
    assert_eq!(feed.last_sequence(), 5);
    drop(reader);
    drop(ctx);
    let (ctx, feed) = open(&path).await;
    assert_eq!(feed.last_sequence(), 5);
    let rows = ctx.get::<TodoStore>().unwrap().snapshot().items;
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].done);
    assert!(rows[1].done);
}

#[valtron_test]
async fn failed_frame_encoding_preserves_both_store_and_stream() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todos.sqlite3");
    let writer = WriterConfig::default()
        .limits(foundation_ui_wire::WireLimits::default().max_frame_bytes(1));
    let (ctx, feed) = open_with_writer(&path, writer).await;
    assert!(add(&ctx, FIRST, "first").await.is_err());
    assert_eq!(feed.last_sequence(), 0);
    assert_eq!(
        ctx.get::<TodoStore>().unwrap().snapshot().items,
        [] as [ui_todo::Todo; 0]
    );
    drop(ctx);
    let (ctx, restored) = open(&path).await;
    assert_eq!(restored.last_sequence(), 0);
    add(&ctx, FIRST, "first").await.unwrap();
    assert_eq!(restored.last_sequence(), 1);
}

#[valtron_test]
async fn failed_startup_releases_database_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todos.sqlite3");
    let host = Ctx::host();
    let store = TodoStore::open(&host, &path).await.unwrap();
    let wrong = LiveChannel::new(5, BacklogConfig::default(), WriterConfig::default());
    assert!(ui_todo::server::app_with_store(store, wrong).is_err());
    let (ctx, feed) = open(&path).await;
    add(&ctx, FIRST, "first").await.unwrap();
    assert_eq!(feed.last_sequence(), 1);
}

#[valtron_test]
async fn wrong_record_type_fails_startup_and_keeps_the_original_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("todos.sqlite3");
    let host = Ctx::host();
    let kv = LibsqlStore::new_kv(path.to_str().unwrap(), None, host.clone())
        .await
        .unwrap();
    kv.init_kv().await.unwrap();
    let key = format!("todo.record/{FIRST}");
    kv.set(&key, "unexpected record type").await.unwrap();
    assert!(TodoStore::open(&host, &path).await.is_err());
    assert_eq!(
        kv.get::<String>(&key).await.unwrap().as_deref(),
        Some("unexpected record type")
    );
}
