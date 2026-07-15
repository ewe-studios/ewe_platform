use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::ItemFn;

const MARKER_BROWSER: u8 = 0x01;

pub(crate) fn valtron_bindgen(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_item: ItemFn = match syn::parse(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    let fn_name = &fn_item.sig.ident;
    let marker_name = format_ident!("__bindgen_env_{fn_name}");
    let vis = &fn_item.vis;
    let block = &fn_item.block;
    let output = &fn_item.sig.output;
    let is_async = fn_item.sig.asyncness.is_some();
    let attrs: Vec<_> = fn_item.attrs.iter().filter(|a| !a.path().is_ident("test")).collect();

    let (inner_def, run_call) = if is_async {
        (quote! {}, quote! { foundation_core::valtron::block_on_future(async move #block) })
    } else {
        let inner = format_ident!("__valtron_body_{fn_name}");
        (quote! {
            #[allow(clippy::items_after_statements)]
            fn #inner() #output #block
        }, quote! { #inner() })
    };

    quote! {
        #[link_section = "__wasm_bindgen_test_unstable"]
        #[cfg(target_arch = "wasm32")]
        #[used]
        #[allow(non_upper_case_globals)]
        static #marker_name: [u8; 1] = [#MARKER_BROWSER];

        #[::wasm_bindgen_test::wasm_bindgen_test]
        #(#attrs)*
        #[allow(non_snake_case)]
        #vis fn #fn_name() #output {
            #inner_def
            let __valtron_guard = foundation_core::valtron::initialize_pool(
                ::std::hash::BuildHasher::hash_one(
                    &::std::collections::hash_map::RandomState::new(),
                    0x0045_5745_u64,
                ),
                ::core::option::Option::None,
            );
            let __valtron_out = #run_call;
            ::core::mem::drop(__valtron_guard);
            __valtron_out
        }
    }
    .into()
}
