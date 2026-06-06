use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

fn resolve_crate(name: &str) -> proc_macro2::TokenStream {
    match crate_name(name) {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(n)) => {
            let ident = syn::Ident::new(&n, proc_macro2::Span::call_site());
            quote! { #ident }
        }
        Err(_) => {
            let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
            quote! { #ident }
        }
    }
}

pub fn foundation_arrow_path() -> proc_macro2::TokenStream {
    resolve_crate("foundation_arrow")
}

pub fn foundation_jsonschema_path() -> proc_macro2::TokenStream {
    resolve_crate("foundation_jsonschema")
}

pub fn serde_json_path() -> proc_macro2::TokenStream {
    resolve_crate("serde_json")
}
