use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, DeriveInput, LitStr, Meta, Token};
use uuid::Uuid;

fn foundation_core_path() -> proc_macro2::TokenStream {
    match crate_name("foundation_core") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { #ident }
        }
        Err(_) => quote! { foundation_core },
    }
}

pub fn type_uuid_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).expect("TypeUuid: failed to parse input");
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let mut uuid = None;
    for attr in &ast.attrs {
        if !attr.path().is_ident("uuid") {
            continue;
        }

        let value: LitStr = attr.parse_args().unwrap_or_else(|_| {
            if let Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    return s.clone();
                }
            }
            panic!(
                "uuid attribute must be `#[uuid = \"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx\"]` or `#[uuid(\"...\")]`"
            );
        });

        uuid = Some(
            Uuid::parse_str(&value.value())
                .expect("Value specified to `#[uuid]` attribute is not a valid UUID"),
        );
    }

    let uuid =
        uuid.expect("No `#[uuid = \"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx\"]` attribute found");
    let bytes = uuid.as_bytes().iter().map(|byte| {
        let b = *byte;
        quote! { #b }
    });

    let krate = foundation_core_path();

    let gen = quote! {
        impl #impl_generics #krate::type_uuid::TypeUuid for #name #ty_generics #where_clause {
            const UUID: #krate::type_uuid::Bytes = [
                #( #bytes ),*
            ];
        }
    };
    gen.into()
}

struct ExternalTypeUuidInput {
    path: syn::Path,
    uuid_str: LitStr,
}

impl Parse for ExternalTypeUuidInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        input.parse::<Token![,]>()?;
        let uuid_str = input.parse()?;
        Ok(Self { path, uuid_str })
    }
}

/// Implement `TypeUuid` for an external/foreign type that you can't derive on.
///
/// ```ignore
/// foundation_macros::external_type_uuid!(std::time::Duration, "449a4224-4665-47ce-88a2-8d0310d20572");
/// ```
pub fn external_type_uuid_impl(tokens: TokenStream) -> TokenStream {
    let ExternalTypeUuidInput { path, uuid_str } =
        syn::parse_macro_input!(tokens as ExternalTypeUuidInput);

    let uuid = Uuid::parse_str(&uuid_str.value())
        .expect("Value specified to external_type_uuid is not a valid UUID");

    let bytes = uuid.as_bytes().iter().map(|byte| {
        let b = *byte;
        quote! { #b }
    });

    let krate = foundation_core_path();

    let gen = quote! {
        impl #krate::type_uuid::TypeUuid for #path {
            const UUID: #krate::type_uuid::Bytes = [
                #( #bytes ),*
            ];
        }
    };
    gen.into()
}
