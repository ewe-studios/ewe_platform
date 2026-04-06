//! JsonHash derive macro implementation.
//!
//! WHY: Generates a `struct_hash()` method for structs that produces a deterministic
//! hash from the struct name + serialized JSON representation.
//!
//! WHAT: A derive macro that adds an inherent `struct_hash()` method to structs.
//!
//! HOW: Uses SHA-256 for hashing and base85 for encoding, consistent with
//! existing `EmbedFileAs`/`EmbedDirectoryAs` macros.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro that generates a `struct_hash()` method.
///
/// The struct must also derive `serde::Serialize` - the macro will not
/// automatically add it, and compilation will fail if it's missing.
pub fn json_hash_derive(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);
    let struct_name = &ast.ident;

    let expanded = quote! {
        impl #struct_name {
            /// Compute a deterministic hash for this struct instance.
            ///
            /// The hash is computed from:
            /// 1. The name of this struct
            /// 2. The serialized JSON representation of this struct
            ///
            /// # Returns
            ///
            /// A base85-encoded SHA-256 hash string.
            ///
            /// # Panics
            ///
            /// Panics if serialization fails. This should not happen
            /// for structs that properly implement `Serialize`.
            pub fn struct_hash(&self) -> String {
                use sha2::{Digest, Sha256};
                use base85rs::encode as base85_encode;

                let type_name = stringify!(#struct_name);
                let json = serde_json::to_string(self)
                    .expect("JsonHash requires Serialize impl");
                let input = format!("{}{}", type_name, json);

                let mut hasher = Sha256::new();
                hasher.update(input.as_bytes());
                let hash = hasher.finalize();

                base85_encode(&hash)
            }
        }
    };

    TokenStream::from(expanded)
}
