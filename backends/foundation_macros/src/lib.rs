use proc_macro::TokenStream;

mod embedders;
mod json_hash;
mod type_uuid;
mod wasm_entrypoint;

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
/// ```
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
/// ```
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
/// ```rust
/// use foundation_macros::TypeUuid;
///
/// #[derive(TypeUuid)]
/// #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
/// struct MyMessage {
///     data: Vec<u8>,
/// }
/// ```
///
/// Generate UUIDs at https://www.uuidgenerator.net
#[proc_macro_derive(TypeUuid, attributes(uuid))]
pub fn type_uuid_derive(item: TokenStream) -> TokenStream {
    type_uuid::type_uuid_derive(item)
}

/// Implement [`foundation_core::type_uuid::TypeUuid`] for a foreign type
/// that you cannot add a derive attribute to.
///
/// # Example
///
/// ```rust
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
