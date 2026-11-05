//! `Directorate` tests moved out of `src/package.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public.

use foundation_macros::EmbedDirectoryAs;
use foundation_packager::{Directorate, PackageDirectorate};

#[derive(EmbedDirectoryAs, Default)]
#[source = "$ARTEFACTS_DIR/temples/templates/test_directory/"]
struct Directory;

#[test]
fn validate_can_create_directorate_generator_safely() {
    let generator = Directorate::<Directory>::default();
    assert!(generator.get_file("README.md").is_some());
}

#[test]
fn validate_can_read_top_directories() {
    let generator = Directorate::<Directory>::default();
    let _files: Vec<String> = generator.files();
    let directories: Vec<String> = generator.root_directories();
    assert_eq!(directories, vec! {"docs", "schema"});
}

#[test]
fn validate_can_read_only_files_for_top_directory() {
    let generator = Directorate::<Directory>::default();
    let files: Option<Vec<String>> = generator.files_for("schema");
    assert!(files.is_some());
    assert_eq!(
        files.unwrap(),
        vec! {"schema/partials/partial_1.sql", "schema/schema.sql"}
    );
}

#[test]
fn validate_can_read_all_directories() {
    let generator = Directorate::<Directory>::default();
    let mut files: Vec<String> = generator.files();
    files.sort();
    let mut expected = vec![
        "docs/runner.sh",
        "schema/partials/partial_1.sql",
        "schema/schema.sql",
        "README.md",
        "elem.js",
    ];
    expected.sort_unstable();
    assert_eq!(files, expected);
}
