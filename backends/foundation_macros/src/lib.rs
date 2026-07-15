use proc_macro::TokenStream;

mod connectrpc_service;
mod arrow_json_schema;
mod arrow_schema;
mod crate_paths;
mod docker_container;
mod proxy;
mod embedders;
mod from_arrow;
mod json_hash;
mod json_schema;
mod scaffold;
mod schema_fields;
mod to_arrow;
mod type_uuid;
mod wasm_entrypoint;
mod html_macro;
mod theme_macro;
mod theme_tokens;
mod wasm_modes;
mod serial_test;
mod valtron_entry;
mod wasm_test;
mod bindgen_test;
mod wasm_ui_server_entry;
mod wasmbin_codec;
mod wireguard;

// scaffold!() — marker for methods delegated by #[scaffold_impl].
// Defined in foundation_nostd (macro_rules! can't be exported from proc-macro crates).
// Import from foundation_nostd::macros.

// ── Embed macros ──

/// [`embed_directory_as`] specifies a proc macro for embedding files into
/// your binary as a series of UTF8 array and UTF16 array with
/// additional meta data like the hash, `date_modified` and mimetype
/// for the target source file.
///
/// You can use:
///
/// `$ROOT_CRATE`: as placeholder for the file path of the root workspace crate.
///
/// `$CURRENT_CRATE`: as placeholder for the file path of the current crate.
///
/// Examples:
///
/// (`ignore`: the derive embeds the referenced files at expansion time, so the
/// example only compiles in a crate that actually has these directories.)
///
/// ```ignore
///  use foundation_macros::EmbedDirectoryAs;
///
///  // Use root crate directory to better ensure consistent path
///  #[derive(EmbedDirectoryAs)]
///  #[source = "$ROOT_CRATE/runtime/js"]
///  pub struct JSHostRuntime;
///
///  // Use crate directory to better ensure consistent path
///  #[derive(EmbedDirectoryAs)]
///  #[source = "$CURRENT_CRATE/runtime/css"]
///  pub struct CSSAssets;
///
///  // embed content with using relative paths
///  #[derive(EmbedDirectoryAs)]
///  #[source = "./runtime/images"]
///  pub struct ImageAssets;
///
///  // compress content with gzip compression algorithm
///  #[derive(EmbedDirectoryAs)]
///  #[source = "./runtime/images"]
///  #[gzip_compression]
///  pub struct ImageAssets2;
///
///  // compress content with brottli compression algorithm
///  #[derive(EmbedDirectoryAs)]
///  #[source = "./runtime/images"]
///  #[brottli_compression]
///  pub struct ImageAssets3;
///
/// ```
///
#[proc_macro_derive(
    EmbedDirectoryAs,
    attributes(source, gzip_compression, brottli_compression, with_utf16)
)]
pub fn embed_directory_as(item: TokenStream) -> TokenStream {
    embedders::embed_directory_on_struct(item)
}

