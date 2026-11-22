//! Open one validated resource through the reader installed by the host.
use crate::ResourceName;
use foundation_context::Ctx;
use futures_util::{future::LocalBoxFuture, stream::LocalBoxStream};
use std::{num::NonZeroUsize, sync::Arc};

/// Failures before or during a streamed resource read.
#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum ResourceError {
    #[display("{_0}")]
    InvalidName(crate::InvalidName),
    #[display("The host has not installed a resource reader")]
    MissingHost,
    #[display("Resource {name} does not exist")]
    Missing { name: String },
    #[display("Cannot read resource {name}: {reason}")]
    Read { name: String, reason: String },
}

impl ResourceError {
    pub(crate) fn read(name: &ResourceName, reason: impl ToString) -> Self {
        Self::Read {
            name: name.as_str().to_owned(),
            reason: reason.to_string(),
        }
    }
    pub(crate) fn missing(name: &ResourceName) -> Self {
        Self::Missing {
            name: name.as_str().to_owned(),
        }
    }
}

/// A stream that reads bytes on demand and can report a later read failure.
pub type Resource = LocalBoxStream<'static, Result<Vec<u8>, ResourceError>>;

/// A host implementation that opens resources by logical name.
pub trait ResourceReader: Send + Sync {
    fn open<'a>(
        &'a self,
        name: ResourceName,
    ) -> LocalBoxFuture<'a, Result<Resource, ResourceError>>;
}

/// Application access to the reader that the host installs.
#[derive(Clone)]
pub struct Resources(Option<Arc<dyn ResourceReader>>);

impl Resources {
    /// Install the host reader in the application context.
    pub fn install(ctx: &Ctx, reader: Arc<dyn ResourceReader>) {
        ctx.store(Self(Some(reader)));
    }

    /// Select the reader from this context.
    #[must_use]
    pub fn from_context(ctx: &Ctx) -> Self {
        ctx.get::<Self>()
            .map_or(Self(None), |value| (*value).clone())
    }

    /// Open a named resource. Each stream item contains the next chunk or an error.
    ///
    /// # Errors
    /// Reports an invalid name, missing reader, missing file, or host read error.
    pub async fn open(&self, name: &str) -> Result<Resource, ResourceError> {
        let valid = ResourceName::new(name).map_err(ResourceError::InvalidName)?;
        self.0
            .as_ref()
            .ok_or(ResourceError::MissingHost)?
            .open(valid)
            .await
    }
}

/// Native transfer settings. The default 8 KiB chunk matches Rust's buffered reader.
#[derive(Clone, Copy, Debug)]
pub struct ReadConfig {
    chunk_bytes: NonZeroUsize,
}
impl Default for ReadConfig {
    fn default() -> Self {
        Self {
            chunk_bytes: NonZeroUsize::new(8192).expect("nonzero default"),
        }
    }
}
impl ReadConfig {
    #[must_use]
    pub fn with_chunk_bytes(mut self, size: NonZeroUsize) -> Self {
        self.chunk_bytes = size;
        self
    }
    #[must_use]
    pub fn chunk_bytes(self) -> usize {
        self.chunk_bytes.get()
    }
}

/// A reader rooted in the package directory supplied by the host.
#[cfg(not(target_family = "wasm"))]
pub struct DirectoryReader {
    root: std::path::PathBuf,
    config: ReadConfig,
}
#[cfg(not(target_family = "wasm"))]
impl DirectoryReader {
    /// Use the supplied public directory or Apple bundle resource directory.
    #[must_use]
    pub fn new(root: std::path::PathBuf, config: ReadConfig) -> Self {
        Self { root, config }
    }
}
#[cfg(not(target_family = "wasm"))]
impl ResourceReader for DirectoryReader {
    fn open<'a>(
        &'a self,
        name: ResourceName,
    ) -> LocalBoxFuture<'a, Result<Resource, ResourceError>> {
        use std::io::Read;
        let root = self.root.clone();
        let cap = self.config.chunk_bytes();
        Box::pin(async move {
            let (file, name) = blocking::unblock(move || {
                let map = |e: std::io::Error| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        ResourceError::missing(&name)
                    } else {
                        ResourceError::read(&name, e)
                    }
                };
                let base = root.canonicalize().map_err(map)?;
                let path = base.join(name.as_str()).canonicalize().map_err(map)?;
                if !path.starts_with(&base) {
                    return Err(ResourceError::read(
                        &name,
                        "resource escapes the package directory",
                    ));
                }
                let file = std::fs::File::open(path).map_err(map)?;
                if !file.metadata().map_err(map)?.is_file() {
                    return Err(ResourceError::missing(&name));
                }
                Ok((file, name))
            })
            .await?;
            let stream = futures_util::stream::try_unfold(
                (file, name),
                move |(mut file, name)| async move {
                    blocking::unblock(move || {
                        let mut data = vec![0; cap];
                        let used = file
                            .read(&mut data)
                            .map_err(|e| ResourceError::read(&name, e))?;
                        data.truncate(used);
                        Ok((used > 0).then_some((data, (file, name))))
                    })
                    .await
                },
            );
            Ok(Box::pin(stream) as Resource)
        })
    }
}
