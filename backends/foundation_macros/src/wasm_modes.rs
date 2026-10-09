//! WHY: Execution modes (decision 014) should be one attribute, not a
//! hand-written `#[wasm_entrypoint]` plus remembered metadata keys — the
//! build pipeline (feature 10) reads these attributes from SOURCE via
//! `CrateScanner`, so the macros' job is compile-time VALIDATION of the
//! convenience syntax (marker-only, like `wasm_entrypoint` itself).
//!
//! WHAT: `#[wasm_bin]`, `#[wasm_worker]`, `#[wasm_service]` — accepted keys:
//!
//! - `js = "single-file" | "separate"` (default `separate`)
//! - `encoded = "b64" | "uint8array"` (single-file only; default `uint8array`)
//! - `desc = "…"` (defaults to the mode + fn name)
//! - `routes = ["/api/…", …]` (`wasm_service` only, REQUIRED there)
//!
//! HOW: Parse + validate the keys, verify the item is a fn, emit it
//! unchanged. The `WasmBundleGenerator` scanner discovers the attribute and
//! its metadata straight from the source text.

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn wasm_bin(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_mode("wasm_bin", false, attr, item, true)
}

pub(crate) fn wasm_worker(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_mode("wasm_worker", false, attr, item, true)
}

pub(crate) fn wasm_service(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_mode("wasm_service", true, attr, item, true)
}

fn expand_mode(
    mode: &str,
    requires_routes: bool,
    attr: TokenStream,
    item: TokenStream,
    default_single_js: bool,
) -> TokenStream {
    let mut func: syn::ItemFn = match syn::parse2(item) {
        Ok(func) => func,
        Err(err) => {
            return syn::Error::new(err.span(), format!("#[{mode}] applies to functions only"))
                .to_compile_error()
        }
    };

    let mut saw_routes = false;
    let mut single_file = false;
    let mut saw_encoded = false;
    let mut extern_c = false;
    let mut jsruntime_single = default_single_js;

    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("js") {
                let value: syn::LitStr = meta.value()?.parse()?;
                match value.value().as_str() {
                    "single-file" => single_file = true,
                    "separate" => single_file = false,
                    other => {
                        return Err(meta.error(format!(
                            "js = \"{other}\" — expected \"single-file\" or \"separate\""
                        )))
                    }
                }
                Ok(())
            } else if meta.path.is_ident("encoded") {
                saw_encoded = true;
                let value: syn::LitStr = meta.value()?.parse()?;
                match value.value().as_str() {
                    "b64" | "uint8array" => Ok(()),
                    other => Err(meta.error(format!(
                        "encoded = \"{other}\" — expected \"b64\" or \"uint8array\""
                    ))),
                }
            } else if meta.path.is_ident("extern") {
                let value: syn::LitStr = meta.value()?.parse()?;
                match value.value().as_str() {
                    "true" => { extern_c = true; Ok(()) }
                    "false" => Ok(()),
                    other => Err(meta.error(format!(
                        "extern = \"{other}\" — expected \"true\" or \"false\""
                    ))),
                }
            } else if meta.path.is_ident("desc") {
                let _: syn::LitStr = meta.value()?.parse()?;
                Ok(())
            } else if meta.path.is_ident("routes") {
                if mode != "wasm_service" {
                    return Err(meta.error("routes = […] is only valid on #[wasm_service]"));
                }
                saw_routes = true;
                let value = meta.value()?;
                let content;
                syn::bracketed!(content in value);
                let _routes =
                    content.parse_terminated(<syn::LitStr as syn::parse::Parse>::parse, syn::Token![,])?;
                Ok(())
            } else if meta.path.is_ident("target") {
                let _: syn::LitStr = meta.value()?.parse()?;
                Ok(())
            } else if meta.path.is_ident("jsruntime_single") {
                let value: syn::LitStr = meta.value()?.parse()?;
                match value.value().as_str() {
                    "true" => { jsruntime_single = true; Ok(()) }
                    "false" => { jsruntime_single = false; Ok(()) }
                    other => Err(meta.error(format!(
                        "jsruntime_single = \"{other}\" — expected \"true\" or \"false\""
                    ))),
                }
            } else {
                Err(meta.error("expected js / encoded / extern / desc / routes / target / jsruntime_single"))
            }
        });
        if let Err(err) = syn::parse::Parser::parse2(parser, attr) {
            return err.to_compile_error();
        }
    }

    if requires_routes && !saw_routes {
        return syn::Error::new(
            func.sig.ident.span(),
            "#[wasm_service] requires routes = [\"/path\", …]",
        )
        .to_compile_error();
    }
    if saw_encoded && !single_file {
        return syn::Error::new(
            func.sig.ident.span(),
            "encoded = … only applies with js = \"single-file\"",
        )
        .to_compile_error();
    }

    // extern = "true": auto-generate #[no_mangle] pub extern "C"
    // Zero boilerplate for wasm export functions.
    if extern_c {
        func.attrs.push(syn::parse_quote!(#[no_mangle]));
        func.vis = syn::parse_quote!(pub);
        func.sig.abi = Some(syn::parse_quote!(extern "C"));
    }

    quote! { #func }
}

// Marker-validation tests live in-file (proc-macro crates can't export hooks).
#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn err(out: &TokenStream) -> String {
        let s = out.to_string();
        assert!(s.contains("compile_error"), "expected an error, got: {s}");
        s
    }

    #[test]
    fn valid_forms_pass_through() {
        let out = wasm_bin(
            quote! { js = "single-file", encoded = "b64" },
            quote! { fn auth_app() {} },
        );
        assert!(out.to_string().contains("fn auth_app"));
        assert!(!out.to_string().contains("compile_error"));

        let out = wasm_worker(quote! {}, quote! { fn data_worker() {} });
        assert!(out.to_string().contains("fn data_worker"));

        let out = wasm_service(
            quote! { routes = ["/api/auth"], js = "single-file" },
            quote! { fn api_service() {} },
        );
        assert!(!out.to_string().contains("compile_error"));
    }

    #[test]
    fn service_requires_routes() {
        let out = err(&wasm_service(quote! {}, quote! { fn s() {} }));
        assert!(out.contains("requires routes"));
    }

    #[test]
    fn encoded_requires_single_file() {
        let out = err(&wasm_bin(quote! { encoded = "b64" }, quote! { fn f() {} }));
        assert!(out.contains("single-file"));
    }

    #[test]
    fn bad_values_rejected() {
        let out = err(&wasm_bin(quote! { js = "inline" }, quote! { fn f() {} }));
        assert!(out.contains("single-file"));
        let out = err(&wasm_worker(quote! { routes = ["/x"] }, quote! { fn f() {} }));
        assert!(out.contains("only valid on"));
    }
}
