#![allow(clippy::wrong_self_convention)]
#![allow(clippy::needless_lifetimes)]

use core::str;
use std::borrow;

pub type IntoStringResult = core::result::Result<String, TryIntoStringError>;

#[derive(Debug, derive_more::From)]
pub enum TryIntoStringError {
    Unconvertible,
    InvalidUTF8,
    Failed(String),
}

impl core::error::Error for TryIntoStringError {}

impl core::fmt::Display for TryIntoStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait IntoString<'a> {
    fn into_string(&'a self) -> String;
}

pub trait TryIntoString<'a> {
    fn try_into_string(&'a self) -> IntoStringResult;
}

impl<'a, T> IntoString<'a> for T
where
    T: TryIntoString<'a>,
{
    fn into_string(&'a self) -> String {
        self.try_into_string().expect("should convert into string")
    }
}

impl<'a> TryIntoString<'a> for borrow::Cow<'a, str> {
    fn try_into_string(&'a self) -> IntoStringResult {
        Ok(String::from(self.clone()))
    }
}

impl<'a> TryIntoString<'a> for i8 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for i16 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for i32 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for i64 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for u8 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for u16 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for u32 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for u64 {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for usize {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(format!("{}", self))
    }
}

impl<'a> TryIntoString<'a> for Vec<u8> {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(String::from(
            str::from_utf8(self).map_err(|_| TryIntoStringError::InvalidUTF8)?,
        ))
    }
}

impl<'a> TryIntoString<'a> for &'a str {
    fn try_into_string(&self) -> IntoStringResult {
        Ok(String::from(*self))
    }
}

impl TryIntoString<'_> for std::path::PathBuf {
    fn try_into_string(&self) -> IntoStringResult {
        match self.to_str() {
            None => Err(TryIntoStringError::Unconvertible),
            Some(c) => Ok(String::from(c)),
        }
    }
}

pub type TryIntoStrResult<'a> = core::result::Result<borrow::Cow<'a, str>, TryIntoStrError>;

#[derive(Debug, derive_more::From)]
pub enum TryIntoStrError {
    Unconvertible,
    InvalidUTF8,

    #[from(ignore)]
    Failed(String),

    #[from(ignore)]
    FailedFromString(TryIntoStringError),
}

impl core::error::Error for TryIntoStrError {}

impl core::fmt::Display for TryIntoStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait IntoStr<'a> {
    fn into_str(&'a self) -> borrow::Cow<'a, str>;
}

pub trait TryIntoStr<'a> {
    fn try_into_str(&'a self) -> TryIntoStrResult<'a>;
}

impl<'a> TryIntoStr<'a> for borrow::Cow<'a, [u8]> {
    fn try_into_str(&'a self) -> TryIntoStrResult<'a> {
        match self {
            borrow::Cow::Borrowed(slice) => Ok(borrow::Cow::from(unsafe {
                std::str::from_utf8_unchecked(slice)
            })),
            borrow::Cow::Owned(vec) => Ok(borrow::Cow::from(unsafe {
                String::from_utf8_unchecked(vec.to_owned())
            })),
        }
    }
}

impl<'a, T> IntoStr<'a> for T
where
    T: TryIntoString<'a>,
{
    fn into_str(&'a self) -> borrow::Cow<'a, str> {
        let to_string = self.try_into_string().expect("should convert to string");
        borrow::Cow::Owned(to_string)
    }
}

impl<'a, T> TryIntoStr<'a> for T
where
    T: TryIntoString<'a>,
{
    fn try_into_str(&'a self) -> TryIntoStrResult<'a> {
        let to_string = self
            .try_into_string()
            .map_err(TryIntoStrError::FailedFromString)?;
        Ok(borrow::Cow::Owned(to_string))
    }
}
