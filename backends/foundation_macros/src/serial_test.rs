//! WHY: Tests that mutate process-global statics (timer registries,
//! singleton caches) cannot run in parallel — their assertions race.
//! `--test-threads=1` works but serialises EVERY test; we only need to
//! serialise the ones that touch shared state.
//!
//! WHAT: `#[serial_test]` — a `#[test]` replacement that acquires a
//! per-binary `FairGate` before running the body, so tagged tests execute
//! one at a time in FIFO order while untagged tests still run in parallel.
//!
//! HOW: The expansion emits a `static __SERIAL_GATE: FairGate = FairGate::new();`
//! and wraps the body inside `let _guard = __SERIAL_GATE.acquire();`.
//! Because the static is per-binary (not per-function), every
//! `#[serial_test]` in the same test crate shares the same gate.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::crate_paths::foundation_nostd_path;

pub(crate) fn serial_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new_spanned(attr, "#[serial_test] takes no arguments")
            .to_compile_error();
    }
    let func = match syn::parse2::<syn::ItemFn>(item) {
        Ok(func) => func,
        Err(err) => return err.to_compile_error(),
    };
    if func.sig.asyncness.is_some() {
        return syn::Error::new_spanned(
            &func.sig.asyncness,
            "#[serial_test] does not support async functions",
        )
        .to_compile_error();
    }

    let nostd = foundation_nostd_path();

    let attrs: Vec<&syn::Attribute> = func
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("test"))
        .collect();
    let vis = &func.vis;
    let sig = &func.sig;
    let output = &func.sig.output;
    let block = &func.block;
    let inner = format_ident!("__serial_body_{}", func.sig.ident);

    quote! {
        #[test]
        #(#attrs)*
        #vis #sig {
            #[allow(clippy::items_after_statements)]
            fn #inner() #output #block

            let _guard = #nostd::comp::fair_gate::SERIAL_TEST_GATE.acquire();
            #inner()
        }
    }
}
