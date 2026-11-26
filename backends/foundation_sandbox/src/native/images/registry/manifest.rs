//! The manifest document of the distribution protocol (feature 15, section
//! 6.2).
//!
//! One manifest names a configuration blob and the layers. An index names a
//! manifest for each platform. The two documents share one JSON shape here:
//! the fields that a document does not use stay empty.
//!
//! The media types decide whether a layer is a plain tar or a gzip tar. Any
//! other type is an error that names the type; the code never guesses
//! (invariant I8).

use serde::Deserialize;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::spec::GuestArch;

/// The media type of an Open Container Initiative (OCI) image index.
pub const OCI_INDEX: &str = "application/vnd.oci.image.index.v1+json";
/// The media type of a Docker manifest list, the Docker index form.
pub const DOCKER_INDEX: &str = "application/vnd.docker.distribution.manifest.list.v2+json";
/// The media type of an OCI image manifest.
pub const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// The media type of a Docker image manifest.
pub const DOCKER_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
/// The media type of an OCI image configuration.
pub const OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";
/// The media type of a Docker image configuration.
pub const DOCKER_CONFIG: &str = "application/vnd.docker.container.image.v1+json";

/// The media types of an index.
pub const INDEX_MEDIA_TYPES: [&str; 2] = [OCI_INDEX, DOCKER_INDEX];
/// The media types of a manifest.
pub const MANIFEST_MEDIA_TYPES: [&str; 2] = [OCI_MANIFEST, DOCKER_MANIFEST];
/// The media types of a configuration.
pub const CONFIG_MEDIA_TYPES: [&str; 2] = [OCI_CONFIG, DOCKER_CONFIG];

/// The media types that a plain tar layer uses.
pub const PLAIN_LAYER_TYPES: [&str; 2] = [
    "application/vnd.oci.image.layer.v1.tar",
    "application/vnd.docker.image.rootfs.diff.tar",
];

/// The media types that a gzip tar layer uses.
pub const GZIP_LAYER_TYPES: [&str; 2] = [
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.docker.image.rootfs.diff.tar.gzip",
];

/// The `Accept` header of a manifest request: the two index types and the two
/// manifest types.
pub const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, \
     application/vnd.docker.distribution.manifest.list.v2+json, \
     application/vnd.oci.image.manifest.v1+json, \
     application/vnd.docker.distribution.manifest.v2+json";

/// One descriptor: a digest and a size, with its media type.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OciDescriptor {
    /// The media type of the blob.
    #[serde(rename = "mediaType", default)]
    pub media_type: String,
    /// The content digest, `sha256:<64 lower-case hex>`.
    pub digest: String,
    /// The size of the blob in bytes.
    #[serde(default)]
    pub size: u64,
}

/// The platform of one manifest in an index.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OciPlatform {
    /// The architecture word, for example `amd64` or `arm64`.
    #[serde(default)]
    pub architecture: String,
    /// The operating system, for example `linux`.
    #[serde(default)]
    pub os: String,
    /// The architecture variant, for example `v8`.
    #[serde(default)]
    pub variant: Option<String>,
}

/// One manifest entry of an index.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OciIndexEntry {
    /// The media type of the manifest.
    #[serde(rename = "mediaType", default)]
    pub media_type: String,
    /// The digest of the manifest.
    pub digest: String,
    /// The size of the manifest in bytes.
    #[serde(default)]
    pub size: u64,
    /// The platform that the manifest runs on.
    #[serde(default)]
    pub platform: Option<OciPlatform>,
}

/// One manifest or index document.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OciManifestDocument {
    /// The media type that the document names.
    #[serde(rename = "mediaType", default)]
    pub media_type: String,
    /// The configuration blob of a manifest.
    #[serde(default)]
    pub config: Option<OciDescriptor>,
    /// The layers of a manifest, in the order that they apply.
    #[serde(default)]
    pub layers: Vec<OciDescriptor>,
    /// The manifest entries of an index.
    #[serde(default)]
    pub manifests: Vec<OciIndexEntry>,
}

/// What a document is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    /// An index: a list of manifests for platforms.
    Index,
    /// A manifest: a configuration and the layers.
    Manifest,
}