/// [`embed_file_as`] specifies a proc macro for embedding files into
/// your binary as a series of UTF8 array and UTF16 array with
/// additional meta data like the hash, `date_modified` and mimetype
/// for the target source file.
///
/// You can use:
///
/// `$ROOT_CRATE`: as placeholder for the file path of the root workspace crate.
///
/// `$CURRENT_CRATE`: as placeholder for the file path of the current crate.
///
/// Examples:
///
/// (`ignore`: the derive embeds the referenced files at expansion time, so the
/// example only compiles in a crate that actually has these files.)
///
/// ```ignore
///  use foundation_macros::EmbedFileAs;
///
///  // Use root crate directory to better ensure consistent path
///  #[derive(EmbedFileAs)]
///  #[source = "$ROOT_CRATE/runtime/js/js_host_runtime.js"]
///  pub struct JSHostRuntime;
///
///  // Use is_binary to indicate file is not a string file but binary file
///  // so file does not get a valid utf16 content.
///  #[derive(EmbedFileAs)]
///  #[is_binary]
///  #[source = "$ROOT_CRATE/runtime/js/js_host_runtime.js"]
///  pub struct JSHostRuntime;
///
///  // Use crate directory to better ensure consistent path
///  #[derive(EmbedFileAs)]
///  #[source = "$CURRENT_CRATE/runtime/js/runtime.js"]
///  pub struct RuntimeCore;
///
///  #[derive(EmbedFileAs)]
///  #[source = "runtime/js/packer.js"]
///  #[gzip_compression]
///  pub struct PackerCore2;
///
///  #[derive(EmbedFileAs)]
///  #[source = "runtime/js/packer.js"]
///  #[brottli_compression]
///  pub struct PackerCore3;
/// ```
///
#[proc_macro_derive(
    EmbedFileAs,
    attributes(source, gzip_compression, brottli_compression, with_utf16)
)]
pub fn embed_file_as(item: TokenStream) -> TokenStream {
    embedders::embed_file_on_struct(item)
}

/// WHY: WASM binary entrypoints need a discoverable marker so the source
/// scanner can find them without linker tricks (which don't work on WASM).
///
/// WHAT: Attribute proc macro that marks a function as a WASM binary entrypoint.
///
/// HOW: Validates the attribute has `name` and `desc` string arguments,
/// verifies it's applied to a function, then passes the function through
/// unchanged. The `foundation_codegen` scanner discovers these at build time.
///
/// # Required Attributes
///
/// - `name` — string literal naming the WASM binary (e.g., `name = "auth_worker"`)
/// - `desc` — string literal describing the entrypoint
///
/// # Examples
///
/// ```ignore
/// use foundation_macros::wasm_entrypoint;
///
/// #[wasm_entrypoint(name = "auth_worker", desc = "Authentication worker")]
/// pub fn auth_handler() {
///     // Function body
/// }
/// ```
///
/// # Panics
///
/// Never panics. Returns compile errors for invalid usage.
#[proc_macro_attribute]
pub fn wasm_entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_entrypoint::expand(attr.into(), item.into()).into()
}

/// Derive macro that adds a `struct_hash()` method to structs.
///
/// # Requirements
///
/// The struct must also derive `serde::Serialize`. The generated
/// `struct_hash()` method serializes the struct to JSON and computes
/// a SHA-256 hash, encoded as base85.
///
/// # Example
///
/// ```rust
/// use foundation_macros::JsonHash;
/// use serde::Serialize;
///
/// #[derive(JsonHash, Serialize)]
/// pub struct MyResource {
///     pub name: String,
///     pub value: i32,
/// }
///
/// let resource = MyResource { name: "test".into(), value: 42 };
/// let hash = resource.struct_hash();
/// ```
///
/// # Hash Computation
///
/// The hash is computed as:
/// 1. Concatenate struct name + JSON serialization: `"MyResource{...}"`
/// 2. Compute SHA-256 of the concatenated string
/// 3. Encode the hash as base85 string
///
/// This ensures:
/// - Same struct + same values = same hash
/// - Different struct names = different hashes (even with identical content)
/// - Deterministic, reproducible results
#[proc_macro_derive(JsonHash)]
pub fn json_hash_derive(item: TokenStream) -> TokenStream {
    json_hash::json_hash_derive(item)
}

/// Derive macro that implements [`foundation_core::type_uuid::TypeUuid`] for a type.
///
/// Assigns a stable, unique 128-bit UUID to a Rust type at compile time.
/// The UUID is specified as a string attribute and converted to a `[u8; 16]` byte array.
///
/// # Example
///
/// (`ignore`: the expansion references `foundation_core::type_uuid`, which this
/// proc-macro crate cannot depend on — see the runnable doctest on
/// `foundation_core::type_uuid` instead.)
///
/// ```ignore
/// use foundation_macros::TypeUuid;
///
/// #[derive(TypeUuid)]
/// #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
/// struct MyMessage {
///     data: Vec<u8>,
/// }
/// ```
///
/// Generate UUIDs at <https://www.uuidgenerator.net>
#[proc_macro_derive(TypeUuid, attributes(uuid))]
pub fn type_uuid_derive(item: TokenStream) -> TokenStream {
    type_uuid::type_uuid_derive(item)
}

