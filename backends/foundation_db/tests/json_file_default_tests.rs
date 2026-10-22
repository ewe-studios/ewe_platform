//! `JsonFileStorage::default()` writes to `.ewe/storage.json` under the working
//! directory. This test changes the working directory, so it lives in its own
//! test binary where no other test can race it.

use foundation_core::valtron::valtron_test;
use foundation_db::traits::KeyValueStore;
use foundation_db::JsonFileStorage;
use tempfile::TempDir;

#[valtron_test]
async fn default_json_file_storage_persists_under_dot_ewe() {
    let dir = TempDir::new().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let storage = JsonFileStorage::default();
    storage.set_async("k", "v").await.unwrap();
    assert!(
        dir.path().join(JsonFileStorage::DEFAULT_PATH).is_file(),
        "default store writes to {}",
        JsonFileStorage::DEFAULT_PATH
    );

    // A second default store reads back what the first one wrote.
    let reopened = JsonFileStorage::default();
    let value: Option<String> = reopened.get_async("k").await.unwrap();
    assert_eq!(value.as_deref(), Some("v"));
}