impl OciManifestDocument {
    /// The kind of this document.
    ///
    /// The document's `mediaType` decides. When it is absent, the response
    /// `Content-Type` decides. When both are absent, the fields decide. Any
    /// other media type is an error that names the type (invariant I8).
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] for an unsupported or unknown media type.
    pub fn kind(&self, content_type: Option<&str>) -> SandboxResult<DocumentKind> {
        let media = if self.media_type.is_empty() {
            content_type.unwrap_or_default()
        } else {
            self.media_type.as_str()
        };
        let media = media.split(';').next().unwrap_or_default().trim();
        if INDEX_MEDIA_TYPES.contains(&media) {
            return Ok(DocumentKind::Index);
        }
        if MANIFEST_MEDIA_TYPES.contains(&media) {
            return Ok(DocumentKind::Manifest);
        }
        if !media.is_empty() {
            return Err(unsupported_media("manifest", media));
        }
        if !self.manifests.is_empty() && self.config.is_none() {
            return Ok(DocumentKind::Index);
        }
        if self.config.is_some() {
            return Ok(DocumentKind::Manifest);
        }
        Err(unsupported_media("manifest", "unknown"))
    }
}

/// How the importer reads one layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    /// An uncompressed tar.
    Plain,
    /// A gzip tar.
    Gzip,
}

/// The reader of one layer media type.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] with the unsupported media type (invariant
/// I8).
pub fn layer_kind(media_type: &str) -> SandboxResult<LayerKind> {
    if PLAIN_LAYER_TYPES.contains(&media_type) {
        return Ok(LayerKind::Plain);
    }
    if GZIP_LAYER_TYPES.contains(&media_type) {
        return Ok(LayerKind::Gzip);
    }
    Err(unsupported_media("layer", media_type))
}

/// The image configuration document of one manifest.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OciImageConfigDocument {
    /// The architecture of the programs in the image, for example `amd64`.
    #[serde(default)]
    pub architecture: Option<String>,
    /// The operating system, for example `linux`.
    #[serde(default)]
    pub os: Option<String>,
    /// The container configuration that becomes the image manifest.
    #[serde(default)]
    pub config: Option<foundation_deployment_docker::generated::json::ImageConfig>,
}

/// Chooses the manifest for this host from an index.
///
/// It takes the entry with `os == "linux"` and the guest architecture. An
/// `arm64` entry needs variant `v8` or no variant.
///
/// # Errors
/// [`SandboxError::ImageArchMismatch`] when no entry matches; the message lists
/// the platforms that exist (invariant I7).
pub fn choose_platform<'a>(
    index: &'a OciManifestDocument,
    host: GuestArch,
    image: &str,
) -> SandboxResult<&'a OciIndexEntry> {
    let wanted = match host {
        GuestArch::X86_64 => "amd64",
        GuestArch::Aarch64 => "arm64",
    };
    for entry in &index.manifests {
        let Some(platform) = &entry.platform else {
            continue;
        };
        if !platform.os.eq_ignore_ascii_case("linux")
            || !platform.architecture.eq_ignore_ascii_case(wanted)
        {
            continue;
        }
        if host == GuestArch::Aarch64 {
            let variant_ok = match platform.variant.as_deref() {
                None | Some("" | "v8") => true,
                Some(_) => false,
            };
            if !variant_ok {
                continue;
            }
        }
        return Ok(entry);
    }
    let platforms = platform_list(index);
    Err(SandboxError::ImageArchMismatch {
        image: format!("{image} (platforms: {platforms})"),
        host: host.as_str().to_string(),
    }
    .into())
}

/// Checks the architecture of an image configuration against this host.
///
/// # Errors
/// [`SandboxError::ImageArchMismatch`] when the architecture or the operating
/// system differs (invariant I7).
pub fn check_image_arch(
    config: &OciImageConfigDocument,
    host: GuestArch,
    image: &str,
) -> SandboxResult<()> {
    let architecture = config.architecture.as_deref().unwrap_or("unknown");
    let os = config.os.as_deref().unwrap_or("linux");
    let matches = match host {
        GuestArch::X86_64 => architecture.eq_ignore_ascii_case("amd64"),
        GuestArch::Aarch64 => architecture.eq_ignore_ascii_case("arm64"),
    };
    if matches && os.eq_ignore_ascii_case("linux") {
        Ok(())
    } else {
        Err(SandboxError::ImageArchMismatch {
            image: format!("{image} (platforms: {os}/{architecture})"),
            host: host.as_str().to_string(),
        }
        .into())
    }
}

/// The `os/architecture` text of every index entry.
fn platform_list(index: &OciManifestDocument) -> String {
    if index.manifests.is_empty() {
        return "none".to_string();
    }
    index
        .manifests
        .iter()
        .map(|entry| match &entry.platform {
            Some(platform) => format!("{}/{}", platform.os, platform.architecture),
            None => "unknown".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The error of one unsupported media type.
pub(crate) fn unsupported_media(
    kind: &str,
    media_type: &str,
) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: "mediaType".to_string(),
        reason: format!("the {kind} media type {media_type} is not supported"),
    }
    .into()
}
