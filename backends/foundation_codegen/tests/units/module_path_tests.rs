//! `ModulePathResolver` tests moved out of `src/module_path.rs` (spec 70
//! feature 59, rule 7). Every item the test uses is public.

use std::fs;
use std::path::Path;

use foundation_codegen::{CrateMetadata, ModulePathResolver};

fn setup_test_crate(tmp_dir: &Path, crate_name: &str) -> CrateMetadata {
    let src_dir = tmp_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let cargo_toml = tmp_dir.join("Cargo.toml");
    fs::write(
        &cargo_toml,
        format!(
            r#"[package]
name = "{}"
version = "0.1.0"
"#,
            crate_name
        ),
    )
    .unwrap();

    CrateMetadata {
        name: crate_name.to_string(),
        version: "0.1.0".to_string(),
        root_dir: tmp_dir.to_path_buf(),
        cargo_toml_path: cargo_toml,
        src_dir: src_dir.clone(),
        entry_point: src_dir.join("lib.rs"),
        is_lib: true,
    }
}

#[test]
fn resolves_lib_rs_to_crate_root() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let lib_rs = tmp.path().join("src").join("lib.rs");
    let result = resolver.resolve_file_module_path(&lib_rs).unwrap();

    assert_eq!(result, "my_crate");
}

#[test]
fn resolves_main_rs_to_crate_root() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_bin");
    let resolver = ModulePathResolver::new(metadata);

    let main_rs = tmp.path().join("src").join("main.rs");
    let result = resolver.resolve_file_module_path(&main_rs).unwrap();

    assert_eq!(result, "my_bin");
}

#[test]
fn resolves_flat_module_file() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let handlers_rs = tmp.path().join("src").join("handlers.rs");
    let result = resolver.resolve_file_module_path(&handlers_rs).unwrap();

    assert_eq!(result, "my_crate::handlers");
}

#[test]
fn resolves_nested_module_file() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let auth_rs = tmp.path().join("src").join("handlers").join("auth.rs");
    fs::create_dir_all(auth_rs.parent().unwrap()).unwrap();
    fs::write(&auth_rs, "").unwrap();

    let result = resolver.resolve_file_module_path(&auth_rs).unwrap();

    assert_eq!(result, "my_crate::handlers::auth");
}

#[test]
fn resolves_mod_rs_to_parent_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let mod_rs = tmp.path().join("src").join("handlers").join("mod.rs");
    fs::create_dir_all(mod_rs.parent().unwrap()).unwrap();
    fs::write(&mod_rs, "").unwrap();

    let result = resolver.resolve_file_module_path(&mod_rs).unwrap();

    assert_eq!(result, "my_crate::handlers");
}

#[test]
fn resolves_deeply_nested_path() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let users_rs = tmp
        .path()
        .join("src")
        .join("api")
        .join("v2")
        .join("handlers")
        .join("users.rs");
    fs::create_dir_all(users_rs.parent().unwrap()).unwrap();
    fs::write(&users_rs, "").unwrap();

    let result = resolver.resolve_file_module_path(&users_rs).unwrap();

    assert_eq!(result, "my_crate::api::v2::handlers::users");
}

#[test]
fn resolves_item_with_inline_modules() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let handlers_rs = tmp.path().join("src").join("handlers.rs");
    fs::write(&handlers_rs, "").unwrap();

    let inline_modules = vec!["auth".to_string(), "middleware".to_string()];
    let result = resolver
        .resolve_item_module_path(&handlers_rs, &inline_modules)
        .unwrap();

    assert_eq!(result, "my_crate::handlers::auth::middleware");
}

#[test]
fn resolves_qualified_path() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let handlers_rs = tmp.path().join("src").join("handlers.rs");
    fs::write(&handlers_rs, "").unwrap();

    let inline_modules = vec!["auth".to_string()];
    let result = resolver
        .resolve_qualified_path(&handlers_rs, &inline_modules, "AuthHandler")
        .unwrap();

    assert_eq!(result, "my_crate::handlers::auth::AuthHandler");
}

#[test]
fn resolves_qualified_path_without_inline_modules() {
    let tmp = tempfile::tempdir().unwrap();
    let metadata = setup_test_crate(tmp.path(), "my_crate");
    let resolver = ModulePathResolver::new(metadata);

    let handlers_rs = tmp.path().join("src").join("handlers.rs");
    fs::write(&handlers_rs, "").unwrap();

    let result = resolver
        .resolve_qualified_path(&handlers_rs, &[], "AuthHandler")
        .unwrap();

    assert_eq!(result, "my_crate::handlers::AuthHandler");
}
