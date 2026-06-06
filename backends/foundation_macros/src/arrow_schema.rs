use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use crate::crate_paths::foundation_arrow_path;
use crate::schema_fields::extract_struct_fields;

pub fn arrow_schema_derive(item: TokenStream2) -> TokenStream2 {
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

    let field_exprs: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let nullable = f.nullable;
            let datatype = rust_type_to_arrow_datatype(&f.rust_type, &arrow);
            quote! {
                #arrow::arrow_schema::Field::new(#name, #datatype, #nullable)
            }
        })
        .collect();

    quote! {
        impl #impl_generics #arrow::ArrowSchema for #struct_name #ty_generics #where_clause {
            fn fields() -> Vec<#arrow::arrow_schema::Field> {
                vec![
                    #( #field_exprs ),*
                ]
            }

            fn schema() -> #arrow::arrow_schema::Schema {
                #arrow::arrow_schema::Schema::new(Self::fields())
            }
        }
    }
}

fn rust_type_to_arrow_datatype(ty: &str, arrow: &TokenStream2) -> TokenStream2 {
    match ty {
        "u8" => quote! { #arrow::arrow_schema::DataType::UInt8 },
        "u16" => quote! { #arrow::arrow_schema::DataType::UInt16 },
        "u32" => quote! { #arrow::arrow_schema::DataType::UInt32 },
        "u64" => quote! { #arrow::arrow_schema::DataType::UInt64 },
        "i8" => quote! { #arrow::arrow_schema::DataType::Int8 },
        "i16" => quote! { #arrow::arrow_schema::DataType::Int16 },
        "i32" => quote! { #arrow::arrow_schema::DataType::Int32 },
        "i64" => quote! { #arrow::arrow_schema::DataType::Int64 },
        "f32" => quote! { #arrow::arrow_schema::DataType::Float32 },
        "f64" => quote! { #arrow::arrow_schema::DataType::Float64 },
        "bool" => quote! { #arrow::arrow_schema::DataType::Boolean },
        "String" => quote! { #arrow::arrow_schema::DataType::Utf8 },
        "Vec<u8>" => quote! { #arrow::arrow_schema::DataType::Binary },
        "SystemTime" => quote! {
            #arrow::arrow_schema::DataType::Timestamp(
                #arrow::arrow_schema::TimeUnit::Millisecond,
                Some("UTC".into()),
            )
        },
        _ => quote! {
            compile_error!(concat!("unsupported type for ArrowSchema: ", #ty))
        },
    }
}
