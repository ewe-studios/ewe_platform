use rust_embed::{Embed, EmbeddedFile};

use core::str;

use crate::extensions::strings_ext::IntoString;

#[derive(Embed)]
#[folder = "src/megatron/jsrum/packages/"]
#[prefix = "packages/"]
pub struct Packages;

/// [package_request_handler] provides a axum request handler that
/// will serve static files from the [Packages] directory providing
/// a way to use the underlying files from within it as part of
/// your routes.
/// It expects you to provide the name prefix the route will be coming
/// with as that determines how we will swap that name to `packages/`.
///
/// i.e if the path incoming is /public/packer.js then the
/// path will become /packages/packer.js to match the expected
/// embedded path.
pub fn package_request_handler(
    incoming_prefix_name: String,
    req_url: &str,
) -> Option<EmbeddedFile> {
    tracing::info!(
        "[PackageRequestHandler] Received request for path: {}",
        req_url
    );
    let request_path = req_url.into_string();

    Packages::get(
        request_path
            .replace(&incoming_prefix_name, "packages")
            .strip_prefix("/")
            .unwrap_or(req_url),
    )
}
