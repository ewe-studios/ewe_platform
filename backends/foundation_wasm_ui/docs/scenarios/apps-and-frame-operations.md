# Application setup and frame operations

Run these commands from the repository root.

```sh
cargo test --locked --profile uat -p foundation_wasm_ui --test app_tests --test theme_tests --test html_macro_tests
node --test backends/foundation_wasm_ui/integration/test/retired-operations.test.js
```

## Install a theme before building content

The [theme fixture](../../tests/theme_tests.rs) uses `theme!` and installs the result on the app.
It checks the CSS in the app, the generated tokens, and the queued instructions for the document head.

## Mount content before a script

The [mount fixture](../../tests/app_tests.rs) mounts content first and a script second.
It checks that the two frames append to the body in the same order.
A separate test verifies that a mounted element uses the reserved body target.

## Preserve the effect of retired operations

[The frame test](../../integration/test/retired-operations.test.js) uses the 19 current binary fixtures.
It checks the document structure, text, attributes, event listeners, style, and node registry.
It also checks that the current frame reports no errors.

The [expected results](../../integration/fixtures/frames/legacy-operation-snapshots.json) come from the old applicator at the parent of `dd15675cf`.
The [old parity test](https://github.com/ewe-studios/ewe_platform/blob/f7cdbdb498c0b19d71c064a6964db93442a9655d/backends/foundation_wasm_ui/integration/test/frames-parity.test.js) passes all 19 operation cases before export.
The current test compares the active wire results to those snapshots.
The repository does not need the retired encoder or frame files to run the test.

The retired Arrow, JSON, and columnar formats have their own parser and layout tests.
Those layouts do not apply to the current frame format.
The [wire fixtures](../../../foundation_ui_wire/tests/fixtures_tests.rs) cover current operation forms and malformed input.
The [frame application tests](../../integration/test/frames-apply.test.js) cover current shared rows and columns.
The old size benchmarks measure an obsolete encoder comparison.
