use crate::extensions::result_ext::BoxedError;
use derive_more::From;
use std::{
    string::{FromUtf16Error, FromUtf8Error},
    sync::PoisonError,
};

pub type Result<T, E> = std::result::Result<T, E>;

#[derive(From, Debug)]
pub enum HttpReaderError {
    #[from(ignore)]
    InvalidLine(String),

    #[from(ignore)]
    UnknownLine(String),

    #[from(ignore)]
    BodyBuildFailed(BoxedError),

    #[from(ignore)]
    ProtoBuildFailed(BoxedError),

    #[from(ignore)]
    LineReadFailed(BoxedError),

    #[from(ignore)]
    InvalidContentSizeValue(Box<std::num::ParseIntError>),

    ExpectedSizedBodyViaContentLength,
    GuardedResourceAccess,
    SeeTrailerBeforeLastChunk,
    TrailerShouldNotOccurHere,
    OnlyTrailersAreAllowedHere,
    InvalidTailerWithNoValue,
    InvalidChunkSize,
    ReadFailed,
    InvalidHeaderKey,
    InvalidHeaderValueStarter,
    InvalidHeaderValueEnder,
    InvalidHeaderValue,
    HeaderValueContainsEncodedCRLF,
    HeaderKeyContainsEncodedCRLF,
    #[from(ignore)]
    HeaderKeyGreaterThanLimit(usize),
    #[from(ignore)]
    HeaderValueGreaterThanLimit(usize),
    BodyContentSizeIsGreaterThanLimit(usize),
    InvalidHeaderLine,

    #[from(ignore)]
    LimitReached(usize),
}

impl std::error::Error for HttpReaderError {}

impl core::fmt::Display for HttpReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum StringHandlingError {
    Unknown,
    Failed,
}

impl std::error::Error for StringHandlingError {}

impl core::fmt::Display for StringHandlingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum RenderHttpError {
    #[from(ignore)]
    UTF8Error(FromUtf8Error),
    #[from(ignore)]
    UTF16Error(FromUtf16Error),

    #[from(ignore)]
    EncodingError(BoxedError),

    InvalidHttpType,
}

impl std::error::Error for RenderHttpError {}

impl core::fmt::Display for RenderHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum Http11RenderError {
    #[from(ignore)]
    UTF8Encoding(FromUtf8Error),

    #[from(ignore)]
    UTF16Encoding(FromUtf16Error),

    #[from(ignore)]
    Failed(BoxedError),

    HeadersRequired,
    InvalidSituationUsedIterator,
}

impl From<BoxedError> for Http11RenderError {
    fn from(value: BoxedError) -> Self {
        Self::Failed(value)
    }
}

impl From<FromUtf8Error> for Http11RenderError {
    fn from(value: FromUtf8Error) -> Self {
        Self::UTF8Encoding(value)
    }
}

impl From<FromUtf16Error> for Http11RenderError {
    fn from(value: FromUtf16Error) -> Self {
        Self::UTF16Encoding(value)
    }
}

impl std::error::Error for Http11RenderError {}

impl core::fmt::Display for Http11RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type SimpleHttpResult<T> = std::result::Result<T, SimpleHttpError>;

#[derive(From, Debug)]
pub enum SimpleHttpError {
    NoRouteProvided,
    NoBodyProvided,
}

impl std::error::Error for SimpleHttpError {}

impl core::fmt::Display for SimpleHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum ChunkStateError {
    ParseFailed,
    ReadErrors,
    InvalidByte(u8),
    ChunkSizeNotFound,
    InvalidChunkEnding,
    InvalidChunkEndingExpectedCRLF,
    ExtensionWithNoValue,
    InvalidOctetBytes(FromUtf8Error),
}

impl<T> From<PoisonError<T>> for ChunkStateError {
    fn from(_: PoisonError<T>) -> Self {
        Self::ReadErrors
    }
}

impl From<std::io::Error> for ChunkStateError {
    fn from(_: std::io::Error) -> Self {
        Self::ReadErrors
    }
}

impl std::error::Error for ChunkStateError {}

impl core::fmt::Display for ChunkStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum WireErrors {
    ChunkState(ChunkStateError),
    SimpleHttp(SimpleHttpError),
    RenderError(RenderHttpError),
    Http11Render(Http11RenderError),
    HttpReaderError(HttpReaderError),
}
