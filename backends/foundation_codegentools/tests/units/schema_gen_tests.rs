use std::path::Path;

use foundation_codegentools::schema_gen::SchemaGenerator;

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema_crate")
}

#[test]
fn scan_fixture_finds_schema_structs() {
    let gen = SchemaGenerator::new(&fixture_dir()).expect("should scan fixture crate");

    assert_eq!(gen.structs().len(), 3, "should find 3 structs with schema derives");

    let names: Vec<&str> = gen.structs().iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"VfsMetadata"));
    assert!(names.contains(&"DirEntry"));
    assert!(names.contains(&"ApiResponse"));
}

#[test]
fn scan_detects_correct_derive_flags() {
    let gen = SchemaGenerator::new(&fixture_dir()).expect("should scan fixture crate");

    let vfs = gen.structs().iter().find(|s| s.name == "VfsMetadata").unwrap();
    assert!(vfs.has_arrow_schema, "VfsMetadata has ArrowSchema");
    assert!(vfs.has_json_schema, "VfsMetadata has JsonSchema");

    let dir = gen.structs().iter().find(|s| s.name == "DirEntry").unwrap();
    assert!(dir.has_arrow_schema, "DirEntry has ArrowSchema");
    assert!(!dir.has_json_schema, "DirEntry does not have JsonSchema");

    let api = gen.structs().iter().find(|s| s.name == "ApiResponse").unwrap();
    assert!(!api.has_arrow_schema, "ApiResponse does not have ArrowSchema");
    assert!(api.has_json_schema, "ApiResponse has JsonSchema");
}

#[test]
fn scan_extracts_fields_correctly() {
    let gen = SchemaGenerator::new(&fixture_dir()).expect("should scan fixture crate");

    let vfs = gen.structs().iter().find(|s| s.name == "VfsMetadata").unwrap();
    assert_eq!(vfs.fields.len(), 4);
    assert_eq!(vfs.fields[0].name, "size");
    assert_eq!(vfs.fields[0].rust_type, "u64");
    assert!(!vfs.fields[0].nullable);

    assert_eq!(vfs.fields[3].name, "checksum");
    assert_eq!(vfs.fields[3].rust_type, "Vec<u8>");
    assert!(vfs.fields[3].nullable);
}

#[test]
fn generate_json_schema_roundtrip() {
    let fixture = fixture_dir();
    let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
    copy_dir_recursive(&fixture, temp_dir.path());

    let gen = SchemaGenerator::new(temp_dir.path()).expect("should scan");
    let generated = gen.generate(true, false).expect("should generate json schemas");

    // VfsMetadata and ApiResponse have JsonSchema — 2 files
    assert_eq!(generated.len(), 2, "should generate 2 json schema files");

    let vfs_gen = generated.iter().find(|g| g.struct_name == "VfsMetadata").unwrap();
    let full_path = temp_dir.path().join(&vfs_gen.path);
    assert!(full_path.exists(), "VfsMetadata.json should exist");

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&full_path).unwrap()).unwrap();
    assert_eq!(content["title"], "VfsMetadata");
    assert_eq!(content["type"], "object");
    assert_eq!(content["properties"]["size"]["type"], "integer");
    assert_eq!(content["properties"]["size"]["format"], "uint64");
    assert_eq!(content["properties"]["name"]["type"], "string");

    let required = content["required"].as_array().unwrap();
    let req_names: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(req_names.contains(&"size"));
    assert!(req_names.contains(&"name"));
    assert!(req_names.contains(&"permissions"));
    assert!(!req_names.contains(&"checksum"), "optional field not in required");
}

#[test]
fn generate_arrow_schema_roundtrip() {
    let fixture = fixture_dir();
    let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
    copy_dir_recursive(&fixture, temp_dir.path());

    let gen = SchemaGenerator::new(temp_dir.path()).expect("should scan");
    let generated = gen.generate(false, true).expect("should generate arrow schemas");

    // VfsMetadata and DirEntry have ArrowSchema — 2 files
    assert_eq!(generated.len(), 2, "should generate 2 arrow schema files");

    let dir_gen = generated.iter().find(|g| g.struct_name == "DirEntry").unwrap();
    let full_path = temp_dir.path().join(&dir_gen.path);
    assert!(full_path.exists(), "DirEntry.json should exist");

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&full_path).unwrap()).unwrap();
    assert_eq!(content["struct_name"], "DirEntry");

    let fields = content["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0]["name"], "path");
    assert_eq!(fields[0]["data_type"], "Utf8");
    assert_eq!(fields[1]["name"], "is_dir");
    assert_eq!(fields[1]["data_type"], "Boolean");
    assert_eq!(fields[2]["name"], "size");
    assert_eq!(fields[2]["data_type"], "UInt64");
}

#[test]
fn generate_both_schemas() {
    let fixture = fixture_dir();
    let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
    copy_dir_recursive(&fixture, temp_dir.path());

    let gen = SchemaGenerator::new(temp_dir.path()).expect("should scan");
    let generated = gen.generate(true, true).expect("should generate all schemas");

    // VfsMetadata: both (2), DirEntry: arrow only (1), ApiResponse: json only (1) = 4
    assert_eq!(generated.len(), 4, "should generate 4 total schema files");

    let sdk_dir = temp_dir.path().join("sdk");
    assert!(sdk_dir.join("jsonschema/VfsMetadata.json").exists());
    assert!(sdk_dir.join("jsonschema/ApiResponse.json").exists());
    assert!(sdk_dir.join("arrow_schema/VfsMetadata.json").exists());
    assert!(sdk_dir.join("arrow_schema/DirEntry.json").exists());

    assert!(!sdk_dir.join("jsonschema/DirEntry.json").exists());
    assert!(!sdk_dir.join("arrow_schema/ApiResponse.json").exists());
}

#[test]
fn no_schema_structs_returns_error() {
    let temp_dir = tempfile::TempDir::new().expect("should create temp dir");
    let src = temp_dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"empty\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();

    let gen = SchemaGenerator::new(temp_dir.path()).expect("should scan");
    let result = gen.generate(true, true);
    assert!(result.is_err(), "empty crate should error on generate");
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("should create dest dir");
    for entry in std::fs::read_dir(src).expect("should read source dir") {
        let entry = entry.expect("should read entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).expect("should copy file");
        }
    }
}
