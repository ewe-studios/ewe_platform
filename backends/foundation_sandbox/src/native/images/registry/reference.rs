//! The text form of a registry reference (feature 15, section 6.1).
//!
//! A reference names a registry host, a repository and a tag or a digest. It
//! has the Docker defaults: no host part means Docker Hub, and no tag means
//! `latest`.
//!
//! ```text
//! alpine                                    -> registry-1.docker.io/library/alpine:latest
//! alpine:3.22                               -> registry-1.docker.io/library/alpine:3.22
//! user/app                                  -> registry-1.docker.io/user/app:latest
//! docker.io/library/alpine                  -> registry-1.docker.io/library/alpine:latest
//! ghcr.io/org/app:1                         -> ghcr.io/org/app:1
//! localhost:5000/test/a@sha256:<64 hex>     -> localhost:5000, test/a, digest
//! ```

use std::fmt;

use crate::shared::error::{SandboxError, SandboxResult};

/// The registry host that the Docker Hub names are mapped to.
///
/// The Docker Hub serves its distribution API at `registry-1.docker.io`; the
/// names `docker.io` and `index.docker.io` are aliases of it.
pub const DOCKER_HUB_REGISTRY: &str = "registry-1.docker.io";

/// The registry aliases that name the Docker Hub.
const DOCKER_HUB_ALIASES: [&str; 3] = ["docker.io", "index.docker.io", "registry-1.docker.io"];

/// The repository that a Docker Hub name without a path part uses.
const DOCKER_HUB_LIBRARY: &str = "library";

/// The default tag of a reference with no tag and no digest.
const DEFAULT_TAG: &str = "latest";

/// The most characters in one tag.
const MAX_TAG_CHARS: usize = 128;

/// One registry reference.
///
/// [`OciReference::parse`] builds it. The fields are public so a caller can
/// build a reference of known parts, for example for a test registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OciReference {
    /// The registry host, with its port when it has one.
    pub registry: String,
    /// The repository below the registry, for example `library/alpine`.
    pub repository: String,
    /// The tag or the digest that names one manifest.
    pub reference: TagOrDigest,
}

/// The tag or the digest of one reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagOrDigest {
    /// A tag such as `3.22` or `latest`.
    Tag(String),
    /// A content digest such as `sha256:<64 lower-case hex>`.
    Digest(String),
}

impl TagOrDigest {
    /// The text of this tag or digest, as it appears in the reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tag(tag) => tag,
            Self::Digest(digest) => digest,
        }
    }

    /// The digest text, when the reference names a digest.
    #[must_use]
    pub fn digest(&self) -> Option<&str> {
        match self {
            Self::Tag(_) => None,
            Self::Digest(digest) => Some(digest),
        }
    }
}

impl OciReference {
    /// Parses one reference with the Docker defaults.
    ///
    /// The first path part is a registry host when it has a `.` or a `:` or is
    /// `localhost`. A registry host is reached with `https://` unless the
    /// caller marks it insecure (feature 15, section 4.5).
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] with the field `reference` when the text
    /// is not a valid reference.
    pub fn parse(text: &str) -> SandboxResult<Self> {
        if text.is_empty() {
            return Err(invalid("the reference is empty"));
        }
        let (name, digest) = split_digest(text)?;
        let (name, tag) = split_tag(name)?;
        if digest.is_some() && tag.is_some() {
            return Err(invalid("a reference has a tag or a digest, not both"));
        }
        let (registry, repository) = split_registry(name)?;
        let reference = match digest {
            Some(digest) => TagOrDigest::Digest(digest),
            None => TagOrDigest::Tag(tag.unwrap_or_else(|| DEFAULT_TAG.to_string())),
        };
        Ok(Self {
            registry,
            repository,
            reference,
        })
    }

    /// The tag or the digest text of this reference.
    #[must_use]
    pub fn reference_text(&self) -> &str {
        self.reference.as_str()
    }

    /// The digest of this reference, when it names one.
    #[must_use]
    pub fn digest(&self) -> Option<&str> {
        self.reference.digest()
    }
}

impl fmt::Display for OciReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reference {
            TagOrDigest::Tag(tag) => {
                write!(formatter, "{}/{}:{}", self.registry, self.repository, tag)
            }
            TagOrDigest::Digest(digest) => {
                write!(
                    formatter,
                    "{}/{}@{}",
                    self.registry, self.repository, digest
                )
            }
        }
    }
}

