//! The Todo server packages its browser assets before startup.
use std::{fs, path::Path};

#[test]
fn server_construction_rejects_duplicate_task_identifiers() {
    let first = ui_todo::Todo {
        id: "00000000-0000-4000-8000-000000000042".parse().unwrap(),
        text: "First task".into(),
        done: false,
    };
    let second = ui_todo::Todo {
        text: "Second task".into(),
        ..first.clone()
    };
    let Err(error) = ui_todo::server::app_with_todos(vec![first, second]) else {
        panic!("The server accepted two tasks with the same identifier");
    };
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::InvalidData)
    );
}

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
        "mathalley/ui_todo_bg.wasm",
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

#[test]
fn staged_styles_and_version_match_the_application() {
    let root = Path::new(env!("EWE_STAGED_RESOURCES"));
    assert_eq!(
        fs::read_to_string(root.join("theme.css")).unwrap(),
        ui_todo::todo_theme().to_css()
    );
    assert_eq!(
        fs::read_to_string(root.join("app/version.txt")).unwrap(),
        env!("CARGO_PKG_VERSION")
    );
}
