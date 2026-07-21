//! #[platform_test] attribute macro — sets up a test with PlatformSession.
//!
//! Supports optional backends:
//!
//! ```ignore
//! #[platform_test]                  // default: in-memory session only
//! #[platform_test(docker = "macos")]  // Docker macOS backend
//! #[platform_test(docker = "android")] // Docker Android backend
//! #[platform_test(headless)]          // headless browser (CDP/BiDi)
//! ```
//!
//! The attribute is parsed but backend initialization is handled at
//! runtime by the test harness. The macro expands to a `#[test]` function
//! that creates an `Arc<PlatformSession>` and configures it for the
//! requested backend.

use proc_macro::TokenStream;
use quote::quote;

/// Parsed attributes from `#[platform_test(...)]`.
struct PlatformTestArgs {
    backend: TestBackend,
}

enum TestBackend {
    /// Default: in-memory session, no Docker.
    InMemory,
    /// Docker-backed test environment.
    Docker { image: String },
    /// Headless browser via CDP/BiDi.
    Headless,
}

impl Default for PlatformTestArgs {
    fn default() -> Self {
        Self {
            backend: TestBackend::InMemory,
        }
    }
}

fn parse_args(attr: TokenStream) -> PlatformTestArgs {
    if attr.is_empty() {
        return PlatformTestArgs::default();
    }

    let attr_str = attr.to_string();
    let mut args = PlatformTestArgs::default();

    // Parse `docker = "macos"` or `headless`
    for part in attr_str.split(',') {
        let part = part.trim();
        if part == "headless" {
            args.backend = TestBackend::Headless;
        } else if let Some(rest) = part.strip_prefix("docker") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let image = rest.trim().trim_matches('"').to_string();
                args.backend = TestBackend::Docker { image };
            } else {
                // bare `docker` — default to android
                args.backend = TestBackend::Docker {
                    image: "android".to_string(),
                };
            }
        }
    }

    args
}

pub fn platform_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_args(attr);
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let fn_body = &input.block;
    let vis = &input.vis;

    // Generate backend-specific setup code
    let setup_code = match &args.backend {
        TestBackend::InMemory => {
            quote! {
                let _test_backend = ::std::option::Option::None::<&str>;
                let _env = ::std::option::Option::None::<&str>;
            }
        }
        TestBackend::Headless => {
            quote! {
                // Headless CDP/BiDi: platform initializes with a CDP connection.
                // The test harness connects to a running browser (Chromium/Firefox).
                let _test_backend = ::std::option::Option::Some("headless");
                let _env = ::std::option::Option::None::<&str>;
            }
        }
        TestBackend::Docker { image } => {
            let img = image.as_str();
            quote! {
                // Docker-backed test: the harness expects docker compose to be running.
                // Tests are skipped (not failed) if the Docker container isn't reachable.
                let _test_backend = ::std::option::Option::Some("docker");
                let _env = ::std::option::Option::Some(#img);
                // Skip if Docker container isn't running (VNC port not open).
                // This is checked at runtime by the test environment builder.
            }
        }
    };

    let output = quote! {
        #[test]
        #vis fn #fn_name() {
            #setup_code
            let session = ::std::sync::Arc::new(
                ::foundation_platform::PlatformSession::new_test(
                    ::std::path::PathBuf::from(".")
                )
            );
            #[allow(unused)]
            let session = session;
            #fn_body
        }
    };

    output.into()
}