/// Splits the digest part of a reference from its name part.
///
/// A reference has at most one `@`.
fn split_digest(text: &str) -> SandboxResult<(&str, Option<String>)> {
    let mut parts = text.split('@');
    let name = parts.next().unwrap_or_default();
    let digest = parts.next();
    if parts.next().is_some() {
        return Err(invalid("the reference has more than one `@`"));
    }
    if name.is_empty() {
        return Err(invalid("the reference has no repository"));
    }
    match digest {
        None => Ok((name, None)),
        Some(digest) => {
            check_digest(digest)?;
            Ok((name, Some(digest.to_string())))
        }
    }
}

/// Splits the tag from the name part.
///
/// The tag colon is the colon after the last `/`, so a registry port stays in
/// the name.
fn split_tag(name: &str) -> SandboxResult<(&str, Option<String>)> {
    let after_slash = name.rfind('/').map_or(0, |slash| slash + 1);
    let Some(colon) = name[after_slash..].find(':').map(|at| after_slash + at) else {
        if name.is_empty() {
            return Err(invalid("the reference has no repository"));
        }
        return Ok((name, None));
    };
    let (name, tag) = (&name[..colon], &name[colon + 1..]);
    if name.is_empty() {
        return Err(invalid("the reference has no repository"));
    }
    if !valid_tag(tag) {
        return Err(invalid(&format!(
            "the tag is not `[A-Za-z0-9_][A-Za-z0-9._-]` with at most {MAX_TAG_CHARS} characters: {tag}"
        )));
    }
    Ok((name, Some(tag.to_string())))
}

/// Splits the registry host from the repository, with the Docker defaults.
fn split_registry(name: &str) -> SandboxResult<(String, String)> {
    let Some(slash) = name.find('/') else {
        check_repository(&format!("{DOCKER_HUB_LIBRARY}/{name}"))?;
        return Ok((
            DOCKER_HUB_REGISTRY.to_string(),
            format!("{DOCKER_HUB_LIBRARY}/{name}"),
        ));
    };
    let first = &name[..slash];
    let rest = &name[slash + 1..];
    let is_registry = first.contains('.') || first.contains(':') || first == "localhost";
    if is_registry {
        if first.is_empty() {
            return Err(invalid("the registry host is empty"));
        }
        check_repository(rest)?;
        return Ok((map_registry(first), rest.to_string()));
    }
    check_repository(name)?;
    Ok((DOCKER_HUB_REGISTRY.to_string(), name.to_string()))
}

/// Maps a Docker Hub alias to its distribution API host.
fn map_registry(host: &str) -> String {
    if DOCKER_HUB_ALIASES.contains(&host) {
        DOCKER_HUB_REGISTRY.to_string()
    } else {
        host.to_ascii_lowercase()
    }
}

/// Whether a digest is `sha256:` and 64 lower-case hexadecimal digits.
fn check_digest(digest: &str) -> SandboxResult<()> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(invalid(&format!(
            "a digest is `sha256:<64 lower-case hex>`: {digest}"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid(&format!(
            "a digest is `sha256:<64 lower-case hex>`: {digest}"
        )));
    }
    Ok(())
}

/// Whether a tag matches `[A-Za-z0-9_][A-Za-z0-9._-]{0,127}`.
fn valid_tag(tag: &str) -> bool {
    let mut characters = tag.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if tag.chars().count() > MAX_TAG_CHARS {
        return false;
    }
    let first_ok = first.is_ascii_alphanumeric() || first == '_';
    first_ok
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

/// Checks every repository part against `[a-z0-9]+([._-][a-z0-9]+)*`.
fn check_repository(repository: &str) -> SandboxResult<()> {
    if repository.is_empty() {
        return Err(invalid("the repository is empty"));
    }
    for part in repository.split('/') {
        if !valid_repository_part(part) {
            return Err(invalid(&format!(
                "a repository part is `[a-z0-9]+([._-][a-z0-9]+)*`: {part}"
            )));
        }
    }
    Ok(())
}

/// Whether one repository part matches its pattern.
fn valid_repository_part(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    let mut bytes = part.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return false;
    }
    let mut previous_separator = false;
    for byte in bytes {
        let is_separator = matches!(byte, b'.' | b'_' | b'-');
        if is_separator {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else {
            return false;
        }
    }
    !previous_separator
}

/// The error of a bad reference.
fn invalid(reason: &str) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: "reference".to_string(),
        reason: reason.to_string(),
    }
    .into()
}
