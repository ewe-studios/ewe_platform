//! # `#[wasm_ui_server]` (spec-43 phase-1 §6)
//!
//! WHY: A browser test should read like a normal `#[test]` — the server boot,
//! browser launch, navigation, and (critically) deterministic teardown on
//! success/error/panic are ceremony the macro owns.
//!
//! WHAT: the body behind `foundation_macros::wasm_ui_server`. Re-exported by
//! `foundation_browser` so users write `use foundation_browser::wasm_ui_server;`.
//!
//! HOW: wraps the user fn — `fn name(server: &TestServer, page: &Page) ->
//! foundation_browser::Result<()>` — into a `#[test]` that builds a `TestConfig`
//! (from the attribute args), `Harness::setup(..)`, and `harness.run(name, body)`.
//! `Harness::run` is where the catch_unwind + RAII teardown live (spec §0).
//!
//! Attribute args (all optional): `port = N`, `host = "..."`, `headless = bool`,
//! `html = "..."`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::{parse2, Expr, ItemFn, Meta, Token};

/// Expand `#[wasm_ui_server(..)]` over a test fn.
pub fn wasm_ui_server(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = match parse2(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let mut field_inits: Vec<TokenStream> = Vec::new();
    if !attr.is_empty() {
        let metas = match Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr) {
            Ok(m) => m,
            Err(e) => return e.to_compile_error(),
        };
        for meta in metas {
            let Meta::NameValue(nv) = meta else {
                return syn::Error::new_spanned(meta, "expected `key = value`").to_compile_error();
            };
            let key = nv.path.get_ident().map(ToString::to_string).unwrap_or_default();
            let value: &Expr = &nv.value;
            match key.as_str() {
                "port" => field_inits.push(quote! { port: #value }),
                "headless" => field_inits.push(quote! { headless: #value }),
                "host" => field_inits.push(quote! { host: (#value).to_string() }),
                "html" => field_inits.push(quote! { html: (#value).to_string() }),
                "file" => field_inits.push(quote! { file: ::core::option::Option::Some((#value).into()) }),
                "static_dir" => {
                    field_inits.push(quote! { static_dir: ::core::option::Option::Some((#value).into()) });
                }
                "static_mount" => field_inits.push(quote! { static_mount: (#value).to_string() }),
                "encoding" => field_inits.push(quote! {
                    encoding: ::core::str::FromStr::from_str(#value)
                        .expect("wasm_ui_server: invalid encoding (json|columnar)")
                }),
                "headers" => field_inits.push(quote! {
                    headers: ::core::iter::IntoIterator::into_iter(#value)
                        .map(|(k, v)| (::std::string::ToString::to_string(k), ::std::string::ToString::to_string(v)))
                        .collect()
                }),
                other => {
                    return syn::Error::new_spanned(
                        nv.path,
                        format!(
                            "unknown wasm_ui_server arg `{other}` (expected port/host/headless/\
                             html/file/static_dir/static_mount/encoding/headers)"
                        ),
                    )
                    .to_compile_error();
                }
            }
        }
    }

    let name = &func.sig.ident;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;
    let body = &func.block;
    let attrs = &func.attrs;

    quote! {
        #[test]
        #(#attrs)*
        fn #name() {
            // The user's body, verbatim, in an inner fn so `?`/`return` keep
            // their meaning and the params/types are exactly as written.
            fn __wasm_ui_body(#inputs) #output #body

            let __cfg = ::foundation_browser::test::TestConfig {
                #(#field_inits,)*
                ..::core::default::Default::default()
            };
            ::foundation_browser::test::Harness::setup(__cfg)
                .expect("wasm_ui_server: setup failed (server bind or browser launch)")
                .run(stringify!(#name), __wasm_ui_body);
        }
    }
}
