use rust_embed::Embed;

use core::str;

use crate::extensions::strings_ext::IntoString;

#[derive(Embed)]
#[folder = "$EWE_PLATFORM_DIR/assets"]
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
) -> Option<(Vec<u8>, Option<String>)> {
    tracing::info!(
        "[PackageRequestHandler] Received request for path: {}",
        req_url
    );

    let request_path = req_url.into_string();
    let local_file_path = request_path.replacen(&incoming_prefix_name, "/packages", 1);
    let search_path = local_file_path
        .strip_prefix("/")
        .unwrap_or(local_file_path.as_str());

    tracing::info!(
        "[PackageRequestHandler] Checking for path: {}",
        &search_path,
    );

    Packages::get(search_path).map(|f| (f.data.to_vec(), Some(f.metadata.mimetype().into_string())))
}
