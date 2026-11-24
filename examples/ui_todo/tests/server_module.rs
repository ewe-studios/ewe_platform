//! The Todo server packages its browser assets before startup.
use std::{fs, path::Path};

#[test]
fn only_the_application_bundle_enters_the_public_javascript_tree() {
    fn scripts(directory: &Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scripts(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "js") {
                found.push(path);
            }
        }
    }
    let root = Path::new(env!("EWE_STAGED_RESOURCES"));
    let mut found = Vec::new();
    scripts(root, &mut found);
    assert_eq!(found, [root.join("bundle.js")]);
    for file in [
        "module.wasm",
        "rpc/foundation_ui_connectrpc_bg.wasm",
        "resources-wasm/foundation_resources_bg.wasm",
    ] {
        use std::io::Read;
        let mut magic = [0u8; 8];
        fs::File::open(root.join(file))
            .unwrap()
            .read_exact(&mut magic)
            .unwrap();
        assert_eq!(&magic, b"\0asm\x01\0\0\0");
    }
}
