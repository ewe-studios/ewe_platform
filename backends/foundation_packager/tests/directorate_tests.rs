//! `Directorate` tests moved out of `src/package.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public.

use foundation_macros::EmbedDirectoryAs;
use foundation_nostd::embeddable::{
    DataCompression, DirectoryData, EmbeddableDirectory, FileInfo, HasCompression,
    StaticDirectoryData,
};
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

#[derive(Default)]
struct EmbeddedFixture<const KIND: usize>;

impl<const KIND: usize> HasCompression for EmbeddedFixture<KIND> {
    fn compression(&self) -> DataCompression {
        DataCompression::NONE
    }
}

const fn embedded_entry(
    source: &'static str,
    parent_source: &'static str,
    directory: bool,
) -> FileInfo {
    FileInfo::create(
        None,
        r"C:\checkout\templates",
        "",
        source,
        parent_source,
        r"C:\checkout\templates",
        "",
        "",
        None,
        None,
        directory,
    )
}

const WINDOWS_TEMPLATE: FileInfo =
    embedded_entry(r"mobile\main.txt", r"templates\mobile\main.txt", false);
const WINDOWS_PARTIAL: FileInfo = embedded_entry(
    r"mobile\partials\_title.txt",
    r"templates\mobile\partials\_title.txt",
    false,
);

impl<const KIND: usize> DirectoryData for EmbeddedFixture<KIND> {
    const FILES_DATA: &'static [StaticDirectoryData] = &[
        (
            0,
            r"mobile\main.txt",
            r"templates\mobile\main.txt",
            b"First\r\n{% include 'mobile/partials/_title.txt' %}",
            None,
        ),
        (
            1,
            r"mobile\partials\_title.txt",
            r"templates\mobile\partials\_title.txt",
            b"Hello {{ name }}!",
            None,
        ),
    ];
}

impl<const KIND: usize> EmbeddableDirectory for EmbeddedFixture<KIND> {
    const FILES_METADATA: &'static [FileInfo] = match KIND {
        0 => &[
            embedded_entry("", "", true),
            embedded_entry("mobile", "mobile", true),
            embedded_entry(r"mobile\partials", r"mobile\partials", true),
            WINDOWS_TEMPLATE,
            WINDOWS_PARTIAL,
        ],
        1 => &[
            WINDOWS_TEMPLATE,
            embedded_entry("mobile/main.txt", "templates/mobile/main.txt", false),
        ],
        2 => &[
            WINDOWS_TEMPLATE,
            embedded_entry(
                r"mobile\..\outside.txt",
                r"templates\mobile\..\outside.txt",
                false,
            ),
        ],
        3 => &[
            WINDOWS_TEMPLATE,
            embedded_entry(r"C:\outside.txt", r"C:\outside.txt", false),
        ],
        _ => &[
            WINDOWS_TEMPLATE,
            embedded_entry(
                r"\\host\share\outside.txt",
                r"\\host\share\outside.txt",
                false,
            ),
        ],
    };
}

#[test]
fn reads_windows_paths_and_renders_crlf_templates_with_slash_names() {
    let directorate = Directorate::<EmbeddedFixture<0>>::default();
    assert_eq!(
        directorate.files_for("mobile").unwrap(),
        ["mobile/main.txt", "mobile/partials/_title.txt"],
    );
    assert_eq!(directorate.root_directories(), ["mobile"]);
    assert_eq!(
        directorate.list_all(),
        [
            "",
            "mobile",
            "mobile/partials",
            "mobile/main.txt",
            "mobile/partials/_title.txt"
        ],
    );
    assert_eq!(
        directorate.files_for(r"mobile\partials").unwrap(),
        ["mobile/partials/_title.txt"],
    );
    let expected = b"First\r\n{% include 'mobile/partials/_title.txt' %}";
    assert_eq!(directorate.get_file("mobile/main.txt").unwrap(), expected);
    assert_eq!(
        directorate.get_file(r"templates\mobile\main.txt").unwrap(),
        expected
    );
    let templates = directorate.jinja_for("mobile").unwrap();
    let output = templates
        .get_template("mobile/main.txt")
        .unwrap()
        .render(minijinja::context! { name => "there" })
        .unwrap();
    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        ["First", "Hello there!"]
    );
}

fn assert_rejects_embedded_names<const KIND: usize>() {
    let directorate = Directorate::<EmbeddedFixture<KIND>>::default();
    assert!(directorate.files_for("mobile").is_none());
    assert!(directorate.jinja_for("mobile").is_none());
    assert!(directorate.get_file("mobile/main.txt").is_none());
}

#[test]
fn rejects_duplicate_traversal_and_absolute_embedded_names() {
    assert_rejects_embedded_names::<1>();
    assert_rejects_embedded_names::<2>();
    assert_rejects_embedded_names::<3>();
    assert_rejects_embedded_names::<4>();
}
