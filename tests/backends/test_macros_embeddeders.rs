use foundation_macros::EmbedDirectoryAs;
use foundation_macros::EmbedFileAs;

#[derive(EmbedFileAs, Default)]
#[source = "../../assets/hello/world.js"]
#[with_utf16]
pub struct JSHostRuntime;

#[derive(EmbedDirectoryAs, Default)]
#[source = "../../assets/hello"]
#[with_utf16]
pub struct JSHostRuntimeAssets;

#[test]
fn can_read_data_from_js_host_runtime() {
    let runtime = JSHostRuntime::default();
    assert_eq!(runtime.read_utf8(), Some(vec![]));
}
