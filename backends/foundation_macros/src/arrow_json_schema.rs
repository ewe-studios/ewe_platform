use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use crate::crate_paths::{foundation_arrow_path, serde_json_path};
use crate::schema_fields::extract_struct_fields;

pub fn arrow_json_schema_derive(item: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let fields = match extract_struct_fields(&input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let arrow = foundation_arrow_path();
    let serde = serde_json_path();
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let field_exprs: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let nullable = f.nullable;
            let arrow_type = rust_type_to_arrow_type_string(&f.rust_type);
            quote! {
                #serde::json!({
                    "name": #name,
                    "data_type": #arrow_type,
                    "nullable": #nullable
                })
            }
        })
        .collect();

    quote! {
        impl #impl_generics #arrow::ArrowJsonSchema for #struct_name #ty_generics #where_clause {
            fn arrow_json_schema() -> #serde::Value {
                #serde::json!({
                    "fields": [ #( #field_exprs ),* ]
                })
            }
        }
    }
}

fn rust_type_to_arrow_type_string(ty: &str) -> &'static str {
    match ty {
        "u8" => "UInt8",
        "u16" => "UInt16",
        "u32" => "UInt32",
        "u64" => "UInt64",
        "i8" => "Int8",
        "i16" => "Int16",
        "i32" => "Int32",
        "i64" => "Int64",
        "f32" => "Float32",
        "f64" => "Float64",
        "bool" => "Boolean",
        "String" => "Utf8",
        "Vec<u8>" => "Binary",
        "SystemTime" => "Timestamp(Millisecond, UTC)",
        _ => "Unknown",
    }
}
