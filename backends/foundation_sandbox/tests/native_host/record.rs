//! The record store on disk (feature 10, section 6.6).

use std::sync::Arc;

use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::record::FileRecordStore;
use foundation_sandbox::shared::control::{RecordStore, SandboxState};
use foundation_sandbox::shared::driver::StoredRecord;
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};

/// A record for one identifier.
fn record(id: &str, created_unix_ms: u64) -> StoredRecord {
    StoredRecord {
        id: id.parse().expect("a valid sandbox id"),
        driver: DriverId::Docker,
        spec: SandboxSpec::new(RootFs::ContainerImage("alpine:3.22".to_string())),
        state: SandboxState::Ready,
        created_unix_ms,
        expires_at_unix_ms: None,
        endpoint: None,
        manifest: None,
    }
}

/// A store under a fresh directory.
fn store(tag: &str) -> (FileRecordStore, std::path::PathBuf) {
    let directory = super::temp_dir(tag);
    let store = FileRecordStore::new(Arc::new(NativeFs::new()), directory.clone())
        .expect("the record store");
    (store, directory)
}

#[test]
fn file_store_roundtrip() {
    let (store, directory) = store("record-roundtrip");
    crate::pool::block_on(store.put(record("000000000001", 1))).expect("the first put");
    crate::pool::block_on(store.put(record("000000000002", 2))).expect("the second put");
    let mut records = crate::pool::block_on(store.list()).expect("the list");
    records.sort_by_key(|record| record.created_unix_ms);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].id.as_str(), "000000000001");
    assert_eq!(records[1].id.as_str(), "000000000002");

    crate::pool::block_on(store.remove(&records[0].id)).expect("the remove");
    let records = crate::pool::block_on(store.list()).expect("the list after remove");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id.as_str(), "000000000002");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn file_store_atomic() {
    let (store, directory) = store("record-atomic");
    let first = record("000000000001", 1);
    crate::pool::block_on(store.put(first)).expect("the first put");

    // A crash between the temporary write and the rename leaves the old
    // record: `list` reads only the `.json` files.
    let second = record("000000000001", 2);
    let temp = store.stage(&second).expect("the staged write");
    assert!(temp.exists(), "the temporary file exists");
    let records = crate::pool::block_on(store.list()).expect("the list");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].created_unix_ms, 1,
        "the old record stays until the rename"
    );

    store
        .commit(&second.id, &temp)
        .expect("the commit renames the file");
    let records = crate::pool::block_on(store.list()).expect("the list after commit");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].created_unix_ms, 2);
    let _ = std::fs::remove_dir_all(&directory);
}