/// Implement [`foundation_core::type_uuid::TypeUuid`] for a foreign type
/// that you cannot add a derive attribute to.
///
/// # Example
///
/// (`ignore`: the expansion references `foundation_core::type_uuid`, which this
/// proc-macro crate cannot depend on.)
///
/// ```ignore
/// foundation_macros::external_type_uuid!(std::time::Duration, "449a4224-4665-47ce-88a2-8d0310d20572");
/// ```
#[proc_macro]
pub fn external_type_uuid(tokens: TokenStream) -> TokenStream {
    type_uuid::external_type_uuid_impl(tokens)
}

/// Derive `MessageBox` for an enum of heterogeneous message types.
///
/// Each variant must wrap exactly one type that implements `TypeUuid + Serialize + Deserialize`.
/// The generated impl dispatches encode/decode by UUID, enabling multiple message types on a single bus.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{TypeUuid, MessageBox};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(TypeUuid, Serialize, Deserialize)]
/// #[uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]
/// struct FileEvent { path: String }
///
/// #[derive(TypeUuid, Serialize, Deserialize)]
/// #[uuid = "11111111-2222-3333-4444-555555555555"]
/// struct LogEntry { message: String }
///
/// #[derive(MessageBox)]
/// enum AppMessage {
///     File(FileEvent),
///     Log(LogEntry),
/// }
/// ```
#[proc_macro_derive(MessageBox)]
pub fn message_box_derive(item: TokenStream) -> TokenStream {
    type_uuid::message_box_derive(item)
}

// ── Scaffold macros ──

/// Attribute applied to an impl block to enable automatic method forwarding
/// via `#[derive(Scaffold)]`. Generates a hidden `macro_rules!` macro encoding
/// all pub method signatures.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::scaffoldable;
///
/// #[scaffoldable]
/// impl MyService {
///     pub fn process(&self, input: &[u8]) -> Vec<u8> { /* ... */ }
///     pub fn status(&self) -> Status { /* ... */ }
/// }
/// ```
#[proc_macro_attribute]
pub fn scaffoldable(attr: TokenStream, item: TokenStream) -> TokenStream {
    scaffold::scaffoldable(attr.into(), item.into()).into()
}

/// Derive macro that generates forwarding methods from `#[scaffoldable]`-marked
/// inner types. Use `#[scaffold(field)]` on struct fields to enable forwarding.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{scaffoldable, Scaffold};
///
/// #[scaffoldable]
/// impl InMemoryStore {
///     pub fn get(&self, key: &str) -> Option<Vec<u8>> { /* ... */ }
///     pub fn set(&self, key: &str, value: Vec<u8>) { /* ... */ }
/// }
///
/// #[derive(Scaffold)]
/// pub struct ThreadSafeStore {
///     #[scaffold(field)]
///     inner: Arc<Mutex<InMemoryStore>>,
/// }
/// ```
#[proc_macro_derive(Scaffold, attributes(scaffold, scaffold_call))]
pub fn scaffold_derive(item: TokenStream) -> TokenStream {
    scaffold::scaffold_derive(item.into()).into()
}

/// Attribute for manual impl block delegation. Methods with `scaffold!()`
/// bodies get delegation generated; methods with real bodies are kept as overrides.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{scaffold_impl, scaffold};
///
/// #[scaffold_impl(via = "self.fs")]
/// impl VfsFileSystem for DirectoryDelta {
///     fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
///
///     fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
///         let inner = self.fs.open_directory(path)?;
///         Ok(FilteredDirectory { inner })
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn scaffold_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    scaffold::scaffold_impl(attr.into(), item.into()).into()
}

