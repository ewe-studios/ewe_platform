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
