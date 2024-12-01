use core::str;
use std::borrow;

pub type IntoStringResult = core::result::Result<String, IntoStringError>;

#[derive(Debug, derive_more::From)]
pub enum IntoStringError {
    Unconvertible,
    InvalidUTF8,
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

impl<'a> IntoString<'a> for borrow::Cow<'a, str> {
    fn into_string(&'a self) -> IntoStringResult {
        Ok(String::from(self.to_owned()))
    }
}

impl<'a> IntoString<'a> for i8 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for i16 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for i32 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for i64 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for u8 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for u16 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for u32 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for u64 {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for usize {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(format!("{}", self)))
    }
}

impl<'a> IntoString<'a> for Vec<u8> {
    fn into_string(&self) -> IntoStringResult {
        Ok(String::from(
            str::from_utf8(self).map_err(|_| IntoStringError::InvalidUTF8)?,
        ))
    }
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

pub type IntoStrResult<'a> = core::result::Result<borrow::Cow<'a, str>, IntoStrError>;

#[derive(Debug, derive_more::From)]
pub enum IntoStrError {
    Unconvertible,
    InvalidUTF8,
    Failed(String),
}

impl core::error::Error for IntoStrError {}

impl core::fmt::Display for IntoStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait IntoStr<'a> {
    fn into_str(self) -> IntoStrResult<'a>;
}

impl<'a> IntoStr<'a> for borrow::Cow<'a, [u8]> {
    fn into_str(self) -> IntoStrResult<'a> {
        match self {
            borrow::Cow::Borrowed(slice) => Ok(borrow::Cow::from(unsafe {
                std::str::from_utf8_unchecked(slice)
            })),
            borrow::Cow::Owned(vec) => Ok(borrow::Cow::from(unsafe {
                String::from_utf8_unchecked(vec)
            })),
        }
    }
}
