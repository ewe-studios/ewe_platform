// region -- JsonError

pub type Result<T> = core::result::Result<T, ValueError>;

#[derive(Debug, derive_more::From)]
pub enum ValueError {
    #[from(ignore)]
    Custom(String),

    #[from(ignore)]
    PropertyNotFound(String),

    // -- AsType errors
    #[from(ignore)]
    ValueNotType(&'static str),

    // ToType errors
    #[from(ignore)]
    NotConvertibleToType(&'static str),
}

// --- region: Custom methods

impl ValueError {
    pub fn custom<T>(val: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(val.to_string())
    }

    pub fn into_custom<T>(val: T) -> Self
    where
        T: Into<String>,
    {
        Self::Custom(val.into())
    }
}

// --- end region: Custom methods

// --- region: Error & Display boilerplate

impl std::error::Error for ValueError {}

impl core::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// --- end region: Error & Display boilerplate

// --- end region: JsonError