// ── Schema derive macros ──

/// Derive macro that generates `impl ArrowSchema` for a struct.
///
/// Maps struct fields to Arrow `DataType` variants and generates
/// `fields()` and `schema()` methods. Only named structs are supported.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::ArrowSchema;
///
/// #[derive(ArrowSchema)]
/// struct FileEntry {
///     name: String,
///     size: u64,
///     checksum: Option<Vec<u8>>,
/// }
/// ```
#[proc_macro_derive(ArrowSchema)]
pub fn arrow_schema_derive(item: TokenStream) -> TokenStream {
    arrow_schema::arrow_schema_derive(item.into()).into()
}

/// Derive macro that generates `impl ArrowJsonSchema` for a struct.
///
/// Produces a pre-built `serde_json::Value` with Arrow-specific type names
/// (`UInt64`, Utf8, Binary, etc.). Requires `ArrowSchema` to also be derived.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{ArrowSchema, ArrowJsonSchema};
///
/// #[derive(ArrowSchema, ArrowJsonSchema)]
/// struct FileEntry {
///     name: String,
///     size: u64,
/// }
/// ```
#[proc_macro_derive(ArrowJsonSchema)]
pub fn arrow_json_schema_derive(item: TokenStream) -> TokenStream {
    arrow_json_schema::arrow_json_schema_derive(item.into()).into()
}

/// Derive macro that generates `impl JsonSchema` for a struct.
///
/// Produces a pre-built `serde_json::Value` conforming to the standard
/// W3C/IETF JSON Schema specification. Non-Option fields are listed
/// in the `required` array.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::JsonSchema;
///
/// #[derive(JsonSchema)]
/// struct UserProfile {
///     name: String,
///     age: u32,
///     bio: Option<String>,
/// }
/// ```
#[proc_macro_derive(JsonSchema)]
pub fn json_schema_derive(item: TokenStream) -> TokenStream {
    json_schema::json_schema_derive(item.into()).into()
}

// ── Arrow serialization derives ──

/// Derive macro that generates `impl ToArrow` for a struct.
///
/// Generates `to_arrow(&self)` and `to_arrow_batch(&[Self])` methods
/// that convert struct values into Arrow `RecordBatch`. Requires
/// `ArrowSchema` to also be derived.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{ArrowSchema, ToArrow};
///
/// #[derive(ArrowSchema, ToArrow)]
/// struct FileEntry {
///     name: String,
///     size: u64,
/// }
/// ```
#[proc_macro_derive(ToArrow)]
pub fn to_arrow_derive(item: TokenStream) -> TokenStream {
    to_arrow::to_arrow_derive(item.into()).into()
}

/// Derive macro that generates `impl FromArrow` for a struct.
///
/// Generates `from_arrow(batch)` and `from_arrow_batch(batch)` methods
/// that extract struct values from Arrow `RecordBatch`. Requires
/// `ArrowSchema` to also be derived.
///
/// # Example
///
/// ```ignore
/// use foundation_macros::{ArrowSchema, FromArrow};
///
/// #[derive(ArrowSchema, FromArrow)]
/// struct FileEntry {
///     name: String,
///     size: u64,
/// }
/// ```
#[proc_macro_derive(FromArrow)]
pub fn from_arrow_derive(item: TokenStream) -> TokenStream {
    from_arrow::from_arrow_derive(item.into()).into()
}

