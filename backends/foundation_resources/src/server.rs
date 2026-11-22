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
    Ok(std::env::current_exe()?
        .parent()
        .ok_or_else(|| io::Error::other("The executable has no parent directory"))?
        .join("public")
        .join(package))
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
