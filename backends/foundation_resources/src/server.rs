//! Serve declared resources from a live public directory or embedded data.
use foundation_http::{
    get,
    native::handlers::{
        static_asset::StaticAssetHandler,
        static_file::{StaticFileConfig, StaticFileHandler},
    },
    shared::router::Router,
    Serve,
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Resolve a public directory next to the current server executable.
///
/// # Errors
/// Returns an error if the executable path or parent is unavailable.
pub fn sibling_public(package: &str) -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| io::Error::other("The executable has no parent directory"))?;
    let adjacent = dir.join("public").join(package);
    if adjacent.try_exists()? {
        return Ok(adjacent);
    }
    // Cargo places test executables in the profile's deps directory or build artifact directory.
    let profile = match dir.file_name().and_then(|name| name.to_str()) {
        Some("deps" | "examples") => dir.parent(),
        Some("out") => dir
            .ancestors()
            .nth(3)
            .filter(|p| p.ends_with("build"))
            .and_then(Path::parent),
        _ => None,
    };
    if let Some(profile) = profile {
        if profile.join(".cargo-lock").try_exists()? {
            return Ok(profile.join("public").join(package));
        }
    }
    Ok(adjacent)
}

/// Register each declared file. The optional index also serves the root route.
///
/// A supplied directory serves live files. Otherwise the embedder provides the bytes.
///
/// # Errors
/// Returns an error for invalid names, missing embedded files, or invalid routes.
pub fn register(
    router: &mut Router<Arc<dyn Serve>>,
    names: &str,
    public: Option<&Path>,
    index: Option<&str>,
    embedded: impl Fn(&str) -> Option<Vec<u8>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for name in names.lines() {
        crate::ResourceName::new(name)?;
        let route = format!("/{name}");
        let aliases = std::iter::once(route.as_str()).chain((index == Some(name)).then_some("/"));
        if let Some(directory) = public {
            for route in aliases {
                router.at(
                    route,
                    get(StaticFileHandler::file(
                        directory.join(name),
                        StaticFileConfig::default(),
                    )),
                )?;
            }
        } else {
            let data: Arc<[u8]> = embedded(name)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Missing packaged resource {name}"),
                    )
                })?
                .into();
            let mime = foundation_netio::shared::mime::from_path(Path::new(name))
                .unwrap_or(foundation_netio::shared::mime::OCTET_STREAM);
            for route in aliases {
                router.at(route, get(StaticAssetHandler::new(data.clone(), mime)))?;
            }
        }
    }
    Ok(())
}

/// Declare the resource set that the package build script stages.
///
/// Give the resulting unit struct to the HTTP static file handler.
#[macro_export]
macro_rules! packaged_resources {
    ($vis:vis $name:ident) => {
        $vis struct $name;
        impl $crate::server::PackagedResourceSet for $name {
            fn lookup(&self, name: &str) -> std::io::Result<Option<$crate::server::PackagedFile>> {
                let index: &[(&str, Option<&'static [u8]>)] =
                    include!(concat!(env!("OUT_DIR"), "/ewe-packaged-resources.rs"));
                let find = |name: &str| {
                    index.binary_search_by(|entry| entry.0.cmp(name))
                        .ok()
                        .map(|position| index[position])
                };
                let asset = option_env!("EWE_BROWSER_ASSET_DIRECTORY")
                    .and_then(|directory| name.strip_prefix(directory))
                    .and_then(|suffix| suffix.strip_prefix('/'))
                    .and_then(find);
                let Some((name, contents)) = asset.or_else(|| find(name)) else {
                    return Ok(None);
                };
                match contents {
                    Some(bytes) => Ok(Some($crate::server::PackagedFile::Embedded(bytes))),
                    None => {
                        let root = $crate::server::sibling_public(env!("CARGO_PKG_NAME"))?;
                        Ok(Some($crate::server::PackagedFile::Live(root.join(name))))
                    }
                }
            }
        }
    };
}

pub use foundation_http::native::handlers::static_file::{PackagedFile, PackagedResourceSet};