/// Run a synstructure-based derive body: parse the input, build the `Structure`,
/// and surface any error as a compile error (replaces upstream `decl_derive!`).
fn run_synstructure(
    item: TokenStream,
    body: fn(synstructure::Structure) -> proc_macro2::TokenStream,
) -> TokenStream {
    let input = match syn::parse::<syn::DeriveInput>(item) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error().into(),
    };
    match synstructure::Structure::try_new(&input) {
        Ok(structure) => body(structure).into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derives the WebAssembly binary codec (`Encode` + `Decode`, and
/// `DecodeWithDiscriminant` for `#[repr(N)]` enums / `#[wasmbin(discriminant = N)]`
/// structs) for `foundation_codegen::wasm` model types.
///
/// Ported from `wasmbin-derive` (<https://github.com/RReverser/wasmbin>), Apache-2.0.
#[proc_macro_derive(Wasmbin, attributes(wasmbin))]
pub fn wasmbin_derive(item: TokenStream) -> TokenStream {
    run_synstructure(item, wasmbin_codec::wasmbin_derive)
}

/// Marks a `foundation_codegen::wasm` model type as countable — serializable inside
/// LEB128 length-prefixed collections.
///
/// Ported from `wasmbin-derive` (<https://github.com/RReverser/wasmbin>), Apache-2.0.
#[proc_macro_derive(WasmbinCountable)]
pub fn wasmbin_countable_derive(item: TokenStream) -> TokenStream {
    run_synstructure(item, wasmbin_codec::wasmbin_countable_derive)
}

/// Derives typed deep-traversal (`Visit::visit_children` / `visit_children_mut`)
/// over every field of a `foundation_codegen::wasm` model type.
///
/// Ported from `wasmbin-derive` (<https://github.com/RReverser/wasmbin>), Apache-2.0.
#[proc_macro_derive(Visit)]
pub fn wasmbin_visit_derive(item: TokenStream) -> TokenStream {
    run_synstructure(item, wasmbin_codec::wasmbin_visit_derive)
}

/// Compiles a test case into the owned wasm test model (feature 13, decision 031 —
/// no wasm-bindgen): keeps the fn, adds a discoverable `__fwt_<name>` export that
/// reports its outcome over the `foundation_wasm` ABI (`host_report`), and writes a
/// `name|flags` line into the `__fwt_manifest` custom section for the testbed.
///
/// Flags: `#[wasm_test(should_panic)]` (runner inverts on the trap),
/// `#[wasm_test(ignore)]` (reported as skipped). `async fn` cases run on the owned
/// `schedule_timeout` re-poll loop.
#[proc_macro_attribute]
pub fn wasm_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_test::wasm_test(attr.into(), item.into()).into()
}

/// `#[valtron_wasm_test]` — `#[wasm_test]` with valtron pool auto-init (F52).
///
/// Same as `#[wasm_test]` (owned export + manifest), but wraps the test body
/// in `valtron::initialize_pool()` / `drop()` so `execute()` + `spawn()` are
/// available without manual pool setup.
///
/// ```ignore
/// use foundation_macros::valtron_wasm_test;
///
/// #[valtron_wasm_test]
/// fn my_test() {
///     let mut stream = valtron::execute(task, None).unwrap();
/// }
/// ```
#[proc_macro_attribute]
pub fn valtron_wasm_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_test::valtron_wasm_test(attr.into(), item.into()).into()
}

/// `#[valtron_bindgen]` — wasm-bindgen browser test with valtron pool (F52).
///
/// Single attribute that combines:
/// 1. Browser-mode link-section marker for wasm-bindgen-test-runner
/// 2. Valtron single-threaded pool init + teardown (like `#[valtron_test]`)
/// 3. Delegation to `#[wasm_bindgen_test]` for test discovery (`__wbgt_` export)
///
/// ```ignore
/// use foundation_macros::valtron_bindgen;
/// use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};
///
/// #[valtron_bindgen]
/// fn my_browser_test() {
///     let (task, _delivery) = client.open_websocket_task(...);
///     let mut stream = valtron::execute(task, None).expect("execute");
///     // pool is live here
/// }
/// ```
#[proc_macro_attribute]
pub fn valtron_bindgen(attr: TokenStream, item: TokenStream) -> TokenStream {
    bindgen_test::valtron_bindgen(attr, item)
}

