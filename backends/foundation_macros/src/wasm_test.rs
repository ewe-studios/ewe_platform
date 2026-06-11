//! WHY: Feature 13 — the owned wasm test-execution model. Cases must be plain Rust
//! functions that compile into discoverable, runnable exports WITHOUT wasm-bindgen.
//!
//! WHAT: The `#[wasm_test]` attribute body: keeps the original fn, emits a
//! `#[no_mangle] extern "C" fn __fwt_<name>() -> u32` wrapper (0 = reported
//! synchronously, 1 = report arrives async), and contributes a manifest line to the
//! `__fwt_manifest` custom section (`name|flags\n`; flags: `a`sync, `p`anic
//! expected, `i`gnored) that the testbed reads with the owned wasm model.
//!
//! HOW: wasm32 panics abort, so the wrapper installs a `std` panic hook (per-crate,
//! `Once`-guarded) that reports the failure text over `foundation_wasm::testing`
//! BEFORE the trap; the JS runner catches the trap and re-instantiates. Async
//! bodies are driven by `testing::run_async` (the owned `schedule_timeout`
//! re-poll loop). `should_panic` is a manifest flag the RUNNER inverts on — under
//! abort semantics the module cannot observe its own expected panic.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::crate_paths::foundation_wasm_path;

/// Flags parsed from `#[wasm_test(...)]` arguments.
#[derive(Default)]
struct Flags {
    should_panic: bool,
    ignore: bool,
}

fn parse_flags(attr: TokenStream) -> Result<Flags, syn::Error> {
    let mut flags = Flags::default();
    if attr.is_empty() {
        return Ok(flags);
    }
    struct FlagList(syn::punctuated::Punctuated<syn::Ident, syn::Token![,]>);
    impl syn::parse::Parse for FlagList {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            Ok(Self(syn::punctuated::Punctuated::parse_terminated(input)?))
        }
    }
    let FlagList(idents) = syn::parse2::<FlagList>(attr).map_err(|err| {
        syn::Error::new(
            err.span(),
            "#[wasm_test(...)] accepts `should_panic` and/or `ignore`",
        )
    })?;
    for ident in idents {
        match ident.to_string().as_str() {
            "should_panic" => flags.should_panic = true,
            "ignore" => flags.ignore = true,
            other => {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("unknown #[wasm_test] flag `{other}` (expected `should_panic` or `ignore`)"),
                ))
            }
        }
    }
    Ok(flags)
}

pub(crate) fn wasm_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let flags = match parse_flags(attr) {
        Ok(flags) => flags,
        Err(err) => return err.to_compile_error(),
    };
    let case = match syn::parse2::<syn::ItemFn>(item) {
        Ok(case) => case,
        Err(err) => return err.to_compile_error(),
    };
    if !case.sig.inputs.is_empty() {
        return syn::Error::new_spanned(&case.sig.inputs, "#[wasm_test] functions take no arguments")
            .to_compile_error();
    }

    let fw = foundation_wasm_path();
    let name = &case.sig.ident;
    let name_str = name.to_string();
    let is_async = case.sig.asyncness.is_some();
    let export = format_ident!("__fwt_{name}");
    let meta_ident = format_ident!("__FWT_META_{}", name_str.to_uppercase());

    // Manifest line: `name|flags\n` in the `__fwt_manifest` custom section.
    let mut manifest_flags = String::new();
    if is_async {
        manifest_flags.push('a');
    }
    if flags.should_panic {
        manifest_flags.push('p');
    }
    if flags.ignore {
        manifest_flags.push('i');
    }
    let manifest_line = format!("{name_str}|{manifest_flags}\n");
    let manifest_bytes = syn::LitByteStr::new(manifest_line.as_bytes(), name.span());
    let manifest_len = manifest_line.len();

    let ignore = flags.ignore;
    let invoke = if is_async {
        quote! { #fw::testing::run_async(#name_str, #name()) }
    } else {
        quote! {
            #name();
            #fw::testing::pass(#name_str);
            0
        }
    };

    quote! {
        #case

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        #[no_mangle]
        pub extern "C" fn #export() -> u32 {
            #fw::testing::enter_case(#name_str);
            {
                // One panic hook per crate: a wasm32 panic ABORTS, so the failure
                // text must reach the runner BEFORE the trap.
                static __FWT_HOOK: ::std::sync::Once = ::std::sync::Once::new();
                __FWT_HOOK.call_once(|| {
                    ::std::panic::set_hook(::std::boxed::Box::new(|info| {
                        #fw::testing::fail_current(&::std::string::ToString::to_string(info));
                    }));
                });
            }
            if #ignore {
                #fw::testing::ignored(#name_str);
                return 0;
            }
            #invoke
        }

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        #[used]
        #[link_section = "__fwt_manifest"]
        static #meta_ident: [u8; #manifest_len] = *#manifest_bytes;
    }
}
