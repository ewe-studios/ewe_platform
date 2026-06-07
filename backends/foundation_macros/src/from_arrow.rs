use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use crate::crate_paths::foundation_arrow_path;
use crate::schema_fields::extract_struct_fields;

pub fn from_arrow_derive(item: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let fields = match extract_struct_fields(&input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let arrow = foundation_arrow_path();
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let field_extractions: Vec<TokenStream2> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let field_ident = syn::Ident::new(&f.name, proc_macro2::Span::call_site());
            build_field_extraction(&f.rust_type, f.nullable, &field_ident, i, &arrow)
        })
        .collect();

    let field_names: Vec<syn::Ident> = fields
        .iter()
        .map(|f| syn::Ident::new(&f.name, proc_macro2::Span::call_site()))
        .collect();

    let batch_field_extractions: Vec<TokenStream2> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let field_ident = syn::Ident::new(&f.name, proc_macro2::Span::call_site());
            build_batch_field_extraction(&f.rust_type, f.nullable, &field_ident, i, &arrow)
        })
        .collect();

    let field_names2 = field_names.clone();

    quote! {
        impl #impl_generics #arrow::FromArrow for #struct_name #ty_generics #where_clause {
            fn from_arrow(batch: &#arrow::arrow_array::RecordBatch) -> #arrow::ipc::IpcResult<Self> {
                use #arrow::ArrowValue;
                if batch.num_rows() == 0 {
                    return Err(#arrow::arrow_schema::ArrowError::InvalidArgumentError(
                        "empty batch".to_string(),
                    ));
                }
                #( #field_extractions )*
                Ok(Self { #( #field_names ),* })
            }

            fn from_arrow_batch(batch: &#arrow::arrow_array::RecordBatch) -> #arrow::ipc::IpcResult<Vec<Self>> {
                use #arrow::ArrowValue;
                let num_rows = batch.num_rows();
                let mut results = Vec::with_capacity(num_rows);
                for row in 0..num_rows {
                    #( #batch_field_extractions )*
                    results.push(Self { #( #field_names2 ),* });
                }
                Ok(results)
            }
        }
    }
}

fn build_field_extraction(
    rust_type: &str,
    nullable: bool,
    field_ident: &syn::Ident,
    col_idx: usize,
    arrow: &TokenStream2,
) -> TokenStream2 {
    let col_access = quote! { batch.column(#col_idx) };
    let idx = quote! { 0 };
    if nullable {
        value_extraction_optional(rust_type, field_ident, &col_access, arrow, &idx)
    } else {
        value_extraction_required(rust_type, field_ident, &col_access, arrow, &idx)
    }
}

fn build_batch_field_extraction(
    rust_type: &str,
    nullable: bool,
    field_ident: &syn::Ident,
    col_idx: usize,
    arrow: &TokenStream2,
) -> TokenStream2 {
    let col_access = quote! { batch.column(#col_idx) };
    let idx = quote! { row };
    if nullable {
        value_extraction_optional(rust_type, field_ident, &col_access, arrow, &idx)
    } else {
        value_extraction_required(rust_type, field_ident, &col_access, arrow, &idx)
    }
}

fn value_extraction_optional(
    rust_type: &str,
    field_ident: &syn::Ident,
    col_access: &TokenStream2,
    arrow: &TokenStream2,
    idx: &TokenStream2,
) -> TokenStream2 {
    let value_type = arrow_value_type(rust_type);
    quote! {
        let #field_ident: Option<#value_type> = #arrow::ArrowValue::<#value_type>::arrow_value(#col_access, #idx);
    }
}

fn value_extraction_required(
    rust_type: &str,
    field_ident: &syn::Ident,
    col_access: &TokenStream2,
    arrow: &TokenStream2,
    idx: &TokenStream2,
) -> TokenStream2 {
    let value_type = arrow_value_type(rust_type);
    let field_name = field_ident.to_string();
    quote! {
        let #field_ident: #value_type = #arrow::ArrowValue::<#value_type>::arrow_value(#col_access, #idx)
            .ok_or_else(|| #arrow::arrow_schema::ArrowError::InvalidArgumentError(
                format!("null value in non-nullable field '{}'", #field_name),
            ))?;
    }
}

fn arrow_value_type(rust_type: &str) -> TokenStream2 {
    match rust_type {
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "u32" => quote! { u32 },
        "u64" => quote! { u64 },
        "i8" => quote! { i8 },
        "i16" => quote! { i16 },
        "i32" => quote! { i32 },
        "i64" => quote! { i64 },
        "f32" => quote! { f32 },
        "f64" => quote! { f64 },
        "bool" => quote! { bool },
        "String" => quote! { String },
        "Vec<u8>" => quote! { Vec<u8> },
        "SystemTime" => quote! { std::time::SystemTime },
        _ => quote! { compile_error!(concat!("unsupported type for FromArrow: ", #rust_type)) },
    }
}