/// Runs a function with the valtron execution engine live around it (the
/// `#[tokio::main]` analogue): initializes the pool via
/// `foundation_core::valtron::initialize_pool(seed, threads)` and holds the
/// returned `PoolGuard` until the function body has fully returned.
///
/// Arguments (both optional): `#[valtron(seed = 42, threads = 4)]`. `threads`
/// supplied → `Some(N)`, absent → `None` (engine default). Without `seed`, a
/// random `RandomState`-derived u64 is used. Also exported as
/// `foundation_core::valtron::valtron`.
#[proc_macro_attribute]
pub fn valtron(attr: TokenStream, item: TokenStream) -> TokenStream {
    valtron_entry::valtron(attr.into(), item.into()).into()
}

/// `#[test]` + a live valtron engine around the case (the `#[tokio::test]`
/// analogue). Same `seed`/`threads` rules as `#[valtron]`: `threads = N` →
/// `Some(N)`, absent → `None`; `seed` absent → random. Also exported as
/// `foundation_core::valtron::valtron_test`.
#[proc_macro_attribute]
pub fn valtron_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    valtron_entry::valtron_test(attr.into(), item.into()).into()
}

/// `#[serial_test]` — serialize tests that share process-global state.
///
/// Replaces `#[test]` (like `#[valtron_test]` — do not stack with `#[test]`).
/// All `#[serial_test]` functions in the same test binary share a single
/// `FairGate` (ticket-based FIFO mutex), so they run one at a time in
/// arrival order without starvation.
///
/// ```ignore
/// use foundation_macros::serial_test;
///
/// #[serial_test]
/// fn test_global_timer_state() {
///     // safe to assert on process-global statics here
/// }
/// ```
#[proc_macro_attribute]
pub fn serial_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    serial_test::serial_test(attr.into(), item.into()).into()
}

/// `#[timeout(ms)]` — kills the test if it exceeds the given millisecond limit.
/// Spawns the test body in a thread and waits with `recv_timeout`. On timeout,
/// panics with a message showing actual vs allowed duration.
///
/// ```ignore
/// #[timeout(60000)]
/// fn test_slow_operation() {
///     // panics if this takes more than 60 seconds
/// }
/// ```
#[proc_macro_attribute]
pub fn timeout(attr: TokenStream, item: TokenStream) -> TokenStream {
    use quote::quote;

    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let time_ms: syn::LitInt = syn::parse(attr).unwrap_or_else(|_| {
        panic!("timeout: integer in ms expected. Example: #[timeout(60000)]")
    });
    let vis = &input.vis;
    let sig = &input.sig;
    let output = &sig.output;
    let body = &input.block;
    let attrs = &input.attrs;

    let result = quote! {
        #(#attrs)*
        #vis #sig {
            fn __timeout_callback() #output
            #body
            let __timeout_start = std::time::Instant::now();
            type __PanicPayload = std::boxed::Box<dyn std::any::Any + std::marker::Send + 'static>;
            let (sender, receiver) = std::sync::mpsc::channel::<std::result::Result<_, __PanicPayload>>();
            std::thread::spawn(move || {
                let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    __timeout_callback()
                }));
                let _ = sender.send(panic_result);
            });
            match receiver.recv_timeout(std::time::Duration::from_millis(#time_ms)) {
                std::result::Result::Ok(std::result::Result::Ok(t)) => return t,
                std::result::Result::Ok(std::result::Result::Err(payload)) => {
                    std::panic::resume_unwind(payload);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    panic!("timeout: the test took {} ms. Max {} ms", __timeout_start.elapsed().as_millis(), #time_ms);
                },
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("timeout: test thread disconnected unexpectedly");
                },
            }
        }
    };
    result.into()
}

