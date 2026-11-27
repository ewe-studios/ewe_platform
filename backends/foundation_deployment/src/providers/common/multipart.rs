//! A `multipart/form-data` request body built from named parts.
//!
//! The generated API clients of a multipart endpoint take this type instead of
//! hand-written boundary text. The Cloudflare Worker upload is the first user:
//! its `metadata` part is JSON and its `files` part is the Worker module.

/// The boundary of a body that names none.
pub const DEFAULT_BOUNDARY: &str = "ewe-platform-boundary-1f3c9a";

/// One part of a `multipart/form-data` body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartPart {
    /// The form field name.
    pub name: String,
    /// The file name, for a file part.
    pub filename: Option<String>,
    /// The media type of this part.
    pub content_type: Option<String>,
    /// The bytes of this part.
    pub data: Vec<u8>,
}

impl MultipartPart {
    /// A part with a name and its bytes.
    #[must_use]
    pub fn new(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            filename: None,
            content_type: None,
            data: data.into(),
        }
    }

    /// The same part with a file name.
    #[must_use]
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// The same part with a media type.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

/// A `multipart/form-data` body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartBody {
    /// The boundary between parts.
    pub boundary: String,
    /// The parts, in wire order.
    pub parts: Vec<MultipartPart>,
}

impl Default for MultipartBody {
    fn default() -> Self {
        Self {
            boundary: DEFAULT_BOUNDARY.to_string(),
            parts: Vec::new(),
        }
    }
}

impl MultipartBody {
    /// An empty body with the default boundary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same body with another boundary.
    #[must_use]
    pub fn with_boundary(mut self, boundary: impl Into<String>) -> Self {
        self.boundary = boundary.into();
        self
    }

    /// The same body with one more part.
    #[must_use]
    pub fn with_part(mut self, part: MultipartPart) -> Self {
        self.parts.push(part);
        self
    }

    /// The `Content-Type` of this body.
    #[must_use]
    pub fn content_type(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }

    /// The `Content-Type` and the encoded bytes of this body.
    #[must_use]
    pub fn encode(&self) -> (String, Vec<u8>) {
        let mut bytes = Vec::new();
        for part in &self.parts {
            bytes.extend_from_slice(b"--");
            bytes.extend_from_slice(self.boundary.as_bytes());
            bytes.extend_from_slice(b"\r\nContent-Disposition: form-data; name=\"");
            bytes.extend_from_slice(part.name.as_bytes());
            bytes.push(b'"');
            if let Some(filename) = &part.filename {
                bytes.extend_from_slice(b"; filename=\"");
                bytes.extend_from_slice(filename.as_bytes());
                bytes.push(b'"');
            }
            bytes.extend_from_slice(b"\r\n");
            if let Some(content_type) = &part.content_type {
                bytes.extend_from_slice(b"Content-Type: ");
                bytes.extend_from_slice(content_type.as_bytes());
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend_from_slice(b"\r\n");
            bytes.extend_from_slice(&part.data);
            bytes.extend_from_slice(b"\r\n");
        }
        bytes.extend_from_slice(b"--");
        bytes.extend_from_slice(self.boundary.as_bytes());
        bytes.extend_from_slice(b"--\r\n");
        (self.content_type(), bytes)
    }
}
