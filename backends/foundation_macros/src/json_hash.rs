//! `JsonHash` derive macro implementation.
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
    // Generic types are supported: generated provider code has generic
    // envelopes (`ApiEnvelope<T>`), and the hash only needs the value to
    // serialize, which the method's own bound states.
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    // Only a generic type needs the bound spelled out (its parameters decide
    // whether it serializes); a plain struct reports a missing `Serialize` at
    // the `to_string` call, as it always has.
    let serialize_bound = if ast.generics.params.is_empty() {
        quote! {}
    } else {
        quote! { where Self: serde::Serialize, }
    };

    let expanded = quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
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
            pub fn struct_hash(&self) -> String
            #serialize_bound
            {
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