/// `#[wasm_ui_server]` — wrap a fn into a `#[test]` that boots a `TestServer` +
/// browser, navigates a page, runs the body, and tears everything down on
/// success/error/panic (spec-43). Re-exported by `foundation_browser`.
///
/// ```ignore
/// #[wasm_ui_server(headless = true)]
/// fn dialog_traps_focus(server: &TestServer, page: &Page) -> foundation_browser::Result<()> {
///     page.locator("dialog").expect().to_be_visible()?;
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn wasm_ui_server(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_ui_server_entry::wasm_ui_server(attr.into(), item.into()).into()
}

/// `#[docker_container]` — start Docker containers for the duration of a function.
///
/// Parses key=value attributes (`image`, `port`, `network`, `wait_stdout`, etc.)
/// into a `ContainerConfig`, starts the container before the function body,
/// and stops/removes it after (even on panic — Drop cleanup).
///
/// # Examples
///
/// ```ignore
/// use foundation_deployment_platform::docker_container;
///
/// #[valtron_test]
/// #[docker_container(image = "redis:7", port = 6379)]
/// fn test_redis() {
///     // Redis is running at localhost:<auto-assigned port>
/// }
/// ```
#[proc_macro_attribute]
pub fn docker_container(attr: TokenStream, item: TokenStream) -> TokenStream {
    docker_container::docker_container(attr, item)
}

/// `wireguard!` — compile-time WireGuard mesh configuration (spec-55, feature 09).
///
/// Desugars a custom block syntax into a `WgConfig` builder chain.
/// Unknown keys and missing required fields are compile errors.
///
/// ```ignore
/// use foundation_wireguard::wireguard;
///
/// let config = wireguard! {
///     seed: "base64url-seed...",
///     network_id: "deadbeef...",
///     udp_listen: "0.0.0.0:51820",
///     relay: { advertise: true },
///     security: { mtls: false },
/// };
/// let node = foundation_wireguard::native::WgNode::from_config(config);
/// ```
#[proc_macro]
pub fn wireguard(input: TokenStream) -> TokenStream {
    wireguard::wireguard_impl(input)
}

/// `#[wireguard_main]` — entry point that initialises the valtron pool, joins the mesh,
/// and hands a [`WgHandle`] to the annotated function (spec-55, feature 09).
///
/// ```ignore
/// use foundation_wireguard::wireguard_main;
///
/// #[wireguard_main(config = "wireguard.toml")]
/// fn main(handle: &foundation_wireguard::native::WgHandle) {
///     let stream = handle.tcp_connect(peer_ip, 8080).unwrap();
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn wireguard_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    wireguard::wireguard_main_impl(attr, item)
}

/// `proxy!` — compile-time proxy configuration (Decision 19).
///
/// Desugars a custom block syntax into a `ProxyConfig` builder chain.
/// Unknown keys and missing required fields are compile errors.
///
/// ```ignore
/// use foundation_proxy::proxy;
///
/// let config = proxy! {
///     domain: "example.com",
///     public_ip: "1.2.3.4",
///     ssl: lets_encrypt { email: "admin@example.com" },
///     services: {
///         app: {
///             host: "app.example.com",
///             backends: ["http://localhost:3000"],
///             health_check: { path: "/up", interval: 5, timeout: 2 },
///         },
///     },
/// };
/// config.start()?;
/// ```
#[proc_macro]
pub fn proxy(input: TokenStream) -> TokenStream {
    proxy::proxy_impl(input)
}

/// `html!` — compile-time HTML templates producing typed
/// [`foundation_ui_traits::Html`] trees with `Part` descriptors (feature 03,
/// decisions 001/005/008/029).
///
/// Two forms:
/// - `html! { <div>..</div> }` — a pure `Html` value.
/// - `html! { ctx, receiver, <div>..</div> }` — additionally queues the DOM
///   build as `DomOp`s on `receiver` and creates one effect per dynamic
///   slot/attribute (effects run immediately).
///
/// See `foundation_macros::html_macro` module docs for the full walkthrough.
#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    html_macro::html(input.into()).into()
}

