// region -- JsonError

pub type JsonResult<T> = core::result::Result<T, JsonError>

#[derive(Debug, derive_more::From)]
pub enum JsonError {
    Custom(String),

    PropertyNotFound(String),

    // -- AsType errors
    ValueNotType(&'static str),

    // ToType errors
    NotConvertibleToType(&'static str),

    #[from]
    SerdeJSON(serde_json::Error),
}

// --- region: Custom methods

impl JsonError {
    pub(crate) fn custom<T>(val: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(val.to_string())
    }

    pub(crate) fn into_custom<T>(val: T) -> Self
    where
        T: Into<String>,
    {
        Self::Custom(val.into())
    }
}

// --- end region: Custom methods

// --- region: Error & Display boilerplate

impl std::error::Error for JsonError {}

impl core::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// --- end region: Error & Display boilerplate

// --- end region: JsonError
