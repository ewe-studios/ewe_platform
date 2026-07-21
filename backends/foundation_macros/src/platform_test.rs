//! #[platform_test] attribute macro — sets up a test with PlatformSession.

use proc_macro::TokenStream;
use quote::quote;

/// Attribute macro that wraps a test function with PlatformSession setup.
///
/// ```ignore
/// #[platform_test]
/// fn my_test(session: Arc<PlatformSession>) {
///     session.route("/app/*", webview_app());
///     let d = session.resolve_route(&intent("/app/items"));
///     assert_eq!(d.source, RouteSource::WebviewApp);
/// }
/// ```
///
/// Expands to a `#[test]` function that creates an `Arc<PlatformSession>`
/// and passes it to the test body.
pub fn platform_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let fn_body = &input.block;
    let vis = &input.vis;

    // Preserve the function signature but inject session creation
    let output = quote! {
        #[test]
        #vis fn #fn_name() {
            let session = ::std::sync::Arc::new(::foundation_platform::PlatformSession::new(::std::path::PathBuf::from(".")));
            // Allow dead code for session if the test doesn't use the arg name
            #[allow(unused)]
            let session = session;
            #fn_body
        }
    };

    output.into()
}