/// `#[derive(ThemeTokens)]` — compile-time theme CSS from `#[token(...)]`
/// fields (feature 09, decision 020): custom properties, dark-mode overrides
/// (explicit or ~80%-luminance auto-derived for colors), per-token utility
/// classes, and the built-in utility set, all as one `'static` string
/// (`Theme::CSS` / `theme.css_string()`).
#[proc_macro_derive(ThemeTokens, attributes(token))]
pub fn theme_tokens(item: TokenStream) -> TokenStream {
    theme_tokens::theme_tokens_derive(item.into()).into()
}

/// `theme! { … }` — the headline theme API (decision 021). One function-like
/// macro that reads like a struct of design-token blocks (`colors`, `spacing`,
/// `padding`, `margin`, `radius`, `shadow`, `font_size`, `animation`; unknown
/// blocks are a compile error) and expands to a `const`-capable
/// `foundation_theme::GeneratedTheme` with compile-time-generated CSS.
///
/// ```ignore
/// let theme = theme! {
///     colors  { primary: { light: "#3b82f6", dark: "#60a5fa" }, bg: "#ffffff" }
///     spacing { sm: 8px, md: 16px }
/// };
/// let app = App::new().theme(theme);
/// ```
#[proc_macro]
pub fn theme(input: TokenStream) -> TokenStream {
    theme_macro::theme(input.into()).into()
}

/// `#[wasm_bin]` — main-thread WASM entrypoint (feature 10, decision 014).
/// Marker-validated; the build pipeline reads it from source.
#[proc_macro_attribute]
pub fn wasm_bin(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_modes::wasm_bin(attr.into(), item.into()).into()
}

/// `#[wasm_worker]` — web-worker WASM entrypoint (feature 10).
#[proc_macro_attribute]
pub fn wasm_worker(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_modes::wasm_worker(attr.into(), item.into()).into()
}

/// `#[wasm_service]` — service-worker WASM entrypoint with a route table
/// (feature 10, decision 017). `routes = ["/api/…"]` is required.
#[proc_macro_attribute]
pub fn wasm_service(attr: TokenStream, item: TokenStream) -> TokenStream {
    wasm_modes::wasm_service(attr.into(), item.into()).into()
}

// ── ConnectRPC code-first generation (Feature 27, Decision 10 Mode 3) ────

/// `#[service]` — code-first ConnectRPC (Decision 10 Mode 3).
///
/// Transforms a Rust trait definition into a full ConnectRPC service:
/// service name constant, procedure path constants (R1), the trait with
/// default unimplemented bodies, registration fn, `UnimplementedXxxHandler`
/// (R2), typed client (R4), and an exported descriptor macro for cross-crate
/// generation via `generate!`.
///
/// # Attribute arguments
///
/// - `package = "pkg.name.v1"` — (required) protobuf-style package prefix.
/// - `codecs(json, arrow, proto)` — (optional, default `json`) codec families.
///
/// # Example
///
/// ```ignore
/// use foundation_connectrpc::{service, Ctx, Request, Response, ConnectResult};
///
/// #[service(package = "my.api.v1")]
/// trait MyService {
///     async fn unary(&self, ctx: Ctx, req: Request<MyReq>)
///         -> ConnectResult<Response<MyRes>>;
/// }
/// ```
#[proc_macro_attribute]
pub fn service(attr: TokenStream, item: TokenStream) -> TokenStream {
    connectrpc_service::expand_service(attr.into(), item.into()).into()
}

/// `generate!` — cross-crate ConnectRPC artifact generation.
///
/// Re-expands a descriptor macro exported by `#[service]` in
/// another crate inside a new module.
///
/// # Example
///
/// ```ignore
/// use foundation_connectrpc::generate;
/// generate!(my_api::my_api_rpc_definitions => mod my_svc {
///     server, client
/// });
/// ```
#[proc_macro]
pub fn generate(input: TokenStream) -> TokenStream {
    connectrpc_service::expand_generate(input.into()).into()
}

