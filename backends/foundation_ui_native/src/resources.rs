//! Read package resources with the asset service supplied by Swift or Kotlin.
use std::sync::Arc;

use foundation_resources::{
    OpenFuture, ReadConfig, Resource, ResourceError, ResourceName, ResourceReader, Resources,
};

use crate::{CoreError, NativeCore};

/// A single package file. The application host closes each open file.
#[uniffi::export(with_foreign)]
pub trait HostResourceFile: Send + Sync {
    /// Return the next chunk. An empty chunk marks the end.
    ///
    /// # Errors
    /// Returns the operating system read error.
    fn read(&self, capacity: u32) -> Result<Vec<u8>, CoreError>;

    /// Release this file.
    ///
    /// # Errors
    /// Returns the operating system close error.
    fn close_file(&self) -> Result<(), CoreError>;
}

/// The host opens files in its application bundle or asset manager.
#[uniffi::export(with_foreign)]
pub trait HostResourceSource: Send + Sync {
    /// Return an open file, or None when this name does not exist.
    ///
    /// # Errors
    /// Returns the operating system open error.
    fn open_resource_file(
        &self,
        name: String,
    ) -> Result<Option<Arc<dyn HostResourceFile>>, CoreError>;
}

struct File(Option<Arc<dyn HostResourceFile>>);
impl File {
    fn close(&mut self) -> Result<(), CoreError> {
        self.0.take().map_or(Ok(()), |file| file.close_file())
    }
}
impl Drop for File {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::warn!(%error, "Cannot close the package resource");
        }
    }
}

/// Adapts the host package file to a stream with bounded reads.
pub struct MobileReader {
    source: Arc<dyn HostResourceSource>,
    config: ReadConfig,
}
impl MobileReader {
    #[must_use]
    pub fn new(source: Arc<dyn HostResourceSource>, config: ReadConfig) -> Self {
        Self { source, config }
    }
}

impl ResourceReader for MobileReader {
    fn open(&self, name: ResourceName) -> OpenFuture<'_> {
        let source = self.source.clone();
        let capacity = u32::try_from(self.config.chunk_bytes()).unwrap_or(u32::MAX);
        Box::pin(async move {
            let (file, name) = blocking::unblock(move || {
                let file = source
                    .open_resource_file(name.as_str().into())
                    .map_err(|e| read_error(&name, e.to_string()))?
                    .ok_or_else(|| ResourceError::Missing {
                        name: name.as_str().into(),
                    })?;
                Ok((File(Some(file)), name))
            })
            .await?;
            let stream = futures_util::stream::try_unfold(
                (file, name),
                move |(mut file, name)| async move {
                    blocking::unblock(move || {
                        let input = file.0.as_ref().expect("open file");
                        let bytes = input
                            .read(capacity)
                            .map_err(|e| read_error(&name, e.to_string()))?;
                        if bytes.len() > capacity as usize {
                            return Err(read_error(&name, "host exceeded requested chunk size"));
                        }
                        if bytes.is_empty() {
                            file.close().map_err(|e| read_error(&name, e.to_string()))?;
                            return Ok(None);
                        }
                        Ok(Some((bytes, (file, name))))
                    })
                    .await
                },
            );
            Ok(Box::pin(stream) as Resource)
        })
    }
}
fn read_error(name: &ResourceName, error: impl Into<String>) -> ResourceError {
    ResourceError::Read {
        name: name.as_str().into(),
        reason: error.into(),
    }
}

#[uniffi::export]
impl NativeCore {
    /// Install this application's package resource reader before the first action.
    pub fn install_resources(&self, source: Arc<dyn HostResourceSource>) {
        Resources::install(
            &self.graphics_ctx,
            Arc::new(MobileReader::new(source, ReadConfig::default())),
        );
    }
}

/// A package stream that Swift or Kotlin can consume one chunk at a time.
#[derive(uniffi::Object)]
pub struct NativeResourceStream(futures_util::lock::Mutex<Resource>);

#[uniffi::export]
impl NativeResourceStream {
    /// Return the next chunk, or None at the end.
    ///
    /// # Errors
    /// Returns the package read error.
    pub async fn next_chunk(&self) -> Result<Option<Vec<u8>>, NativeResourceError> {
        use futures_util::StreamExt;
        self.0
            .lock()
            .await
            .next()
            .await
            .transpose()
            .map_err(Into::into)
    }
}

/// Resource failures that retain their type across the native language boundary.
#[derive(Debug, uniffi::Error, derive_more::Display, derive_more::Error)]
pub enum NativeResourceError {
    #[display("Invalid resource name: {name}")]
    InvalidName { name: String },
    #[display("The host has no package reader")]
    MissingHost,
    #[display("Resource {name} does not exist")]
    Missing { name: String },
    #[display("Cannot read {name}: {reason}")]
    Read { name: String, reason: String },
}
impl From<ResourceError> for NativeResourceError {
    fn from(error: ResourceError) -> Self {
        match error {
            ResourceError::InvalidName(error) => Self::InvalidName { name: error.name },
            ResourceError::MissingHost => Self::MissingHost,
            ResourceError::Missing { name } => Self::Missing { name },
            ResourceError::Read { name, reason } => Self::Read { name, reason },
        }
    }
}

#[uniffi::export]
impl NativeCore {
    /// Open a named resource through the reader installed by the application.
    ///
    /// # Errors
    /// Returns the shared error for an invalid name, absent host, missing file, or failed open.
    pub async fn open_resource(
        &self,
        name: String,
    ) -> Result<Arc<NativeResourceStream>, NativeResourceError> {
        let stream = Resources::from_context(&self.graphics_ctx)
            .open(&name)
            .await?;
        Ok(Arc::new(NativeResourceStream(
            futures_util::lock::Mutex::new(stream),
        )))
    }
}
