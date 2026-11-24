//! Check both script formats against the same import graph.
use foundation_codegen::javascript::bundle;
use std::fs;

#[test]
fn resolves_transitive_inputs_and_preserves_legal_notices() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("main.js"), "import './one.js';").unwrap();
    fs::write(
        root.join("one.js"),
        "import { value } from './two.js'; globalThis.answer = value;",
    )
    .unwrap();
    fs::write(
        root.join("two.js"),
        "// SPDX-License-Identifier: MIT\n/* Copyright Example Authors */\n/*! Fixture license */ export const value = 42;",
    )
    .unwrap();
    let output = bundle(&root.join("main.js")).unwrap();
    assert_eq!(output.inputs.len(), 3);
    for js in [&output.readable, &output.minified] {
        let js = std::str::from_utf8(js).unwrap();
        assert!(js.contains("/*! Fixture license */"));
        assert!(js.contains("/* Copyright Example Authors */"));
        assert!(js.contains("// SPDX-License-Identifier: MIT"));
        assert!(js.contains("answer"));
        assert!(!js.contains("export const"));
    }
    assert!(output.minified.len() < output.readable.len());
    fs::write(root.join("two.js"), "export const value = 43;").unwrap();
    let changed = bundle(&root.join("main.js")).unwrap();
    assert_ne!(changed.readable, output.readable);
    assert_ne!(changed.minified, output.minified);
}

#[test]
fn reports_missing_imports() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("main.js");
    fs::write(&path, "import './missing.js';").unwrap();
    let error = bundle(&path).err().expect("missing import fails");
    assert!(format!("{error:#}").contains("missing.js"));
}

#[test]
fn dynamic_imports_share_one_lazy_module_in_one_classic_script() {
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(
        root.join("main.js"),
        "globalThis.before = globalThis.count; globalThis.load = () => import('./lazy.js');",
    )
    .unwrap();
    fs::write(
        root.join("lazy.js"),
        "globalThis.count = (globalThis.count || 0) + 1; export const value = 42;",
    )
    .unwrap();
    for (name, compact) in [("readable.js", false), ("compact.js", true)] {
        let output =
            foundation_codegen::javascript::application(&root.join("main.js"), root, compact)
                .unwrap();
        assert_eq!(output.inputs.len(), 2);
        let js = output.bytes;
        let script = root.join(name);
        let mut js = String::from_utf8(js).unwrap();
        js.push_str("\nif (globalThis.count || globalThis.before) throw Error('early evaluation');\nPromise.all([load(),load()]).then(([a,b]) => { if(a !== b || a.value !== 42 || count !== 1) throw Error('incorrect module'); });");
        fs::write(&script, js).unwrap();
        let result = Command::new("node").arg(script).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn packaged_modules_keep_asset_urls_under_the_public_script_directory() {
    let mut scripts = Vec::new();
    for _ in 0..2 {
        let tree = tempfile::tempdir().unwrap();
        let root = tree.path();
        fs::create_dir(root.join("rpc")).unwrap();
        fs::write(
            root.join("main.js"),
            r#"globalThis.openTransport = () => import("/rpc/client.js");"#,
        )
        .unwrap();
        fs::write(
            root.join("rpc/client.js"),
            r#"export const wasm = new URL("client_bg.wasm", import.meta.url).href;"#,
        )
        .unwrap();
        let script =
            foundation_codegen::javascript::application(&root.join("main.js"), root, false)
                .unwrap()
                .bytes;
        let file = root.join("bundle.cjs");
        let mut source = String::from(
            "globalThis.document = {currentScript:{src:'https://example.test/demo/bundle.js'}};\n",
        );
        source.push_str(std::str::from_utf8(&script).unwrap());
        source.push_str("\nopenTransport().then(x=>{if(x.wasm!=='https://example.test/demo/rpc/client_bg.wasm')throw Error(x.wasm);});");
        fs::write(&file, source).unwrap();
        let result = std::process::Command::new("node")
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        scripts.push(script);
    }
    assert_eq!(scripts[0], scripts[1]);
}
