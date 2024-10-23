pub type IntoStringResult = core::result::Result<String, IntoStringError>;

#[derive(Debug, derive_more::From)]
pub enum IntoStringError {
    Unconvertible,
    Failed(String),
}

impl core::error::Error for IntoStringError {}

impl core::fmt::Display for IntoStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait IntoString<'a> {
    fn into_string(&'a self) -> IntoStringResult;
}

impl<'a> IntoString<'a> for &'a str {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(*self))
    }
}

impl IntoString<'_> for std::path::PathBuf {
    fn into_string(&self) -> IntoStringResult {
        match self.to_str() {
            None => Err(IntoStringError::Unconvertible),
            Some(c) => Ok(String::from(c)),
        }
    }
}
