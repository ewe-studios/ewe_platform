//! The glue patch that drops libtest's `main` from a wasm-bindgen test binary
//! (spec 88 T9, #586).
//!
//! WHY: the wasm-bindgen CLI writes a start call that runs libtest's harness.
//! The harness executes every plain `#[test]` case, and those cases abort a
//! wasm module (they spawn threads), so no `#[wasm_bindgen_test]` case ever
//! runs. The testbed replaces the call with the externref table setup alone.
//! WHAT: writes a glue-shaped file, patches it, reads it back.

use foundation_testbed::wasm::wasm::patch_bindgen_glue;

/// A glue file with the shape the CLI emits: the start call, the imports
/// factory it needs, and an unrelated line that must survive.
const GLUE: &str = r#"let wasm;
function __wbg_get_imports() {
    return { "./pkg_bg.js": { __wbindgen_init_externref_table: function() {} } };
}
function __wbg_finalize_init(instance) {
    wasm = instance.exports;
    wasm.__wbindgen_start();
    return wasm;
}
"#;

fn write_glue(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("pkg.js");
    std::fs::write(&path, GLUE).expect("write glue");
    path
}

#[test]
fn patch_replaces_the_start_call_with_the_table_init() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_glue(&dir);

    patch_bindgen_glue(&path, false).expect("patch the glue");
    let patched = std::fs::read_to_string(&path).expect("read patched glue");

    assert!(
        !patched.contains("wasm.__wbindgen_start();"),
        "the libtest start call must be gone:\n{patched}"
    );
    assert!(
        patched.contains("__wbindgen_init_externref_table"),
        "the externref table setup must stay:\n{patched}"
    );
    assert!(
        patched.contains("function __wbg_finalize_init"),
        "the rest of the glue must survive:\n{patched}"
    );
    assert!(
        !patched.contains("export { wasm };"),
        "a runner that uses `init()` needs no export:\n{patched}"
    );
}

#[test]
fn patch_exports_the_instance_when_the_runner_needs_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_glue(&dir);

    patch_bindgen_glue(&path, true).expect("patch the glue");
    let patched = std::fs::read_to_string(&path).expect("read patched glue");

    assert!(
        patched.trim_end().ends_with("export { wasm };"),
        "the runner must be able to call `wasm[...]`:\n{patched}"
    );
}

#[test]
fn a_glue_without_the_start_call_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pkg.js");
    std::fs::write(&path, "let wasm;\n").expect("write glue");

    let error = patch_bindgen_glue(&path, false).expect_err("a changed glue must fail");
    let message = error.to_string();

    assert!(
        message.contains("wasm.__wbindgen_start();"),
        "the error names the missing call: {message}"
    );
    assert!(
        message.contains("pkg.js"),
        "the error names the glue file: {message}"
    );
}
