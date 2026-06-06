use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

use crate::crate_paths::foundation_arrow_path;
use crate::schema_fields::extract_struct_fields;

pub fn to_arrow_derive(item: TokenStream2) -> TokenStream2 {
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

    let single_array_exprs: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let field_ident = syn::Ident::new(&f.name, proc_macro2::Span::call_site());
            build_single_array_expr(&f.rust_type, f.nullable, &field_ident, &arrow)
        })
        .collect();

    let batch_array_exprs: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let field_ident = syn::Ident::new(&f.name, proc_macro2::Span::call_site());
            build_batch_array_expr(&f.rust_type, f.nullable, &field_ident, &arrow)
        })
        .collect();

    quote! {
        impl #impl_generics #arrow::ToArrow for #struct_name #ty_generics #where_clause {
            fn to_arrow(&self) -> #arrow::ipc::IpcResult<#arrow::arrow_array::RecordBatch> {
                use std::sync::Arc;
                let schema = Arc::new(<Self as #arrow::ArrowSchema>::schema());
                let columns: Vec<#arrow::arrow_array::ArrayRef> = vec![
                    #( #single_array_exprs ),*
                ];
                #arrow::arrow_array::RecordBatch::try_new(schema, columns)
            }

            fn to_arrow_batch(values: &[Self]) -> #arrow::ipc::IpcResult<#arrow::arrow_array::RecordBatch> {
                use std::sync::Arc;
                let schema = Arc::new(<Self as #arrow::ArrowSchema>::schema());
                let columns: Vec<#arrow::arrow_array::ArrayRef> = vec![
                    #( #batch_array_exprs ),*
                ];
                #arrow::arrow_array::RecordBatch::try_new(schema, columns)
            }
        }
    }
}

