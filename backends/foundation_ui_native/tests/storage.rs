use foundation_core::valtron::valtron_test;
use foundation_ui_native::storage::NativeRecordFile;

#[valtron_test]
async fn the_legacy_reader_preserves_invalid_and_oversized_files() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("old.json");
    let records =
        NativeRecordFile::new(dir.path().join("new.sqlite3"), legacy.clone()).with_legacy_limit(8);
    for contents in [b"invalid".as_slice(), b"123456789"] {
        std::fs::write(&legacy, contents).unwrap();
        assert!(records.legacy_record::<serde_json::Value>().await.is_err());
        assert_eq!(std::fs::read(&legacy).unwrap(), contents);
    }
    std::fs::write(&legacy, b"12").unwrap();
    assert_eq!(records.legacy_record::<u32>().await.unwrap(), Some(12));
}

#[valtron_test]
async fn a_database_does_not_keep_its_host_context_alive() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = foundation_context::Ctx::host();
    ctx.store(NativeRecordFile::new(
        dir.path().join("records.sqlite3"),
        dir.path().join("old.json"),
    ));
    let records = ctx.get::<NativeRecordFile>().unwrap();
    let weak = std::sync::Arc::downgrade(&records);
    let _database = records.database(&ctx).await.unwrap();
    drop(records);
    drop(ctx);
    assert!(weak.upgrade().is_none());
}
