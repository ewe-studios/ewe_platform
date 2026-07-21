//! #[platform_test] attribute macro — sets up a test with PlatformSession.
//!
//! Supports optional backends:
//!
//! ```ignore
//! #[platform_test]                        // default: in-memory session only
//! #[platform_test(docker = "macos")]      // Docker macOS backend (skips if container not running)
//! #[platform_test(docker = "android")]    // Docker Android backend
//! #[platform_test(docker = "windows")]    // Docker Windows backend
//! #[platform_test(headless)]              // headless browser (CDP/BiDi)
//! ```
//!
//! For Docker backends, the test is silently skipped if the container isn't
//! reachable — no failure, just an early return. This means the same test
//! suite works in CI (containers running) and locally (containers optional).

use proc_macro::TokenStream;
use quote::quote;

struct PlatformTestArgs {
    backend: TestBackend,
}

enum TestBackend {
    InMemory,
    Docker { image: String },
    Headless,
}

impl Default for PlatformTestArgs {
    fn default() -> Self {
        Self { backend: TestBackend::InMemory }
    }
}

fn parse_args(attr: TokenStream) -> PlatformTestArgs {
    if attr.is_empty() {
        return PlatformTestArgs::default();
    }

    let attr_str = attr.to_string();
    let mut args = PlatformTestArgs::default();

    for part in attr_str.split(',') {
        let part = part.trim();
        if part == "headless" {
            args.backend = TestBackend::Headless;
        } else if let Some(rest) = part.strip_prefix("docker") {
            let rest = rest.trim();
            let image = if let Some(rest) = rest.strip_prefix('=') {
                rest.trim().trim_matches('"').to_string()
            } else {
                "android".to_string()
            };
            args.backend = TestBackend::Docker { image };
        }
    }

    args
}

/// Quick check: is a Docker container running?
fn docker_running_check(image: &str) -> String {
    format!(
        "docker ps --filter name={img} --format '{{{{.Status}}}}' 2>/dev/null | grep -q Up",
        img = image
    )
}

pub fn platform_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_args(attr);
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let fn_body = &input.block;
    let vis = &input.vis;

    match &args.backend {
        TestBackend::InMemory | TestBackend::Headless => {
            let output = quote! {
                #[test]
                #vis fn #fn_name() {
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
        TestBackend::Docker { image } => {
            let check = docker_running_check(image);
            let output = quote! {
                #[test]
                #vis fn #fn_name() {
                    // Skip if the Docker container isn't running — no failure,
                    // just an early return. CI runs with containers; local dev
                    // runs the test only when the container is up.
                    let container_running = ::std::process::Command::new("sh")
                        .arg("-c")
                        .arg(#check)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if !container_running {
                        // Docker container not available — silently skip.
                        return;
                    }

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
    }
}