fn build_single_array_expr(
    rust_type: &str,
    nullable: bool,
    field_ident: &syn::Ident,
    arrow: &TokenStream2,
) -> TokenStream2 {
    if nullable {
        let inner = match rust_type {
            "u8" => quote! { #arrow::arrow_array::UInt8Array::from(vec![Some(*v)]) },
            "u16" => quote! { #arrow::arrow_array::UInt16Array::from(vec![Some(*v)]) },
            "u32" => quote! { #arrow::arrow_array::UInt32Array::from(vec![Some(*v)]) },
            "u64" => quote! { #arrow::arrow_array::UInt64Array::from(vec![Some(*v)]) },
            "i8" => quote! { #arrow::arrow_array::Int8Array::from(vec![Some(*v)]) },
            "i16" => quote! { #arrow::arrow_array::Int16Array::from(vec![Some(*v)]) },
            "i32" => quote! { #arrow::arrow_array::Int32Array::from(vec![Some(*v)]) },
            "i64" => quote! { #arrow::arrow_array::Int64Array::from(vec![Some(*v)]) },
            "f32" => quote! { #arrow::arrow_array::Float32Array::from(vec![Some(*v)]) },
            "f64" => quote! { #arrow::arrow_array::Float64Array::from(vec![Some(*v)]) },
            "bool" => quote! { #arrow::arrow_array::BooleanArray::from(vec![Some(*v)]) },
            "String" => quote! { #arrow::arrow_array::StringArray::from(vec![Some(v.as_str())]) },
            "Vec<u8>" => quote! { #arrow::arrow_array::BinaryArray::from_opt_vec(vec![Some(v.as_slice())]) },
            "SystemTime" => quote! {
                #arrow::arrow_array::TimestampMillisecondArray::from(vec![Some(
                    v.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0),
                )])
            },
            _ => quote! { compile_error!(concat!("unsupported type for ToArrow: ", #rust_type)) },
        };
        let none_expr = match rust_type {
            "String" => quote! { #arrow::arrow_array::StringArray::from(vec![Option::<&str>::None]) },
            "Vec<u8>" => quote! { #arrow::arrow_array::BinaryArray::from_opt_vec(vec![Option::<&[u8]>::None]) },
            "bool" => quote! { #arrow::arrow_array::BooleanArray::from(vec![Option::<bool>::None]) },
            "SystemTime" => quote! { #arrow::arrow_array::TimestampMillisecondArray::from(vec![Option::<i64>::None]) },
            _ => null_typed_array(rust_type, arrow),
        };
        quote! {
            Arc::new(match &self.#field_ident {
                Some(v) => #inner,
                None => #none_expr,
            }) as #arrow::arrow_array::ArrayRef
        }
    } else {
        match rust_type {
            "u8" => quote! { Arc::new(#arrow::arrow_array::UInt8Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "u16" => quote! { Arc::new(#arrow::arrow_array::UInt16Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "u32" => quote! { Arc::new(#arrow::arrow_array::UInt32Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "u64" => quote! { Arc::new(#arrow::arrow_array::UInt64Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "i8" => quote! { Arc::new(#arrow::arrow_array::Int8Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "i16" => quote! { Arc::new(#arrow::arrow_array::Int16Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "i32" => quote! { Arc::new(#arrow::arrow_array::Int32Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "i64" => quote! { Arc::new(#arrow::arrow_array::Int64Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "f32" => quote! { Arc::new(#arrow::arrow_array::Float32Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "f64" => quote! { Arc::new(#arrow::arrow_array::Float64Array::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "bool" => quote! { Arc::new(#arrow::arrow_array::BooleanArray::from(vec![self.#field_ident])) as #arrow::arrow_array::ArrayRef },
            "String" => quote! { Arc::new(#arrow::arrow_array::StringArray::from(vec![self.#field_ident.as_str()])) as #arrow::arrow_array::ArrayRef },
            "Vec<u8>" => quote! { Arc::new(#arrow::arrow_array::BinaryArray::from_vec(vec![self.#field_ident.as_slice()])) as #arrow::arrow_array::ArrayRef },
            "SystemTime" => quote! {
                Arc::new(#arrow::arrow_array::TimestampMillisecondArray::from(vec![
                    self.#field_ident.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0),
                ])) as #arrow::arrow_array::ArrayRef
            },
            _ => quote! { compile_error!(concat!("unsupported type for ToArrow: ", #rust_type)) },
        }
    }
}

fn build_batch_array_expr(
    rust_type: &str,
    nullable: bool,
    field_ident: &syn::Ident,
    arrow: &TokenStream2,
) -> TokenStream2 {
    if nullable {
        match rust_type {
            "u8" => quote! { Arc::new(#arrow::arrow_array::UInt8Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u16" => quote! { Arc::new(#arrow::arrow_array::UInt16Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u32" => quote! { Arc::new(#arrow::arrow_array::UInt32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u64" => quote! { Arc::new(#arrow::arrow_array::UInt64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i8" => quote! { Arc::new(#arrow::arrow_array::Int8Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i16" => quote! { Arc::new(#arrow::arrow_array::Int16Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i32" => quote! { Arc::new(#arrow::arrow_array::Int32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i64" => quote! { Arc::new(#arrow::arrow_array::Int64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "f32" => quote! { Arc::new(#arrow::arrow_array::Float32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "f64" => quote! { Arc::new(#arrow::arrow_array::Float64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "bool" => quote! { Arc::new(#arrow::arrow_array::BooleanArray::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "String" => quote! { Arc::new(#arrow::arrow_array::StringArray::from(values.iter().map(|v| v.#field_ident.as_deref()).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "Vec<u8>" => quote! { Arc::new(#arrow::arrow_array::BinaryArray::from_opt_vec(values.iter().map(|v| v.#field_ident.as_deref()).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "SystemTime" => quote! {
                Arc::new(#arrow::arrow_array::TimestampMillisecondArray::from(values.iter().map(|v| v.#field_ident.as_ref().map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0))).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef
            },
            _ => quote! { compile_error!(concat!("unsupported type for ToArrow batch: ", #rust_type)) },
        }
    } else {
        match rust_type {
            "u8" => quote! { Arc::new(#arrow::arrow_array::UInt8Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u16" => quote! { Arc::new(#arrow::arrow_array::UInt16Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u32" => quote! { Arc::new(#arrow::arrow_array::UInt32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "u64" => quote! { Arc::new(#arrow::arrow_array::UInt64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i8" => quote! { Arc::new(#arrow::arrow_array::Int8Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i16" => quote! { Arc::new(#arrow::arrow_array::Int16Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i32" => quote! { Arc::new(#arrow::arrow_array::Int32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "i64" => quote! { Arc::new(#arrow::arrow_array::Int64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "f32" => quote! { Arc::new(#arrow::arrow_array::Float32Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "f64" => quote! { Arc::new(#arrow::arrow_array::Float64Array::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "bool" => quote! { Arc::new(#arrow::arrow_array::BooleanArray::from(values.iter().map(|v| v.#field_ident).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "String" => quote! { Arc::new(#arrow::arrow_array::StringArray::from(values.iter().map(|v| v.#field_ident.as_str()).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "Vec<u8>" => quote! { Arc::new(#arrow::arrow_array::BinaryArray::from_vec(values.iter().map(|v| v.#field_ident.as_slice()).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef },
            "SystemTime" => quote! {
                Arc::new(#arrow::arrow_array::TimestampMillisecondArray::from(values.iter().map(|v| v.#field_ident.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)).collect::<Vec<_>>())) as #arrow::arrow_array::ArrayRef
            },
            _ => quote! { compile_error!(concat!("unsupported type for ToArrow batch: ", #rust_type)) },
        }
    }
}

fn null_typed_array(rust_type: &str, arrow: &TokenStream2) -> TokenStream2 {
    match rust_type {
        "u8" => quote! { #arrow::arrow_array::UInt8Array::from(vec![Option::<u8>::None]) },
        "u16" => quote! { #arrow::arrow_array::UInt16Array::from(vec![Option::<u16>::None]) },
        "u32" => quote! { #arrow::arrow_array::UInt32Array::from(vec![Option::<u32>::None]) },
        "u64" => quote! { #arrow::arrow_array::UInt64Array::from(vec![Option::<u64>::None]) },
        "i8" => quote! { #arrow::arrow_array::Int8Array::from(vec![Option::<i8>::None]) },
        "i16" => quote! { #arrow::arrow_array::Int16Array::from(vec![Option::<i16>::None]) },
        "i32" => quote! { #arrow::arrow_array::Int32Array::from(vec![Option::<i32>::None]) },
        "i64" => quote! { #arrow::arrow_array::Int64Array::from(vec![Option::<i64>::None]) },
        "f32" => quote! { #arrow::arrow_array::Float32Array::from(vec![Option::<f32>::None]) },
        "f64" => quote! { #arrow::arrow_array::Float64Array::from(vec![Option::<f64>::None]) },
        _ => quote! { compile_error!(concat!("unsupported nullable type: ", #rust_type)) },
    }
}
