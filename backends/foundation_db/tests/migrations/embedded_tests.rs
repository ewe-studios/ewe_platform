//! `embed_migrations!` gives the same plan as `from_dir`.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use foundation_core::valtron::valtron_test;
use foundation_db::{embed_migrations, MigrationRunner, MigrationSet};
use foundation_vfs::EmptyFs;

use crate::common::{memory_fs, mkdir, write};

static EMBEDDED: MigrationSet = embed_migrations!("tests/migrations/data/embedded_group");

#[valtron_test]
async fn embedded_and_directory_give_the_same_plan() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/migrations/data/embedded_group");
    let fs = memory_fs();
    mkdir(&fs, "embedded_group").await;
    for entry in std::fs::read_dir(&fixture).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let text = std::fs::read_to_string(entry.path()).unwrap();
        write(&fs, &format!("embedded_group/{name}"), &text).await;
    }

    let embedded = MigrationRunner::from_static(&EMBEDDED)
        .plan(&EmptyFs::new())
        .await
        .unwrap();
    let directory = MigrationRunner::from_dir("embedded_group")
        .plan(&fs)
        .await
        .unwrap();

    assert_eq!(embedded.len(), 2);
    assert_eq!(embedded, directory);
    assert_eq!(embedded[0].number, 1);
    assert_eq!(embedded[0].name, "create_widgets");
    assert_eq!(embedded[1].number, 2);
    assert_eq!(embedded[1].name, "add_widget_name");
}
