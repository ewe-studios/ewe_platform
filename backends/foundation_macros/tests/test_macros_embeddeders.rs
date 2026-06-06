use foundation_macros::{EmbedDirectoryAs, EmbedFileAs};
use foundation_nostd::embeddable::FileData;

#[derive(EmbedFileAs, Default)]
#[source = "hello/world.js"]
#[with_utf16]
pub struct JSHostRuntime;

#[derive(EmbedDirectoryAs, Default)]
#[source = "hello"]
#[with_utf16]
pub struct JSHostRuntimeAssets;

#[test]
fn can_read_data_from_js_host_runtime() {
    let runtime = JSHostRuntime::default();
    assert_eq!(runtime.read_utf8(), Some(b"world;\n".to_vec()));
}
