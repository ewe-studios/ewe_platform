use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use crate::crate_paths::{foundation_jsonschema_path, serde_json_path};
use crate::schema_fields::extract_struct_fields;

pub fn json_schema_derive(item: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let fields = match extract_struct_fields(&input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let jsonschema = foundation_jsonschema_path();
    let serde = serde_json_path();
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let property_exprs: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let schema = rust_type_to_json_schema(&f.rust_type, &serde);
            quote! { #name: #schema }
        })
        .collect();

    let required_names: Vec<&str> = fields
        .iter()
        .filter(|f| !f.nullable)
        .map(|f| f.name.as_str())
        .collect();

    quote! {
        impl #impl_generics #jsonschema::JsonSchema for #struct_name #ty_generics #where_clause {
            fn json_schema() -> #serde::Value {
                #serde::json!({
                    "type": "object",
                    "properties": {
                        #( #property_exprs ),*
                    },
                    "required": [ #( #required_names ),* ]
                })
            }
        }
    }
}

fn rust_type_to_json_schema(ty: &str, serde: &TokenStream2) -> TokenStream2 {
    match ty {
        "u8" => quote! { #serde::json!({"type": "integer", "format": "uint8"}) },
        "u16" => quote! { #serde::json!({"type": "integer", "format": "uint16"}) },
        "u32" => quote! { #serde::json!({"type": "integer", "format": "uint32"}) },
        "u64" => quote! { #serde::json!({"type": "integer", "format": "uint64"}) },
        "i8" => quote! { #serde::json!({"type": "integer", "format": "int8"}) },
        "i16" => quote! { #serde::json!({"type": "integer", "format": "int16"}) },
        "i32" => quote! { #serde::json!({"type": "integer", "format": "int32"}) },
        "i64" => quote! { #serde::json!({"type": "integer", "format": "int64"}) },
        "f32" => quote! { #serde::json!({"type": "number", "format": "float32"}) },
        "f64" => quote! { #serde::json!({"type": "number", "format": "float64"}) },
        "bool" => quote! { #serde::json!({"type": "boolean"}) },
        "String" => quote! { #serde::json!({"type": "string"}) },
        "Vec<u8>" => quote! { #serde::json!({"type": "string", "format": "binary"}) },
        "SystemTime" => quote! { #serde::json!({"type": "string", "format": "date-time"}) },
        _ => quote! { #serde::json!({"type": "unknown"}) },
    }
}
