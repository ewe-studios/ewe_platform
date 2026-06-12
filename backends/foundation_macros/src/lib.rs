use proc_macro::TokenStream;

mod arrow_json_schema;
mod arrow_schema;
mod crate_paths;
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
mod valtron_entry;
mod wasm_test;
mod wasmbin_codec;

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

/// Runs a function with the valtron execution engine live around it (the
/// `#[tokio::main]` analogue): initializes the pool via
/// `foundation_core::valtron::initialize_pool(seed, threads)` and holds the
/// returned `PoolGuard` until the function body has fully returned.
///
/// Arguments (both optional): `#[valtron(seed = 42, threads = 4)]`. Without
/// `seed`, a `RandomState`-derived u64 is used; without `threads`, the engine
/// default applies. Also exported as `foundation_core::valtron::valtron`.
#[proc_macro_attribute]
pub fn valtron(attr: TokenStream, item: TokenStream) -> TokenStream {
    valtron_entry::valtron(attr.into(), item.into()).into()
}

/// `#[test]` + a live valtron engine around the case (the `#[tokio::test]`
/// analogue). Defaults to `Some(3)` pool threads and clamps an explicit
/// `threads = N` to a minimum of 3 — scheduling-sensitive tests need real
/// interleaving. Also exported as `foundation_core::valtron::valtron_test`.
#[proc_macro_attribute]
pub fn valtron_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    valtron_entry::valtron_test(attr.into(), item.into()).into()
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
