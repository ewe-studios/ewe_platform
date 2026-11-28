# Feature 12 documentation scenarios

The record at `04561dcdd` gives 42 ignored tests. The source has 42 ignored example fences.
The source does not identify the 42 results individually. This table maps the fences in source order.
It records the current scenario and the proof for each case.

Run the commands in the [macro crate scenario guide](../../../../backends/foundation_macros/docs/scenarios/README.md).
A fixture link gives a reproducible candidate. A pending row still needs the shown check.
This record does not replace the separate review of removed operations.

| Case | Historical source | Scenario | Fixture | Result |
| ---: | --- | --- | --- | --- |
| 1 | [`html_macro/mod.rs:8`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/html_macro/mod.rs#L8) | HTML tree and signal effects | [reactive_slot_renders_and_updates](../../../../backends/foundation_wasm_ui/tests/html_macro_tests.rs) | Pending HTML check |
| 2 | [`lib.rs:66`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L66) | Embed a directory by workspace or crate path | [can_read_data_from_js_host_runtime](../../../../backends/foundation_macros/tests/test_macros_embeddeders.rs) | Pending Embeds check |
| 3 | [`lib.rs:116`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L116) | Retain mobile file metadata and use an asset root | [mobile_assets_keep_metadata_and_use_the_runtime_root](../../../../backends/foundation_macros/tests/test_macros_embeddeders.rs) | Pending Embeds check |
| 4 | [`lib.rs:145`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L145) | Embed a file with Unicode encodings | [can_read_data_from_js_host_runtime](../../../../backends/foundation_macros/tests/test_macros_embeddeders.rs) | Pending Embeds check |
| 5 | [`lib.rs:200`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L200) | Attach a named WebAssembly entry point | [wasm_entrypoint_compile_tests](../../../../backends/foundation_macros/tests/units/foundation_macros_wasm_entrypoint_tests.rs) | Passed and linked from source |
| 6 | [`lib.rs:238`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L238) | Configure a WebAssembly console tracing subscriber | [wasm_logs on fetch](../../../../examples/cf-agent-loop/src/lib.rs) | Pending Workers check |
| 7 | [`lib.rs:302`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L302) | Derive a type UUID | [derive_macro_works](../../../../backends/foundation_core/tests/type_uuid_tests.rs) | Pending UUID check |
| 8 | [`lib.rs:326`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L326) | Attach a UUID to an external type | [external_type_uuid_proc_macro](../../../../backends/foundation_core/tests/type_uuid_tests.rs) | Pending UUID check |
| 9 | [`lib.rs:341`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L341) | Encode and decode a tagged message | [heterogeneous_message_types](../../../../backends/foundation_nativeapis/tests/ipc/mod.rs) | Pending IPC check |
| 10 | [`lib.rs:372`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L372) | Expose public delegation methods | [test_scaffoldable_skips_private](../../../../backends/foundation_macros/tests/units/foundation_macros_scaffold_tests.rs) | Passed and linked from source |
| 11 | [`lib.rs:391`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L391) | Derive a delegated implementation | [test_derive_scaffold_basic](../../../../backends/foundation_macros/tests/units/foundation_macros_scaffold_tests.rs) | Passed and linked from source |
| 12 | [`lib.rs:416`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L416) | Select delegated trait methods | [test_scaffold_impl_basic](../../../../backends/foundation_macros/tests/units/foundation_macros_scaffold_tests.rs) | Passed and linked from source |
| 13 | [`lib.rs:443`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L443) | Derive an Arrow schema | [test_derive_arrow_schema_basic](../../../../backends/foundation_macros/tests/units/foundation_macros_arrow_schema_tests.rs) | Passed and linked from source |
| 14 | [`lib.rs:465`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L465) | Derive an Arrow JSON schema | [test_derive_arrow_json_schema](../../../../backends/foundation_macros/tests/units/foundation_macros_arrow_json_schema_tests.rs) | Passed and linked from source |
| 15 | [`lib.rs:487`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L487) | Derive a JSON schema | [test_derive_json_schema_basic](../../../../backends/foundation_macros/tests/units/foundation_macros_json_schema_tests.rs) | Passed and linked from source |
| 16 | [`lib.rs:512`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L512) | Encode a record to Arrow | [test_to_arrow_single_row](../../../../backends/foundation_macros/tests/units/foundation_macros_arrow_serialize_tests.rs) | Passed and linked from source |
| 17 | [`lib.rs:534`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L534) | Decode an Arrow record | [test_roundtrip_single](../../../../backends/foundation_macros/tests/units/foundation_macros_arrow_serialize_tests.rs) | Passed and linked from source |
| 18 | [`lib.rs:611`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L611) | Run a WebAssembly test inside the Valtron pool | [valtron_wasm_test_initializes_the_pool](../../../../backends/foundation_core/integration/wasm_tests/src/lib.rs) | Pending Owned check |
| 19 | [`lib.rs:633`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L633) | Bind a runnable asynchronous WebAssembly case | [wasm-bindgen cases](../../../../backends/foundation_core/tests/valtron/async_first_api.rs) | Pending Bindgen check |
| 20 | [`lib.rs:710`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L710) | Serialize tests with the Foundation macro | [No exact caller fixture found](../../../../backends/foundation_macros/src/serial_test.rs) | Exact executable case pending |
| 21 | [`lib.rs:731`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L731) | Enforce a test timeout | [timeout guard tests](../../../../backends/foundation_core/tests/valtron/timeout_guard_unwind.rs) | Pending Valtron check |
| 22 | [`lib.rs:784`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L784) | Drive a server and page from the browser test macro | [macro_drives_a_served_page](../../../../backends/foundation_browser/tests/smoke.rs) | Pending Browser check |
| 23 | [`lib.rs:837`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L837) | Start a container for a test body | [docker_container_macro_serves_traffic_while_body_runs](../../../../backends/foundation_deployment_platform/tests/docker_macro_tests.rs) | Pending Docker check |
| 24 | [`lib.rs:852`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L852) | Inject a context into container setup | [the_macro_accepts_a_ctx_argument](../../../../backends/foundation_deployment_platform/tests/docker_macro_clock_arg_tests.rs) | Pending Docker check |
| 25 | [`lib.rs:865`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L865) | Start several containers for one test | [docker_container_macro_starts_multiple_containers](../../../../backends/foundation_deployment_platform/tests/docker_macro_tests.rs) | Pending Docker check |
| 26 | [`lib.rs:887`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L887) | Build a WireGuard configuration | [macro_produces_valid_config](../../../../backends/foundation_wireguard/tests/completeness_tests.rs) | Pending WireGuard check |
| 27 | [`lib.rs:909`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L909) | Old WireGuard entry point example | [Obsolete placeholder that calls an outdated connection signature](../../../../backends/foundation_macros/src/lib.rs) | Removed with owner approval |
| 28 | [`lib.rs:928`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L928) | Build a proxy configuration | [test_proxy_macro_minimal](../../../../backends/foundation_proxy/tests/proxy_macro_tests.rs) | Pending Proxy check |
| 29 | [`lib.rs:1020`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L1020) | Build and apply a theme | [theme_macro_builds_app_injects_and_reads_back](../../../../backends/foundation_wasm_ui/tests/theme_tests.rs) | Passed and linked from source |
| 30 | [`lib.rs:1069`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L1069) | Declare a code-first Connect RPC service | [No exact calling test found](../../../../backends/foundation_macros/src/connectrpc_service.rs) | Exact executable case pending |
| 31 | [`lib.rs:1090`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/lib.rs#L1090) | Generate a Rust service module | [No exact generate macro calling test found](../../../../backends/foundation_macros/src/connectrpc_service.rs) | Exact executable case pending |
| 32 | [`platform_test.rs:5`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/platform_test.rs#L5) | Select platform test backends | [No exact caller covering all backend variants found](../../../../backends/foundation_macros/src/platform_test.rs) | Exact executable case pending |
| 33 | [`theme_tokens.rs:10`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/theme_tokens.rs#L10) | Derive and render theme tokens | [theme_tokens_generate_the_spec_css](../../../../backends/foundation_wasm_ui/tests/theme_tests.rs) | Passed and linked from source |
| 34 | [`type_uuid.rs:102`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/type_uuid.rs#L102) | Attach a UUID to an external type | [external_type_uuid_proc_macro](../../../../backends/foundation_core/tests/type_uuid_tests.rs) | Pending UUID check |
| 35 | [`type_uuid.rs:133`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/type_uuid.rs#L133) | Dispatch message variants by type | [heterogeneous_message_types](../../../../backends/foundation_nativeapis/tests/ipc/mod.rs) | Pending IPC check |
| 36 | [`valtron_entry.rs:17`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/valtron_entry.rs#L17) | Run a function inside the Valtron pool | [valtron_entry_point_wraps_and_returns](../../../../backends/foundation_core/tests/valtron/valtron_macro_tests.rs) | Pending Valtron check |
| 37 | [`valtron_entry.rs:24`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/valtron_entry.rs#L24) | Old expansion of the Valtron entry point | [Obsolete expansion uses an outdated pool initializer](../../../../backends/foundation_macros/src/valtron_entry.rs) | Removed with owner approval |
| 38 | [`valtron_entry.rs:56`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/valtron_entry.rs#L56) | Use the Valtron test macro for test registration | [explicit_seed_and_threads_are_accepted](../../../../backends/foundation_core/tests/valtron/valtron_macro_tests.rs) | Pending Valtron check |
| 39 | [`wasm_logs.rs:18`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/wasm_logs.rs#L18) | Set up WebAssembly console and standard-output logging | [console caller only. Standard output needs separate proof](../../../../examples/cf-agent-loop/src/lib.rs) | Pending Workers check |
| 40 | [`wasm_test.rs:19`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/wasm_test.rs#L19) | Export a WebAssembly test | [WebAssembly test manifest and runner](../../../../backends/foundation_core/integration/wasm_tests/src/lib.rs) | Pending Owned check |
| 41 | [`wasm_test.rs:26`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_macros/src/wasm_test.rs#L26) | Keep the body and call its WebAssembly export | [WebAssembly test manifest and runner](../../../../backends/foundation_core/integration/wasm_tests/src/lib.rs) | Pending Owned check |
| 42 | [`context.rs:159`](https://github.com/ewe-studios/ewe_platform/blob/04561dcdd/backends/foundation_signals/src/context.rs#L159) | Toggle a signal from a button click without a value | [button_callback_toggles_signal_without_a_value](../../../../backends/foundation_wasm_ui/tests/html_macro_tests.rs) | Pending HTML check |

## Additional WebAssembly UI cases

These two fences are separate from the 42 macro and signal cases.

| Source | Actual scenario |
| --- | --- |
| `foundation_wasm_ui/src/app.rs` theme example | `theme_macro_builds_app_injects_and_reads_back` in the theme test file |
| `foundation_wasm_ui/src/app.rs` mount example | `successive_mounts_append_in_order` in the app test file |

Both focused files pass. The app file passes seven tests and the theme file passes six.

The [application scenario guide](../../../../backends/foundation_wasm_ui/docs/scenarios/apps-and-frame-operations.md) gives the commands.

## Retired operation fixture equivalence

The [19-operation test](../../../../backends/foundation_wasm_ui/integration/test/retired-operations.test.js) verifies the full prior document and registry snapshots.
The old comparison passes 20 tests at the parent of `dd15675cf`. The extra test counts the 19 cases.
The current comparison passes the same count and checks every result without a retired encoder.
The full current WebAssembly UI JavaScript run passes 181 tests with one existing optional fixture skip.
See the application scenario guide for the separate encoding and benchmark boundaries.
