#![allow(clippy::type_complexity)]

use crate::extensions::result_ext::BoxedError;
use crate::extensions::strings_ext::{TryIntoString, TryIntoStringError};
use crate::io::ioutils::{self, ByteBufferPointer, SharedByteBufferStream};
use crate::io::ubytes::{self};
use crate::valtron::{
    BoxedResultIterator, CloneableFn, SendVecIterator, StringBoxedIterator, TransformIterator,
};
use crate::wire::simple_http::errors::{
    ChunkStateError, Http11RenderError, HttpReaderError, LineFeedError, Result, SimpleHttpError,
    SimpleHttpResult, StringHandlingError,
};
use derive_more::From;
use regex::{self, Regex};
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::{
    collections::BTreeMap, convert::Infallible, io::Read, str::FromStr, string::FromUtf8Error,
};

pub type Trailer = String;
pub type Extensions = Vec<(String, Option<String>)>;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub enum ChunkedData {
    Data(Vec<u8>, Option<Extensions>),
    Trailers(Vec<(String, Option<String>)>),
    DataEnded,
}

impl ChunkedData {
    pub fn into_bytes(&mut self) -> Vec<u8> {
        match self {
            ChunkedData::Data(data, exts) => {
                let hexa_octet = format!("{:x}", data.len());
                let extension_string: Option<Vec<String>> = exts.as_mut().map(|extensions| {
                    extensions
                        .iter_mut()
                        .map(|(key, value)| {
                            if value.is_none() {
                                format!("; {key}")
                            } else {
                                let cloned_value = value.clone().unwrap();
                                format!("; {key}=\"{cloned_value}\"")
                            }
                        })
                        .collect()
                });

                let mut chunk_data: Vec<u8> = Vec::new();
                if extension_string.is_some() {
                    chunk_data.append(
                        &mut format!("{} {}", hexa_octet, extension_string.unwrap().join(""))
                            .into_bytes(),
                    );
                } else {
                    chunk_data.extend(hexa_octet.clone().into_bytes());
                }

                chunk_data.append(data);
                chunk_data
            }
            ChunkedData::DataEnded => b"0\r\n".to_vec(),
            ChunkedData::Trailers(trailers) => {
                let content: Vec<String> = trailers
                    .iter()
                    .map(|(key, value)| {
                        if value.is_some() {
                            let v = value.clone().unwrap();
                            format!("{key}:{v}")
                        } else {
                            key.clone()
                        }
                    })
                    .collect();
                content.join(";").into_bytes()
            }
        }
    }
}

pub type ChunkedVecIterator<E> = BoxedResultIterator<ChunkedData, E>;
pub type LineFeedVecIterator<E> = BoxedResultIterator<LineFeed, E>;

pub struct ChunkedDataLimitIterator {
    limit: BodySizeLimit,
    parent: ChunkedVecIterator<BoxedError>,
    collected: AtomicUsize,
    exhausted: AtomicBool,
}

impl ChunkedDataLimitIterator {
    #[must_use]
    pub fn new(limit: BodySizeLimit, parent: ChunkedVecIterator<BoxedError>) -> Self {
        Self {
            limit,
            parent,
            collected: AtomicUsize::new(0),
            exhausted: AtomicBool::new(true),
        }
    }
}

impl Iterator for ChunkedDataLimitIterator {
    type Item = Result<ChunkedData, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted.load(Ordering::SeqCst) {
            return None;
        }

        if self.collected.load(Ordering::Relaxed) > self.limit {
            self.exhausted.store(true, Ordering::Relaxed);
            return Some(Err(Box::new(HttpReaderError::LimitReached(self.limit))));
        }

        match self.parent.next() {
            Some(chunked_result) => match chunked_result {
                Ok(data) => {
                    let chunked_size: usize = match &data {
                        ChunkedData::Data(content, _) => content.len(),
                        ChunkedData::DataEnded => 0,
                        ChunkedData::Trailers(c) => c.len(),
                    };
                    let _ = self.collected.fetch_add(chunked_size, Ordering::SeqCst);
                    Some(Ok(data))
                }
                Err(err) => Some(Err(err)),
            },
            None => None,
        }
    }
}

pub type BodySize = u64;
pub type BodySizeLimit = usize;

#[derive(Clone, Debug)]
pub enum Body {
    /// [`Self::FullBody`] let indicates when you wish to read the full
    /// content of the stream till EOF, if the second option is supplied
    /// then its used to limit the total read amount.
    FullBody(SimpleHeaders, Option<BodySizeLimit>),

    /// [`LimitedBody`] returns a body which is limited by the size and will
    /// attempt to read that exact size via [`Reader::read_exact`].
    LimitedBody(BodySize, SimpleHeaders),

    /// [`ChunkedBody`] returns a chunked body for iterating through
    /// a chunked encoded body of data think transfer encoding style data.
    ChunkedBody(Vec<String>, SimpleHeaders),

    /// [`LineFeedBody`] returns a body reader that iterating through
    /// each line yielding each line to the reader.
    LineFeedBody(SimpleHeaders),
}

pub enum SimpleBody {
    None,
    Text(String),
    Bytes(Vec<u8>),
    Stream(Option<SendVecIterator<BoxedError>>),
    ChunkedStream(Option<ChunkedVecIterator<BoxedError>>),
    LineFeedStream(Option<LineFeedVecIterator<BoxedError>>),
}

impl Eq for SimpleBody {}

// PartialEq is implemented but threads the `Self::Stream` and `Self::ChunkedStream`
// differently in that we do not compare the contents but rather compare that both have
// value of same type (i.e both have provided iterators).
#[allow(clippy::match_like_matches_macro)]
impl PartialEq for SimpleBody {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Text(me), Self::Text(other)) => me == other,
            (Self::Bytes(me), Self::Bytes(other)) => me == other,
            (Self::Stream(me), Self::Stream(other)) => match (me, other) {
                (Some(_this), Some(_that)) => true,
                _ => false,
            },
            (Self::ChunkedStream(me), Self::ChunkedStream(other)) => match (me, other) {
                (Some(_this), Some(_that)) => true,
                _ => false,
            },
            (Self::LineFeedStream(me), Self::LineFeedStream(other)) => match (me, other) {
                (Some(_this), Some(_that)) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl core::fmt::Debug for SimpleBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(dead_code)]
        #[derive(Debug)]
        enum SimpleBodyRepr<'a> {
            None,
            Text(&'a str),
            Bytes(&'a [u8]),
            Stream(Option<()>),
            ChunkedStream(Option<()>),
            LineFeedStream(Option<()>),
        }

        let repr = match self {
            Self::None => SimpleBodyRepr::None,
            Self::Text(inner) => SimpleBodyRepr::Text(inner),
            Self::Bytes(inner) => SimpleBodyRepr::Bytes(inner),
            Self::LineFeedStream(inner) => SimpleBodyRepr::LineFeedStream(match inner {
                Some(_) => Some(()),
                None => None,
            }),
            Self::Stream(inner) => SimpleBodyRepr::Stream(match inner {
                Some(_) => Some(()),
                None => None,
            }),
            Self::ChunkedStream(inner) => SimpleBodyRepr::ChunkedStream(match inner {
                Some(_) => Some(()),
                None => None,
            }),
        };

        repr.fmt(f)
    }
}

impl core::fmt::Display for SimpleBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Text(inner) => write!(f, "Text({inner})"),
            Self::Bytes(inner) => write!(f, "Bytes({inner:?})"),
            Self::LineFeedStream(inner) => match inner {
                Some(_) => write!(f, "LineFeedStream(CloneableIterator<T>)"),
                None => write!(f, "LineFeedStream(None)"),
            },
            Self::Stream(inner) => match inner {
                Some(_) => write!(f, "Stream(CloneableIterator<T>)"),
                None => write!(f, "Stream(None)"),
            },
            Self::ChunkedStream(inner) => match inner {
                Some(_) => write!(f, "ChunkedStream(CloneableIterator<T>)"),
                None => write!(f, "ChunkedStream(None)"),
            },
        }
    }
}

/// `RenderHttp` lets types implement the ability to be rendered into
/// http protocol which makes it easily for more structured types.
#[allow(unused)]
pub trait RenderHttp {
    type Error: From<FromUtf8Error> + From<BoxedError> + 'static;

    fn http_render(
        self,
    ) -> std::result::Result<BoxedResultIterator<Vec<u8>, Self::Error>, Self::Error>
    where
        Self: Sized;

    /// `http_render_encoded_string` attempts to render the results of calling
    /// `RenderHttp::http_render()` as a custom encoded strings.
    fn http_render_encoded_string<E>(
        self,
        encoder: E,
    ) -> std::result::Result<StringBoxedIterator<Self::Error>, Self::Error>
    where
        E: Fn(Result<Vec<u8>, Self::Error>) -> Option<Result<String, Self::Error>> + Send + 'static,
        Self: Sized,
    {
        let render_bytes = self.http_render()?;
        let transformed = TransformIterator::new(Box::new(encoder), render_bytes);
        Ok(Box::new(transformed))
    }

    /// `http_render_utf8_string` attempts to render the results of calling
    /// `RenderHttp::http_render()` as utf8 strings.
    fn http_render_utf8_string(
        self,
    ) -> std::result::Result<StringBoxedIterator<Self::Error>, Self::Error>
    where
        Self: Sized,
    {
        self.http_render_encoded_string(|part_result| match part_result {
            Ok(part) => match String::from_utf8(part) {
                Ok(inner) => Some(Ok(inner)),
                Err(err) => Some(Err(err.into())),
            },
            Err(err) => Some(Err(err)),
        })
    }

    /// allows implementing string representation of the http constructs
    /// as a string. You can override to implement a custom render but by
    /// default it calls `RenderHttp::http_render_utf8_string`.
    fn http_render_string(self) -> std::result::Result<String, Self::Error>
    where
        Self: Sized,
    {
        let mut encoded_content = String::new();
        for part in self.http_render_utf8_string()? {
            match part {
                Ok(inner) => {
                    encoded_content.push_str(&inner);
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(encoded_content)
    }
}

// -- HTTP Artefacts

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Proto {
    HTTP10,
    HTTP11,
    HTTP20,
    HTTP30,
    Custom(String),
}

impl From<String> for Proto {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("should match protocols")
    }
}

impl From<&str> for Proto {
    fn from(value: &str) -> Self {
        Self::from_str(value).expect("should match protocols")
    }
}

impl FromStr for Proto {
    type Err = StringHandlingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        match upper.as_str() {
            "HTTP/1.0" | "HTTP 1.0" | "HTTP10" | "HTTP_10" => Ok(Self::HTTP10),
            "HTTP/1.1" | "HTTP 1.1" | "HTTP11" | "HTTP_11" => Ok(Self::HTTP11),
            "HTTP/2.0" | "HTTP 2.0" | "HTTP20" | "HTTP_20" => Ok(Self::HTTP20),
            "HTTP/3.0" | "HTTP 3.0" | "HTTP30" | "HTTP_30" => Ok(Self::HTTP30),
            _ => Ok(Self::Custom(upper)),
        }
    }
}

impl core::fmt::Display for Proto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HTTP10 => write!(f, "HTTP/1.0"),
            Self::HTTP11 => write!(f, "HTTP/1.1"),
            Self::HTTP20 => write!(f, "HTTP/2.0"),
            Self::HTTP30 => write!(f, "HTTP/3.0"),
            Self::Custom(inner) => write!(f, "{inner:?}"),
        }
    }
}

pub type SimpleHeaders = BTreeMap<SimpleHeader, Vec<String>>;

/// `is_sub_set_of_other_header` returns True if the `SimpleHeaders` is a subset of the
/// other headers in `other`.
#[must_use]
pub fn is_sub_set_of_other_header(this: &SimpleHeaders, other: &SimpleHeaders) -> bool {
    for (key, value) in this {
        match other.get(key) {
            Some(other_value) => {
                if value == other_value {
                    continue;
                }
                return false;
            }
            None => return false,
        }
    }
    true
}

/// HTTP Headers
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SimpleHeader {
    ACCEPT,
    ACCEPT_CHARSET,
    ACCEPT_ENCODING,
    ACCEPT_LANGUAGE,
    ACCEPT_RANGES,
    ACCESS_CONTROL_ALLOW_CREDENTIALS,
    ACCESS_CONTROL_ALLOW_HEADERS,
    ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_EXPOSE_HEADERS,
    ACCESS_CONTROL_MAX_AGE,
    ACCESS_CONTROL_REQUEST_HEADERS,
    ACCESS_CONTROL_REQUEST_METHOD,
    AGE,
    ALLOW,
    ALT_SVC,
    AUTHORIZATION,
    CACHE_CONTROL,
    CACHE_STATUS,
    CDN_CACHE_CONTROL,
    CONNECTION,
    CONTENT_DISPOSITION,
    CONTENT_ENCODING,
    CONTENT_LANGUAGE,
    CONTENT_LENGTH,
    CONTENT_LOCATION,
    CONTENT_RANGE,
    CONTENT_SECURITY_POLICY,
    CONTENT_SECURITY_POLICY_REPORT_ONLY,
    CONTENT_TYPE,
    COOKIE,
    DNT,
    DATE,
    ETAG,
    EXPECT,
    EXPIRES,
    FORWARDED,
    FROM,
    HOST,
    IF_MATCH,
    IF_MODIFIED_SINCE,
    IF_NONE_MATCH,
    IF_RANGE,
    IF_UNMODIFIED_SINCE,
    LAST_MODIFIED,
    KEEP_ALIVE,
    LINK,
    LOCATION,
    MAX_FORWARDS,
    ORIGIN,
    PRAGMA,
    PROXY_AUTHENTICATE,
    PROXY_AUTHORIZATION,
    PUBLIC_KEY_PINS,
    PUBLIC_KEY_PINS_REPORT_ONLY,
    RANGE,
    REFERER,
    REFERRER_POLICY,
    REFRESH,
    RETRY_AFTER,
    SEC_WEBSOCKET_ACCEPT,
    SEC_WEBSOCKET_EXTENSIONS,
    SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_PROTOCOL,
    SEC_WEBSOCKET_VERSION,
    SERVER,
    SET_COOKIE,
    STRICT_TRANSPORT_SECURITY,
    TE,
    TRAILER,
    TRANSFER_ENCODING,
    UPGRADE,
    UPGRADE_INSECURE_REQUESTS,
    USER_AGENT,
    VARY,
    VIA,
    WARNING,
    WWW_AUTHENTICATE,
    X_CONTENT_TYPE_OPTIONS,
    X_DNS_PREFETCH_CONTROL,
    X_FRAME_OPTIONS,
    X_XSS_PROTECTION,
    Custom(String),
}

impl SimpleHeader {
    pub fn custom<S: Into<String>>(value: S) -> Self {
        Self::Custom(value.into())
    }
}

impl From<String> for SimpleHeader {
    fn from(value: String) -> Self {
        let upper = value.to_uppercase();
        match upper.as_str() {
            "KEEP_ALIVE" => Self::KEEP_ALIVE,
            "KEEP-ALIVE" => Self::KEEP_ALIVE,
            "ACCEPT" => Self::ACCEPT,
            "ACCEPT-CHARSET" => Self::ACCEPT_CHARSET,
            "ACCEPT-ENCODING" => Self::ACCEPT_ENCODING,
            "ACCEPT-LANGUAGE" => Self::ACCEPT_LANGUAGE,
            "ACCEPT-RANGES" => Self::ACCEPT_RANGES,
            "ACCESS-CONTROL-ALLOW-CREDENTIALS" => Self::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            "ACCESS-CONTROL-ALLOW-HEADERS" => Self::ACCESS_CONTROL_ALLOW_HEADERS,
            "ACCESS-CONTROL-ALLOW-METHODS" => Self::ACCESS_CONTROL_ALLOW_METHODS,
            "ACCESS-CONTROL-ALLOW-ORIGIN" => Self::ACCESS_CONTROL_ALLOW_ORIGIN,
            "ACCESS-CONTROL-EXPOSE-HEADERS" => Self::ACCESS_CONTROL_EXPOSE_HEADERS,
            "ACCESS-CONTROL-MAX-AGE" => Self::ACCESS_CONTROL_MAX_AGE,
            "ACCESS-CONTROL-REQUEST-HEADERS" => Self::ACCESS_CONTROL_REQUEST_HEADERS,
            "ACCESS-CONTROL-REQUEST-METHOD" => Self::ACCESS_CONTROL_REQUEST_METHOD,
            "AGE" => Self::AGE,
            "ALLOW" => Self::ALLOW,
            "ALT-SVC" => Self::ALT_SVC,
            "AUTHORIZATION" => Self::AUTHORIZATION,
            "CACHE-CONTROL" => Self::CACHE_CONTROL,
            "CACHE-STATUS" => Self::CACHE_STATUS,
            "CDN-CACHE-CONTROL" => Self::CDN_CACHE_CONTROL,
            "CONNECTION" => Self::CONNECTION,
            "CONTENT-DISPOSITION" => Self::CONTENT_DISPOSITION,
            "CONTENT-ENCODING" => Self::CONTENT_ENCODING,
            "CONTENT-LANGUAGE" => Self::CONTENT_LANGUAGE,
            "CONTENT-LENGTH" => Self::CONTENT_LENGTH,
            "CONTENT-LOCATION" => Self::CONTENT_LOCATION,
            "CONTENT-RANGE" => Self::CONTENT_RANGE,
            "CONTENT-SECURITY-POLICY" => Self::CONTENT_SECURITY_POLICY,
            "CONTENT-SECURITY-POLICY-REPORT-ONLY" => Self::CONTENT_SECURITY_POLICY_REPORT_ONLY,
            "CONTENT-TYPE" => Self::CONTENT_TYPE,
            "COOKIE" => Self::COOKIE,
            "DNT" => Self::DNT,
            "DATE" => Self::DATE,
            "ETAG" => Self::ETAG,
            "EXPECT" => Self::EXPECT,
            "EXPIRES" => Self::EXPIRES,
            "FORWARDED" => Self::FORWARDED,
            "FROM" => Self::FROM,
            "HOST" => Self::HOST,
            "IF-MATCH" => Self::IF_MATCH,
            "IF-MODIFIED-SINCE" => Self::IF_MODIFIED_SINCE,
            "IF-NONE-MATCH" => Self::IF_NONE_MATCH,
            "IF-RANGE" => Self::IF_RANGE,
            "IF-UNMODIFIED-SINCE" => Self::IF_UNMODIFIED_SINCE,
            "LAST-MODIFIED" => Self::LAST_MODIFIED,
            "LINK" => Self::LINK,
            "LOCATION" => Self::LOCATION,
            "MAX-FORWARDS" => Self::MAX_FORWARDS,
            "ORIGIN" => Self::ORIGIN,
            "PRAGMA" => Self::PRAGMA,
            "PROXY-AUTHENTICATE" => Self::PROXY_AUTHENTICATE,
            "PROXY-AUTHORIZATION" => Self::PROXY_AUTHORIZATION,
            "PUBLIC-KEY-PINS" => Self::PUBLIC_KEY_PINS,
            "PUBLIC-KEY-PINS-REPORT-ONLY" => Self::PUBLIC_KEY_PINS_REPORT_ONLY,
            "RANGE" => Self::RANGE,
            "REFERER" => Self::REFERER,
            "REFERRER-POLICY" => Self::REFERRER_POLICY,
            "REFRESH" => Self::REFRESH,
            "RETRY-AFTER" => Self::RETRY_AFTER,
            "SEC-WEBSOCKET-ACCEPT" => Self::SEC_WEBSOCKET_ACCEPT,
            "SEC-WEBSOCKET-EXTENSIONS" => Self::SEC_WEBSOCKET_EXTENSIONS,
            "SEC-WEBSOCKET-KEY" => Self::SEC_WEBSOCKET_KEY,
            "SEC-WEBSOCKET-PROTOCOL" => Self::SEC_WEBSOCKET_PROTOCOL,
            "SEC-WEBSOCKET-VERSION" => Self::SEC_WEBSOCKET_VERSION,
            "SERVER" => Self::SERVER,
            "SET-COOKIE" => Self::SET_COOKIE,
            "STRICT-TRANSPORT-SECURITY" => Self::STRICT_TRANSPORT_SECURITY,
            "TE" => Self::TE,
            "TRAILER" => Self::TRAILER,
            "TRANSFER-ENCODING" => Self::TRANSFER_ENCODING,
            "UPGRADE" => Self::UPGRADE,
            "UPGRADE-INSECURE-REQUESTS" => Self::UPGRADE_INSECURE_REQUESTS,
            "USER-AGENT" => Self::USER_AGENT,
            "VARY" => Self::VARY,
            "VIA" => Self::VIA,
            "WARNING" => Self::WARNING,
            "WWW-AUTHENTICATE" => Self::WWW_AUTHENTICATE,
            "X-CONTENT-TYPE-OPTIONS" => Self::X_CONTENT_TYPE_OPTIONS,
            "X-DNS-PREFETCH-CONTROL" => Self::X_DNS_PREFETCH_CONTROL,
            "X-FRAME-OPTIONS" => Self::X_FRAME_OPTIONS,
            "X-XSS-PROTECTION" => Self::X_XSS_PROTECTION,
            _ => Self::Custom(value),
        }
    }
}

impl FromStr for SimpleHeader {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(String::from(s)))
    }
}

impl core::fmt::Display for SimpleHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(inner) => write!(f, "{inner}"),
            Self::ACCEPT => write!(f, "ACCEPT"),
            Self::KEEP_ALIVE => write!(f, "KEEP-ALIVE"),
            Self::ACCEPT_CHARSET => write!(f, "ACCEPT-CHARSET"),
            Self::ACCEPT_ENCODING => write!(f, "ACCEPT-ENCODING"),
            Self::ACCEPT_LANGUAGE => write!(f, "ACCEPT-LANGUAGE"),
            Self::ACCEPT_RANGES => write!(f, "ACCEPT-RANGES"),
            Self::ACCESS_CONTROL_ALLOW_CREDENTIALS => write!(f, "ACCESS-CONTROL-ALLOW-CREDENTIALS"),
            Self::ACCESS_CONTROL_ALLOW_HEADERS => write!(f, "ACCESS-CONTROL-ALLOW-HEADERS"),
            Self::ACCESS_CONTROL_ALLOW_METHODS => write!(f, "ACCESS-CONTROL-ALLOW-METHODS"),
            Self::ACCESS_CONTROL_ALLOW_ORIGIN => write!(f, "ACCESS-CONTROL-ALLOW-ORIGIN"),
            Self::ACCESS_CONTROL_EXPOSE_HEADERS => write!(f, "ACCESS-CONTROL-EXPOSE-HEADERS"),
            Self::ACCESS_CONTROL_MAX_AGE => write!(f, "ACCESS-CONTROL-MAX-AGE"),
            Self::ACCESS_CONTROL_REQUEST_HEADERS => write!(f, "ACCESS-CONTROL-REQUEST-HEADERS"),
            Self::ACCESS_CONTROL_REQUEST_METHOD => write!(f, "ACCESS-CONTROL-REQUEST-METHOD"),
            Self::AGE => write!(f, "AGE"),
            Self::ALLOW => write!(f, "ALLOW"),
            Self::ALT_SVC => write!(f, "ALT-SVC"),
            Self::AUTHORIZATION => write!(f, "AUTHORIZATION"),
            Self::CACHE_CONTROL => write!(f, "CACHE-CONTROL"),
            Self::CACHE_STATUS => write!(f, "CACHE-STATUS"),
            Self::CDN_CACHE_CONTROL => write!(f, "CDN-CACHE-CONTROL"),
            Self::CONNECTION => write!(f, "CONNECTION"),
            Self::CONTENT_DISPOSITION => write!(f, "CONTENT-DISPOSITION"),
            Self::CONTENT_ENCODING => write!(f, "CONTENT-ENCODING"),
            Self::CONTENT_LANGUAGE => write!(f, "CONTENT-LANGUAGE"),
            Self::CONTENT_LENGTH => write!(f, "CONTENT-LENGTH"),
            Self::CONTENT_LOCATION => write!(f, "CONTENT-LOCATION"),
            Self::CONTENT_RANGE => write!(f, "CONTENT-RANGE"),
            Self::CONTENT_SECURITY_POLICY => write!(f, "CONTENT-SECURITY-POLICY"),
            Self::CONTENT_SECURITY_POLICY_REPORT_ONLY => {
                write!(f, "CONTENT-SECURITY-POLICY-REPORT-ONLY")
            }
            Self::CONTENT_TYPE => write!(f, "CONTENT-TYPE"),
            Self::COOKIE => write!(f, "COOKIE"),
            Self::DNT => write!(f, "DNT"),
            Self::DATE => write!(f, "DATE"),
            Self::ETAG => write!(f, "ETAG"),
            Self::EXPECT => write!(f, "EXPECT"),
            Self::EXPIRES => write!(f, "EXPIRES"),
            Self::FORWARDED => write!(f, "FORWARDED"),
            Self::FROM => write!(f, "FROM"),
            Self::HOST => write!(f, "HOST"),
            Self::IF_MATCH => write!(f, "IF-MATCH"),
            Self::IF_MODIFIED_SINCE => write!(f, "IF-MODIFIED-SINCE"),
            Self::IF_NONE_MATCH => write!(f, "IF-NONE-MATCH"),
            Self::IF_RANGE => write!(f, "IF-RANGE"),
            Self::IF_UNMODIFIED_SINCE => write!(f, "IF-UNMODIFIED-SINCE"),
            Self::LAST_MODIFIED => write!(f, "LAST-MODIFIED"),
            Self::LINK => write!(f, "LINK"),
            Self::LOCATION => write!(f, "LOCATION"),
            Self::MAX_FORWARDS => write!(f, "MAX-FORWARDS"),
            Self::ORIGIN => write!(f, "ORIGIN"),
            Self::PRAGMA => write!(f, "PRAGMA"),
            Self::PROXY_AUTHENTICATE => write!(f, "PROXY-AUTHENTICATE"),
            Self::PROXY_AUTHORIZATION => write!(f, "PROXY-AUTHORIZATION"),
            Self::PUBLIC_KEY_PINS => write!(f, "PUBLIC-KEY-PINS"),
            Self::PUBLIC_KEY_PINS_REPORT_ONLY => write!(f, "PUBLIC-KEY-PINS-REPORT-ONLY"),
            Self::RANGE => write!(f, "RANGE"),
            Self::REFERER => write!(f, "REFERER"),
            Self::REFERRER_POLICY => write!(f, "REFERRER-POLICY"),
            Self::REFRESH => write!(f, "REFRESH"),
            Self::RETRY_AFTER => write!(f, "RETRY-AFTER"),
            Self::SEC_WEBSOCKET_ACCEPT => write!(f, "SEC-WEBSOCKET-ACCEPT"),
            Self::SEC_WEBSOCKET_EXTENSIONS => write!(f, "SEC-WEBSOCKET-EXTENSIONS"),
            Self::SEC_WEBSOCKET_KEY => write!(f, "SEC-WEBSOCKET-KEY"),
            Self::SEC_WEBSOCKET_PROTOCOL => write!(f, "SEC-WEBSOCKET-PROTOCOL"),
            Self::SEC_WEBSOCKET_VERSION => write!(f, "SEC-WEBSOCKET-VERSION"),
            Self::SERVER => write!(f, "SERVER"),
            Self::SET_COOKIE => write!(f, "SET-COOKIE"),
            Self::STRICT_TRANSPORT_SECURITY => write!(f, "STRICT-TRANSPORT-SECURITY"),
            Self::TE => write!(f, "TE"),
            Self::TRAILER => write!(f, "TRAILER"),
            Self::TRANSFER_ENCODING => write!(f, "TRANSFER-ENCODING"),
            Self::UPGRADE => write!(f, "UPGRADE"),
            Self::UPGRADE_INSECURE_REQUESTS => write!(f, "UPGRADE-INSECURE-REQUESTS"),
            Self::USER_AGENT => write!(f, "USER-AGENT"),
            Self::VARY => write!(f, "VARY"),
            Self::VIA => write!(f, "VIA"),
            Self::WARNING => write!(f, "WARNING"),
            Self::WWW_AUTHENTICATE => write!(f, "WWW-AUTHENTICATE"),
            Self::X_CONTENT_TYPE_OPTIONS => write!(f, "X-CONTENT-TYPE-OPTIONS"),
            Self::X_DNS_PREFETCH_CONTROL => write!(f, "X-DNS-PREFETCH-CONTROL"),
            Self::X_FRAME_OPTIONS => write!(f, "X-FRAME-OPTIONS"),
            Self::X_XSS_PROTECTION => write!(f, "X-XSS-PROTECTION"),
        }
    }
}

/// HTTP methods
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SimpleMethod {
    HEAD,
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    OPTIONS,
    CONNECT,
    TRACE,
    Custom(String),
}

impl core::fmt::Display for SimpleMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl From<&str> for SimpleMethod {
    fn from(value: &str) -> Self {
        match value {
            "HEAD" => Self::HEAD,
            "CONNECT" => Self::CONNECT,
            "TRACE" => Self::TRACE,
            "GET" => Self::GET,
            "POST" => Self::POST,
            "PUT" => Self::PUT,
            "DELETE" => Self::DELETE,
            "PATCH" => Self::PATCH,
            "OPTION" | "OPTIONS" => Self::OPTIONS,
            _ => Self::Custom(value.into()),
        }
    }
}

impl From<String> for SimpleMethod {
    fn from(value: String) -> Self {
        match value.to_uppercase().as_str() {
            "GET" => Self::GET,
            "HEAD" => Self::HEAD,
            "POST" => Self::POST,
            "PUT" => Self::PUT,
            "DELETE" => Self::DELETE,
            "PATCH" => Self::PATCH,
            "TRACE" => Self::TRACE,
            "CONNECT" => Self::CONNECT,
            "OPTION" | "OPTIONS" => Self::OPTIONS,
            _ => Self::Custom(value),
        }
    }
}

impl SimpleMethod {
    fn value(&self) -> String {
        match self {
            SimpleMethod::HEAD => "HEAD".into(),
            SimpleMethod::GET => "GET".into(),
            SimpleMethod::POST => "POST".into(),
            SimpleMethod::PUT => "PUT".into(),
            SimpleMethod::DELETE => "DELETE".into(),
            SimpleMethod::PATCH => "PATCH".into(),
            SimpleMethod::OPTIONS => "OPTIONS".into(),
            SimpleMethod::CONNECT => "CONNECT".into(),
            SimpleMethod::TRACE => "TRACE".into(),
            SimpleMethod::Custom(inner) => inner.clone(),
        }
    }

    /// compares with string equivalent
    #[must_use]
    pub fn equal(&self, value: &str) -> bool {
        self.value() == value
    }
}

/// HTTP status
///
/// Can be converted to its numeral equivalent.
#[derive(Debug, Eq, PartialEq, PartialOrd, Clone)]
#[repr(u64)]
pub enum Status {
    Continue = 100,
    SwitchingProtocols = 101,
    Processing = 102,
    OK = 200,
    Created = 201,
    Accepted = 202,
    NonAuthoritativeInformation = 203,
    NoContent = 204,
    ResetContent = 205,
    PartialContent = 206,
    MultiStatus = 207,
    MultipleChoices = 300,
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    UseProxy = 305,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Conflict = 409,
    Gone = 410,
    LengthRequired = 411,
    PreconditionFailed = 412,
    PayloadTooLarge = 413,
    UriTooLong = 414,
    UnsupportedMediaType = 415,
    RangeNotSatisfiable = 416,
    ExpectationFailed = 417,
    ImATeapot = 418,
    UnprocessableEntity = 422,
    Locked = 423,
    FailedDependency = 424,
    UpgradeRequired = 426,
    PreconditionRequired = 428,
    TooManyRequests = 429,
    RequestHeaderFieldsTooLarge = 431,
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    HttpVersionNotSupported = 505,
    InsufficientStorage = 507,
    NetworkAuthenticationRequired = 511,
    Numbered(usize, String),
    Text(String),
}

#[allow(clippy::recursive_format_impl)]
impl core::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Numbered(code, desc) => write!(f, "{code:} {desc:}"),
            Self::Text(code) => write!(f, "{code:}"),
            _ => write!(f, "{self:}"),
        }
    }
}

impl From<String> for Status {
    fn from(value: String) -> Self {
        let values: Vec<&str> = value.split(' ').collect();

        let target = if values.len() > 1 {
            values[0]
        } else {
            value.as_str()
        };

        match target.parse::<usize>() {
            Ok(inner) => match inner {
                100 => Self::Continue,
                101 => Self::SwitchingProtocols,
                102 => Self::Processing,
                200 => Self::OK,
                201 => Self::Created,
                202 => Self::Accepted,
                203 => Self::NonAuthoritativeInformation,
                204 => Self::NoContent,
                205 => Self::ResetContent,
                206 => Self::PartialContent,
                207 => Self::MultiStatus,
                300 => Self::MultipleChoices,
                301 => Self::MovedPermanently,
                302 => Self::Found,
                303 => Self::SeeOther,
                304 => Self::NotModified,
                305 => Self::UseProxy,
                307 => Self::TemporaryRedirect,
                308 => Self::PermanentRedirect,
                400 => Self::BadRequest,
                401 => Self::Unauthorized,
                402 => Self::PaymentRequired,
                403 => Self::Forbidden,
                404 => Self::NotFound,
                405 => Self::MethodNotAllowed,
                406 => Self::NotAcceptable,
                407 => Self::ProxyAuthenticationRequired,
                408 => Self::RequestTimeout,
                409 => Self::Conflict,
                410 => Self::Gone,
                411 => Self::LengthRequired,
                412 => Self::PreconditionFailed,
                413 => Self::PayloadTooLarge,
                414 => Self::UriTooLong,
                415 => Self::UnsupportedMediaType,
                416 => Self::RangeNotSatisfiable,
                417 => Self::ExpectationFailed,
                418 => Self::ImATeapot,
                422 => Self::UnprocessableEntity,
                423 => Self::Locked,
                424 => Self::FailedDependency,
                426 => Self::UpgradeRequired,
                428 => Self::PreconditionRequired,
                429 => Self::TooManyRequests,
                431 => Self::RequestHeaderFieldsTooLarge,
                500 => Self::InternalServerError,
                501 => Self::NotImplemented,
                502 => Self::BadGateway,
                503 => Self::ServiceUnavailable,
                504 => Self::GatewayTimeout,
                505 => Self::HttpVersionNotSupported,
                507 => Self::InsufficientStorage,
                511 => Self::NetworkAuthenticationRequired,
                _ => Self::Numbered(inner, value),
            },
            Err(err) => {
                tracing::error!(
                    "Failed to convert string to Status: {:?} -> {:?}",
                    &value,
                    err
                );
                Self::Text(value)
            }
        }
    }
}

impl Status {
    /// Returns status' full description
    #[must_use]
    pub fn status_line(&self) -> String {
        match self {
            Self::Continue => "100 Continue".into(),
            Self::SwitchingProtocols => "101 Switching Protocols".into(),
            Self::Processing => "102 Processing".into(),
            Self::OK => "200 Ok".into(),
            Self::Created => "201 Created".into(),
            Self::Accepted => "202 Accepted".into(),
            Self::NonAuthoritativeInformation => "203 Non Authoritative Information".into(),
            Self::NoContent => "204 No Content".into(),
            Self::ResetContent => "205 Reset Content".into(),
            Self::PartialContent => "206 Partial Content".into(),
            Self::MultiStatus => "207 Multi Status".into(),
            Self::MultipleChoices => "300 Multiple Choices".into(),
            Self::MovedPermanently => "301 Moved Permanently".into(),
            Self::Found => "302 Found".into(),
            Self::SeeOther => "303 See Other".into(),
            Self::NotModified => "304 Not Modified".into(),
            Self::UseProxy => "305 Use Proxy".into(),
            Self::TemporaryRedirect => "307 Temporary Redirect".into(),
            Self::PermanentRedirect => "308 Permanent Redirect".into(),
            Self::BadRequest => "400 Bad Request".into(),
            Self::Unauthorized => "401 Unauthorized".into(),
            Self::PaymentRequired => "402 Payment Required".into(),
            Self::Forbidden => "403 Forbidden".into(),
            Self::NotFound => "404 Not Found".into(),
            Self::MethodNotAllowed => "405 Method Not Allowed".into(),
            Self::NotAcceptable => "406 Not Acceptable".into(),
            Self::ProxyAuthenticationRequired => "407 Proxy Authentication Required".into(),
            Self::RequestTimeout => "408 Request Timeout".into(),
            Self::Conflict => "409 Conflict".into(),
            Self::Gone => "410 Gone".into(),
            Self::LengthRequired => "411 Length Required".into(),
            Self::PreconditionFailed => "412 Precondition Failed".into(),
            Self::PayloadTooLarge => "413 Payload Too Large".into(),
            Self::UriTooLong => "414 URI Too Long".into(),
            Self::UnsupportedMediaType => "415 Unsupported Media Type".into(),
            Self::RangeNotSatisfiable => "416 Range Not Satisfiable".into(),
            Self::ExpectationFailed => "417 Expectation Failed".into(),
            Self::ImATeapot => "418 I'm A Teapot".into(),
            Self::UnprocessableEntity => "422 Unprocessable Entity".into(),
            Self::Locked => "423 Locked".into(),
            Self::FailedDependency => "424 Failed Dependency".into(),
            Self::UpgradeRequired => "426 Upgrade Required".into(),
            Self::PreconditionRequired => "428 Precondition Required".into(),
            Self::TooManyRequests => "429 Too Many Requests".into(),
            Self::RequestHeaderFieldsTooLarge => "431 Request Header Fields Too Large".into(),
            Self::InternalServerError => "500 Internal Server Error".into(),
            Self::NotImplemented => "501 Not Implemented".into(),
            Self::BadGateway => "502 Bad Gateway".into(),
            Self::ServiceUnavailable => "503 Service Unavailable".into(),
            Self::GatewayTimeout => "504 Gateway Timeout".into(),
            Self::HttpVersionNotSupported => "505 Http Version Not Supported".into(),
            Self::InsufficientStorage => "507 Insufficient Storage".into(),
            Self::NetworkAuthenticationRequired => "511 Network Authentication Required".into(),
            Self::Numbered(code, description) => format!("{code} {description}"),
            Self::Text(description) => description.clone(),
        }
    }
}

/// `ActUrl` represents a url string and query parameters hashmap
#[derive(Clone, Debug)]
pub struct SimpleUrl {
    pub url: String,
    pub url_only: bool,
    pub matcher: Option<regex::Regex>,
    pub params: Option<Vec<String>>,
    pub queries: Option<BTreeMap<String, String>>,
}

impl Eq for SimpleUrl {}

impl PartialEq for SimpleUrl {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

static CAPTURE_QUERY: &str = r"\?.*";
static CAPTURE_PATH: &str = r".*\?";
static QUERY_REPLACER: &str = r"(?P<$p>[^//|/?]+)";
static CAPTURE_PARAM_STR: &str = r"\{(?P<p>([A-z|0-9|_])+)\}";
static CAPTURE_QUERY_KEY_VALUE: &str = r"((?P<qk>[^&]+)=(?P<qv>[^&]+))*";

#[allow(unused)]
impl SimpleUrl {
    pub fn new(
        url_only: bool,
        request_url: String,
        matcher: regex::Regex,
        params: Vec<String>,
        query: BTreeMap<String, String>,
    ) -> SimpleUrl {
        Self {
            url_only,
            url: request_url,
            queries: Some(query),
            params: Some(params),
            matcher: Some(matcher),
        }
    }

    /// `url_only` indicates you wish to represent a URL only where the Url
    /// will not have queries or parameters to be extracted.
    /// Generally you will use this on the server side when representing
    /// a request with no queries or parameters.
    pub fn url_only<S: Into<String>>(request_url: S) -> SimpleUrl {
        Self {
            url: request_url.into(),
            url_only: true,
            matcher: None,
            queries: None,
            params: None,
        }
    }

    /// `url_with_query` is used when parsing a url with queries
    /// e.g service.com/path/{param1}/{param2}?key=value&..
    /// this will extract these out into the `SimpleUrl` constructs.
    ///
    /// This is the method to use when constructing your `ServiceAction`
    /// has it lets you match against specific paths, queries and parameters.
    ///
    /// A unique thing to note is the query part of a url (?key=value&..)
    /// will be extracted and matched against the url when checking
    /// both `SimpleURL::match_url` and `SimpleURL::extract_matched_url`
    /// this means the matched URL must match the queries as well except in
    /// the cases where the value part of your query `key={value}` is a `*`
    /// which allows you to match any with the condition the key is present.
    pub fn url_with_query<S: Into<String>>(request_url: S) -> SimpleUrl {
        let request_url_str = request_url.into();
        let params = Self::capture_url_params(&request_url_str);
        let matcher = Self::capture_path_pattern(&request_url_str);
        let queries = Self::capture_query_hashmap(&request_url_str);
        SimpleUrl {
            params,
            queries,
            url_only: false,
            url: request_url_str,
            matcher: Some(matcher),
        }
    }

    #[must_use]
    pub fn extract_matched_url(&self, target: &str) -> (bool, Option<BTreeMap<String, String>>) {
        let (matched_uri_regex, params): (bool, Option<BTreeMap<String, String>>) =
            match &self.matcher {
                Some(inner) => {
                    if inner.is_match(target) {
                        let extracted_params: Vec<String> = inner
                            .captures_iter(target)
                            .flat_map(|cap| {
                                let mut captures: Vec<String> = Vec::new();

                                // since the 0 index is always the full string
                                // then start capture from index 1.
                                for index in (1..cap.len()) {
                                    if let Some(item) = cap.get(index) {
                                        captures.push(String::from(item.as_str()));
                                        continue;
                                    }
                                    break;
                                }

                                captures
                            })
                            .collect();

                        if self.params.is_none() {
                            (true, None)
                        } else {
                            match self.merge_params(extracted_params) {
                                Some(params) => (true, Some(params)),
                                None => (false, None),
                            }
                        }
                    } else {
                        (false, None)
                    }
                }
                None => (self.url == target, None),
            };

        if self.url_only {
            return (matched_uri_regex, None);
        }

        if matched_uri_regex {
            return (self.match_queries(target), params);
        }

        (false, params)
    }

    fn merge_params(&self, extracted: Vec<String>) -> Option<BTreeMap<String, String>> {
        match &self.params {
            Some(inner) => {
                if inner.len() != extracted.len() {
                    return None;
                }

                let mut items: BTreeMap<String, String> = BTreeMap::new();
                for index in (0..inner.len()) {
                    let key = inner[index].clone();
                    let value = extracted[index].clone();
                    items.insert(key, value);
                }
                Some(items)
            }
            None => None,
        }
    }

    #[must_use]
    pub fn matches_other(&self, target: &SimpleUrl) -> bool {
        let matched_uri_regex = match &self.matcher {
            Some(inner) => inner.is_match(&target.url),
            None => self.url == target.url,
        };

        if self.url_only {
            return matched_uri_regex;
        }

        if !matched_uri_regex {
            return false;
        }

        self.match_queries_tree(&target.queries)
    }

    #[must_use]
    pub fn matches_url(&self, target: &str) -> bool {
        let matched_uri_regex = match &self.matcher {
            Some(inner) => inner.is_match(target),
            None => self.url == target,
        };

        if self.url_only {
            return matched_uri_regex;
        }

        if !matched_uri_regex {
            return false;
        }

        self.match_queries(target)
    }

    pub fn match_queries(&self, target: &str) -> bool {
        let target_queries = Self::capture_query_hashmap(target);
        self.match_queries_tree(&target_queries)
    }

    pub fn match_queries_tree(&self, target_queries: &Option<BTreeMap<String, String>>) -> bool {
        if self.queries.is_none() && target_queries.is_none() {
            return true;
        }
        if self.queries.is_none() && target_queries.is_some() {
            return false;
        }
        if self.queries.is_some() && target_queries.is_none() {
            return false;
        }

        match &self.queries {
            Some(inner) => match target_queries {
                Some(extracted_queries) => {
                    let mut found = true;
                    for (expected_key, expected_value) in inner {
                        if let Some(value) = extracted_queries.get(expected_key) {
                            if expected_value != value && expected_value != "*" {
                                found = false;
                                break;
                            }
                            continue;
                        }

                        found = false;
                        break;
                    }
                    found
                }
                None => false,
            },
            None => false,
        }
    }

    #[must_use]
    pub fn capture_url_params(url: &str) -> Option<Vec<String>> {
        let re = Regex::new(CAPTURE_PARAM_STR).unwrap();
        let params: Vec<String> = re
            .captures_iter(url)
            .filter_map(|cap| cap.name("p").map(|p| String::from(p.as_str())))
            .collect();

        if params.is_empty() {
            return None;
        }
        Some(params)
    }

    pub fn capture_path_pattern(url: &str) -> regex::Regex {
        let re = Regex::new(CAPTURE_PARAM_STR).unwrap();
        let query_regex = Regex::new(CAPTURE_QUERY).unwrap();
        let pattern = query_regex.replace(url, "");
        let pattern = re.replace_all(&pattern, QUERY_REPLACER);
        let url_pattern = match Regex::new(&pattern) {
            Ok(item) => Ok(item),
            Err(err) => match &err {
                regex::Error::Syntax(detail) => {
                    tracing::error!("Regex syntax error occurred: {:?} -> {:?}", detail, err);
                    let escaped_url = regex::escape(&pattern);
                    Regex::new(&escaped_url)
                }
                _ => Err(err),
            },
        };
        url_pattern.expect("Should have created url matcher")
    }

    #[must_use]
    pub fn capture_query_hashmap(url: &str) -> Option<BTreeMap<String, String>> {
        let re = Regex::new(CAPTURE_QUERY_KEY_VALUE).unwrap();
        let path_regex = Regex::new(CAPTURE_PATH).unwrap();
        let only_query_parameters = path_regex.replace(url, "");

        let queries: BTreeMap<String, String> = re
            .captures_iter(&only_query_parameters)
            .filter_map(|cap| {
                if let Some(query_key) = cap.name("qk") {
                    let query_value = match cap.name("qv") {
                        Some(v) => String::from(v.as_str()),
                        None => String::new(),
                    };
                    return Some((String::from(query_key.as_str()), query_value));
                }
                None
            })
            .collect();

        if queries.is_empty() {
            return None;
        }
        Some(queries)
    }
}

#[cfg(test)]
mod simple_url_tests {
    use super::*;

    #[test]
    fn test_parsed_url_without_any_special_elements() {
        let content = "/v1/service/endpoint";
        let resource_url = SimpleUrl::url_with_query(content);
        let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint");

        assert!(matched);
        assert!(params.is_none());
    }

    #[test]
    fn test_parsed_url_with_multi_params_extracted() {
        let content = "/v1/service/endpoint/{user_id}/{message}";

        let params: Vec<String> = vec!["user_id".into(), "message".into()];

        let resource_url = SimpleUrl::url_with_query(content);

        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.queries, None);
        assert_eq!(resource_url.params, Some(params));
        assert!(resource_url.matcher.is_some());

        let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint/123/hello");

        assert!(matched);
        assert!(params.is_some());

        let mut expected_params: BTreeMap<String, String> = BTreeMap::new();
        expected_params.insert("user_id".into(), "123".into());
        expected_params.insert("message".into(), "hello".into());

        assert_eq!(params.unwrap(), expected_params);
    }

    #[test]
    fn test_parsed_url_with_params_extracted() {
        let content = "/v1/service/endpoint/{user_id}/message";

        let params: Vec<String> = vec!["user_id".into()];

        let resource_url = SimpleUrl::url_with_query(content);

        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.queries, None);
        assert_eq!(resource_url.params, Some(params));
        assert!(resource_url.matcher.is_some());

        let (matched, params) =
            resource_url.extract_matched_url("/v1/service/endpoint/123/message");

        assert!(matched);
        assert!(params.is_some());

        let mut expected_params: BTreeMap<String, String> = BTreeMap::new();
        expected_params.insert("user_id".into(), "123".into());

        assert_eq!(params.unwrap(), expected_params);
    }

    #[test]
    fn test_parsed_url_with_params() {
        let content = "/v1/service/endpoint/{user_id}/message";

        let params: Vec<String> = vec!["user_id".into()];

        let resource_url = SimpleUrl::url_with_query(content);

        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.queries, None);
        assert_eq!(resource_url.params, Some(params));
        assert!(resource_url.matcher.is_some());

        assert!(resource_url.matches_url("/v1/service/endpoint/123/message"));
        assert!(!resource_url.matches_url("/v1/service/endpoint/123/hello"));
    }

    #[test]
    fn test_parsed_url_with_queries() {
        let content = "/v1/service/endpoint?userId=123&hello=abc";
        let mut queries: BTreeMap<String, String> = BTreeMap::new();
        queries.insert("userId".into(), "123".into());
        queries.insert("hello".into(), "abc".into());

        let resource_url = SimpleUrl::url_with_query(content);
        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.params, None);
        assert_eq!(resource_url.queries, Some(queries));
        assert!(resource_url.matcher.is_some());
        assert!(resource_url.matches_url("/v1/service/endpoint?userId=123&hello=abc"));
        assert!(!resource_url.matches_url("/v1/service/endpoint?userId=567&hello=abc"));
        assert!(!resource_url.matches_url("/v1/service/endpoint?userId=123&hello=bda"));
    }

    #[test]
    fn test_unparsed_url() {
        let content = "/v1/service/endpoint?userId=123&hello=abc";
        let resource_url = SimpleUrl::url_only(content);
        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.params, None);
        assert_eq!(resource_url.queries, None);
        assert!(resource_url.matcher.is_none());
        assert!(resource_url.matches_url("/v1/service/endpoint?userId=123&hello=abc"));
        assert!(!resource_url.matches_url("/v1/service/endpoint?userId=123&hello=alex"));
        assert!(matches!(
            resource_url.extract_matched_url("/v1/service/endpoint?userId=123&hello=abc"),
            (true, None)
        ));
        assert!(matches!(
            resource_url.extract_matched_url("/v1/service/endpoint?userId=123&hello=alx"),
            (false, None)
        ));
    }
}

pub struct SimpleOutgoingResponse {
    pub proto: Proto,
    pub status: Status,
    pub headers: SimpleHeaders,
    pub body: Option<SimpleBody>,
}

impl SimpleOutgoingResponse {
    #[must_use]
    pub fn builder() -> SimpleOutgoingResponseBuilder {
        SimpleOutgoingResponseBuilder::default()
    }

    #[must_use]
    pub fn empty() -> SimpleOutgoingResponse {
        SimpleOutgoingResponseBuilder::default()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_LENGTH, "0")
            .build()
            .unwrap()
    }
}

#[derive(Default)]
pub struct SimpleOutgoingResponseBuilder {
    proto: Option<Proto>,
    status: Option<Status>,
    headers: Option<SimpleHeaders>,
    body: Option<SimpleBody>,
}

pub type SimpleResponseResult<T> = std::result::Result<T, SimpleResponseError>;

#[derive(From, Debug)]
pub enum SimpleResponseError {
    StatusIsRequired,
    StringConversion(TryIntoStringError),
}

impl std::error::Error for SimpleResponseError {}

impl core::fmt::Display for SimpleResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl SimpleOutgoingResponseBuilder {
    #[must_use]
    pub fn with_proto(mut self, proto: Proto) -> Self {
        self.proto = Some(proto);
        self
    }
    #[must_use]
    pub fn with_status(mut self, status: Status) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: SimpleBody) -> Self {
        self.body = Some(body);
        self
    }

    #[must_use]
    pub fn with_body_stream(mut self, body: SendVecIterator<BoxedError>) -> Self {
        self.body = Some(SimpleBody::Stream(Some(body)));
        self
    }

    pub fn with_body_bytes<S: Into<Vec<u8>>>(mut self, body: S) -> Self {
        self.body = Some(SimpleBody::Bytes(body.into()));
        self
    }

    pub fn with_body_string<S: Into<String>>(mut self, body: S) -> Self {
        self.body = Some(SimpleBody::Text(body.into()));
        self
    }

    #[must_use]
    pub fn with_headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, S: Into<String>>(mut self, key: H, value: S) -> Self {
        let mut headers = self.headers.unwrap_or_default();

        let actual_key = key.into();
        if let Some(values) = headers.get_mut(&actual_key) {
            values.push(value.into());
        } else {
            headers.insert(actual_key, vec![value.into()]);
        }

        self.headers = Some(headers);
        self
    }

    /// Builds the outgoing HTTP response.
    ///
    /// # Errors
    /// Returns an error if the status is not set or if building fails.
    pub fn build(self) -> SimpleResponseResult<SimpleOutgoingResponse> {
        let status = match self.status {
            Some(inner) => inner,
            None => return Err(SimpleResponseError::StatusIsRequired),
        };

        let mut headers = self.headers.unwrap_or_default();
        let proto = self.proto.unwrap_or(Proto::HTTP11);
        let body = self.body.unwrap_or(SimpleBody::None);

        match &body {
            SimpleBody::None => {
                headers.insert(SimpleHeader::CONTENT_LENGTH, vec![String::from("0")]);
            }
            SimpleBody::Bytes(inner) => {
                let content_length = inner
                    .len()
                    .try_into_string()
                    .map_err(SimpleResponseError::StringConversion)?;

                if let Some(header_values) = headers.get_mut(&SimpleHeader::CONTENT_LENGTH) {
                    header_values.push(content_length);
                } else {
                    headers.insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);
                }
            }
            SimpleBody::Text(inner) => {
                let content_length = inner
                    .len()
                    .try_into_string()
                    .map_err(SimpleResponseError::StringConversion)?;

                if let Some(header_values) = headers.get_mut(&SimpleHeader::CONTENT_LENGTH) {
                    header_values.push(content_length);
                } else {
                    headers.insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);
                }
            }
            _ => {}
        }

        Ok(SimpleOutgoingResponse {
            body: Some(body),
            proto,
            status,
            headers,
        })
    }
}

pub type SimpleRequestResult<T> = std::result::Result<T, SimpleRequestError>;

#[derive(From, Debug)]
pub enum SimpleRequestError {
    NoURLProvided,
    StringConversion(TryIntoStringError),
}

impl std::error::Error for SimpleRequestError {}

impl core::fmt::Display for SimpleRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone)]
pub struct RequestDescriptor {
    pub proto: Proto,
    pub request_url: SimpleUrl,
    pub headers: SimpleHeaders,
    pub method: SimpleMethod,
}

#[derive(Debug)]
pub struct SimpleIncomingRequest {
    pub proto: Proto,
    pub request_url: SimpleUrl,
    pub body: Option<SimpleBody>,
    pub headers: SimpleHeaders,
    pub method: SimpleMethod,
}

impl SimpleIncomingRequest {
    #[must_use]
    pub fn builder() -> SimpleIncomingRequestBuilder {
        SimpleIncomingRequestBuilder::default()
    }

    #[must_use]
    pub fn descriptor(&self) -> RequestDescriptor {
        RequestDescriptor {
            proto: self.proto.clone(),
            request_url: self.request_url.clone(),
            headers: self.headers.clone(),
            method: self.method.clone(),
        }
    }
}

#[derive(Default)]
pub struct SimpleIncomingRequestBuilder {
    proto: Option<Proto>,
    url: Option<SimpleUrl>,
    body: Option<SimpleBody>,
    method: Option<SimpleMethod>,
    headers: Option<SimpleHeaders>,
}

impl SimpleIncomingRequestBuilder {
    pub fn with_plain_url<S: Into<String>>(mut self, url: S) -> Self {
        self.url = Some(SimpleUrl::url_only(url.into()));
        self
    }

    pub fn with_parsed_url<S: Into<String>>(mut self, url: S) -> Self {
        self.url = Some(SimpleUrl::url_with_query(url.into()));
        self
    }

    #[must_use]
    pub fn with_url(mut self, url: SimpleUrl) -> Self {
        self.url = Some(url);
        self
    }

    #[must_use]
    pub fn with_some_body(mut self, body: Option<SimpleBody>) -> Self {
        self.body = body;
        self
    }

    #[must_use]
    pub fn with_proto(mut self, proto: Proto) -> Self {
        self.proto = Some(proto);
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: SimpleBody) -> Self {
        self.body = Some(body);
        self
    }

    #[must_use]
    pub fn with_body_stream(mut self, body: SendVecIterator<BoxedError>) -> Self {
        self.body = Some(SimpleBody::Stream(Some(body)));
        self
    }

    pub fn with_body_bytes<S: Into<Vec<u8>>>(mut self, body: S) -> Self {
        self.body = Some(SimpleBody::Bytes(body.into()));
        self
    }

    pub fn with_body_string<S: Into<String>>(mut self, body: S) -> Self {
        self.body = Some(SimpleBody::Text(body.into()));
        self
    }

    #[must_use]
    pub fn with_headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, S: Into<String>>(mut self, key: H, value: S) -> Self {
        let mut headers = self.headers.unwrap_or_default();

        let actual_key = key.into();
        let actual_value: String = value.into();
        let actual_value_parts: Vec<String> = actual_value
            .split(',')
            .map(|item| item.trim().into())
            .collect();

        if let Some(values) = headers.get_mut(&actual_key) {
            values.extend(actual_value_parts);
        } else {
            headers.insert(actual_key, actual_value_parts);
        }

        self.headers = Some(headers);
        self
    }

    #[must_use]
    pub fn with_method(mut self, method: SimpleMethod) -> Self {
        self.method = Some(method);
        self
    }

    /// Builds the incoming HTTP request.
    ///
    /// # Errors
    /// Returns an error if the URL is not provided or if building fails.
    pub fn build(self) -> SimpleRequestResult<SimpleIncomingRequest> {
        let request_url = match self.url {
            Some(inner) => inner,
            None => return Err(SimpleRequestError::NoURLProvided),
        };

        let mut headers = self.headers.unwrap_or_default();
        let proto = self.proto.unwrap_or(Proto::HTTP11);
        let method = self.method.unwrap_or(SimpleMethod::GET);
        let body = self.body.unwrap_or(SimpleBody::None);

        match &body {
            SimpleBody::None => {
                headers.insert(SimpleHeader::CONTENT_LENGTH, vec![String::from("0")]);
            }
            SimpleBody::Bytes(inner) => {
                let content_length = inner
                    .len()
                    .try_into_string()
                    .map_err(SimpleRequestError::StringConversion)?;

                if let Some(header_values) = headers.get_mut(&SimpleHeader::CONTENT_LENGTH) {
                    header_values.push(content_length);
                } else {
                    headers.insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);
                }
            }
            SimpleBody::Text(inner) => {
                let content_length = inner
                    .len()
                    .try_into_string()
                    .map_err(SimpleRequestError::StringConversion)?;

                if let Some(header_values) = headers.get_mut(&SimpleHeader::CONTENT_LENGTH) {
                    header_values.push(content_length);
                } else {
                    headers.insert(SimpleHeader::CONTENT_LENGTH, vec![content_length]);
                }
            }
            _ => {}
        }

        Ok(SimpleIncomingRequest {
            body: Some(body),
            proto,
            request_url,
            method,
            headers,
        })
    }
}

/// [`Http11ReqState`] is an interesting pattern I am playing with
/// where instead of forcing async where I want chunked process instead
/// we can use rust typed state pattern where we define an enum of a singular
/// type with it's multiple iterations where each defines a possible state
/// though we loose the benefit where a state can't be returned to since
/// we would use different structs in a true typedstate pattern.
/// I am not sure what to call this maybe the switching enum option state
/// pattern.
///
/// The benefit is that now I can represent different states of the rendering
/// of a HTTP 1.1 Request object via enum's options/variants where the iterator
/// [`Http11RequestIterator`] can swap out the state and use this to decide
/// it's internal state with just use of the Iterator.
/// The idea is this pattern will work regardless of whether sync or async
/// because you can wrap the iterator in an async iterator if you want which is nice
/// as iterator are pulled based nor pushed based, you need to call `Iterator::next` to
/// get the next data anyway which in my view fits great with such a pattern.
pub enum Http11ReqState {
    /// Stating variant of the rendering of a HTTP 1.1 request
    /// when this starts it renders the starting line of your request.
    /// e.g GET location:port HTTP/1.1
    ///
    /// Once done it moves state to the `Http11ReqState::Headers` variant.
    Intro(SimpleIncomingRequest),

    /// Second state which renders the headers of a request to the iterator
    /// as the next value.
    ///
    /// Once done it moves state to the `Http11ReqState::Body` variant.
    Headers(SimpleIncomingRequest),

    /// Third state which starts rendering the body of the request
    /// this variant is unique because depending on the body type it can
    /// go to the End variant or the `BodyStreaming` variant.
    ///
    /// Once done it moves state to the `Http11ReqState::BodyStream`
    ///  or `Http11ReqState::End` variant.
    Body(SimpleIncomingRequest),

    /// Fourth state which starts or continues rendering of the body in
    /// the case of a streaming equivalent where we can accept an Iterator
    /// for the stream and keep calling it as needed without going OOM because
    /// we can pause operation but use the enum state pattern simply get the next
    /// data chunk from the inner iterator on the next call to `Iterator::next()`.
    ///
    /// This is really a super useful pattern, I had a hard time thinking
    /// of what to do when you have no ability to pause a state due to
    /// say some IO that needs to occur. More nicer is this pattern will work
    /// in WebAssembly as well because its just a nitty iterator that can based on
    /// it's state decide to switch behaviour allow us to representing a streaming
    /// pattern easily because an iterator can move to the next state after calling
    /// it's `Iterator::next()` method and since until the stream is exhausted or an
    /// error is raised, we simply swap a new `Self::BodyStreaming` with the new state of
    /// the iterator securely tracked via the wrapping Arc.
    ///
    /// Once done it moves state to the `Http11ReqState::BodyStream`
    ///  or `Http11ReqState::End` variant.
    BodyStreaming(Option<SendVecIterator<BoxedError>>),

    /// `ChunkedBodyStreaming` like `BodyStreaming` is meant to support
    /// handling of a chunked body parts where
    ChunkedBodyStreaming(Option<ChunkedVecIterator<BoxedError>>),

    /// `LineFeedStreaming` like `BodyStreaming` is meant to support
    /// handling of a chunked body parts where
    LineFeedStreaming(Option<LineFeedVecIterator<BoxedError>>),

    /// The final state of the rendering which once read ends the iterator.
    End,
}

/// [`Http11RequestIterator`] represents the rendering of a `HTTP`
/// request via an Iterator pattern that supports both sync and async
/// contexts.
pub struct Http11RequestIterator(Option<Http11ReqState>);

impl Iterator for Http11RequestIterator {
    type Item = Result<Vec<u8>, Http11RenderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.take()? {
            Http11ReqState::Intro(request) => {
                let method = request.method.clone();
                let url = request.request_url.url.clone();
                // switch state to headers
                self.0 = Some(Http11ReqState::Headers(request));

                // generate HTTP 1.1 intro
                let http_intro_string = format!("{method} {url} HTTP/1.1\r\n",);

                Some(Ok(http_intro_string.into_bytes()))
            }
            Http11ReqState::Headers(request) => {
                // HTTP 1.1 requires atleast 1 header in the request being generated
                let borrowed_headers = &request.headers;
                if borrowed_headers.is_empty() {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ReqState::End);

                    return Some(Err(Http11RenderError::HeadersRequired));
                }

                let mut encoded_headers: Vec<String> = borrowed_headers
                    .iter()
                    .map(|(key, value)| {
                        let joined_value = value.join(", ");
                        format!("{key}: {joined_value}\r\n")
                    })
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // switch state to body rendering next
                self.0 = Some(Http11ReqState::Body(request));

                // join all intermediate with CLRF (last
                // element does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("").into_bytes()))
            }
            Http11ReqState::Body(mut request) => {
                if request.body.is_none() {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ReqState::End);

                    return Some(Err(Http11RenderError::InvalidSituationUsedIterator));
                }

                let body = request.body.take().unwrap();
                match body {
                    SimpleBody::None => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(b"".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(inner.into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(inner.clone()))
                    }
                    SimpleBody::ChunkedStream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ReqState::ChunkedBodyStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ReqState::End);
                            Some(Ok(b"\r\n".to_vec()))
                        }
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ReqState::BodyStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ReqState::End);
                            Some(Ok(b"\r\n".to_vec()))
                        }
                    }
                    SimpleBody::LineFeedStream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ReqState::LineFeedStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ReqState::End);
                            Some(Ok(b"\r\n".to_vec()))
                        }
                    }
                }
            }
            Http11ReqState::LineFeedStreaming(container) => {
                if let Some(mut body_iterator) = container {
                    if let Some(collected) = body_iterator.next() {
                        match collected {
                            Ok(inner) => {
                                self.0 =
                                    Some(Http11ReqState::LineFeedStreaming(Some(body_iterator)));

                                match inner {
                                    LineFeed::Line(content) => Some(Ok(content.into_bytes())),
                                    LineFeed::SKIP => Some(Ok(b"".to_vec())),
                                    LineFeed::END => {
                                        // tell the iterator we want it to end
                                        self.0 = Some(Http11ReqState::End);
                                        None
                                    }
                                }
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ReqState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ReqState::End);
                    Some(Ok(b"".to_vec()))
                }
            }
            Http11ReqState::ChunkedBodyStreaming(container) => {
                if let Some(mut body_iterator) = container {
                    if let Some(collected) = body_iterator.next() {
                        match collected {
                            Ok(mut inner) => {
                                self.0 =
                                    Some(Http11ReqState::ChunkedBodyStreaming(Some(body_iterator)));
                                Some(Ok(inner.into_bytes()))
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ReqState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ReqState::End);
                    Some(Ok(b"".to_vec()))
                }
            }
            Http11ReqState::BodyStreaming(container) => {
                if let Some(mut body_iterator) = container {
                    if let Some(collected) = body_iterator.next() {
                        match collected {
                            Ok(inner) => {
                                self.0 = Some(Http11ReqState::BodyStreaming(Some(body_iterator)));
                                Some(Ok(inner))
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ReqState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ReqState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ReqState::End);
                    Some(Ok(b"".to_vec()))
                }
            }

            // Ends the iterator
            Http11ReqState::End => None,
        }
    }
}

/// State representing the varying rendering status of a http response into
/// the final HTTP message.
pub enum Http11ResState {
    Intro(SimpleOutgoingResponse),
    Headers(SimpleOutgoingResponse),
    Body(SimpleOutgoingResponse),
    BodyStreaming(Option<SendVecIterator<BoxedError>>),
    LineFeedStreaming(Option<LineFeedVecIterator<BoxedError>>),
    ChunkedBodyStreaming(Option<ChunkedVecIterator<BoxedError>>),
    End,
}

pub struct Http11ResponseIterator(Option<Http11ResState>);

/// We want to implement an iterator that generates valid HTTP response
/// message like:
///
///   HTTP/1.1 200 OK
///   Date: Sun, 10 Oct 2010 23:26:07 GMT
///   Server: Apache/2.2.8 (Ubuntu) `mod_ssl/2.2.8` OpenSSL/0.9.8g
///   Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
///   `ETag`: "45b6-834-49130cc1182c0"
///   Accept-Ranges: bytes
///   Content-Length: 12
///   Connection: close
///   Content-Type: text/html
///
///   Hello world!
///
impl Iterator for Http11ResponseIterator {
    type Item = Result<Vec<u8>, Http11RenderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.take()? {
            Http11ResState::Intro(response) => {
                // switch state to headers

                // generate HTTP 1.1 intro
                let http_intro_string = format!("HTTP/1.1 {}\r\n", response.status.status_line());

                self.0 = Some(Http11ResState::Headers(response));

                Some(Ok(http_intro_string.into_bytes()))
            }
            Http11ResState::Headers(response) => {
                // HTTP 1.1 requires atleast 1 header in the response being generated
                if response.headers.is_empty() {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ResState::End);

                    return Some(Err(Http11RenderError::HeadersRequired));
                }

                let borrowed_headers = &response.headers;

                let mut encoded_headers: Vec<String> = borrowed_headers
                    .iter()
                    .map(|(key, value)| {
                        let joined_value = value.join(", ");
                        format!("{key}: {joined_value}\r\n")
                    })
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // switch state to body rendering next
                self.0 = Some(Http11ResState::Body(response));

                // join all intermediate with CLRF (last element
                // does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("").into_bytes()))
            }
            Http11ResState::Body(mut response) => {
                if response.body.is_none() {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ResState::End);

                    return Some(Err(Http11RenderError::InvalidSituationUsedIterator));
                }

                let body = response.body.take().unwrap();
                match body {
                    SimpleBody::None => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(b"".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(inner.into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(inner.clone()))
                    }
                    SimpleBody::ChunkedStream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ResState::ChunkedBodyStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ResState::End);
                            Some(Ok(b"".to_vec()))
                        }
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ResState::BodyStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ResState::End);
                            Some(Ok(b"".to_vec()))
                        }
                    }
                    SimpleBody::LineFeedStream(mut streamer_container) => {
                        if let Some(inner) = streamer_container.take() {
                            self.0 = Some(Http11ResState::LineFeedStreaming(Some(inner)));
                            Some(Ok(b"".to_vec()))
                        } else {
                            // tell the iterator we want it to end
                            self.0 = Some(Http11ResState::End);
                            Some(Ok(b"".to_vec()))
                        }
                    }
                }
            }
            Http11ResState::LineFeedStreaming(container) => {
                if let Some(mut body_iterator) = container {
                    if let Some(collected) = body_iterator.next() {
                        match collected {
                            Ok(inner) => {
                                self.0 =
                                    Some(Http11ResState::LineFeedStreaming(Some(body_iterator)));

                                match inner {
                                    LineFeed::Line(content) => Some(Ok(content.into_bytes())),
                                    LineFeed::SKIP => Some(Ok(b"".to_vec())),
                                    LineFeed::END => {
                                        // tell the iterator we want it to end
                                        self.0 = Some(Http11ResState::End);

                                        // return None here
                                        None
                                    }
                                }
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ResState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ResState::End);
                    Some(Ok(b"".to_vec()))
                }
            }
            Http11ResState::ChunkedBodyStreaming(mut response) => {
                if let Some(mut actual_iterator) = response.take() {
                    if let Some(collected) = actual_iterator.next() {
                        match collected {
                            Ok(mut chunked) => {
                                self.0 = Some(Http11ResState::ChunkedBodyStreaming(Some(
                                    actual_iterator,
                                )));
                                Some(Ok(chunked.into_bytes()))
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ResState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ResState::End);
                    Some(Ok(b"".to_vec()))
                }
            }
            Http11ResState::BodyStreaming(mut response) => {
                if let Some(mut actual_iterator) = response.take() {
                    let next = actual_iterator.next();

                    if let Some(collected) = next {
                        match collected {
                            Ok(inner) => {
                                self.0 = Some(Http11ResState::BodyStreaming(Some(actual_iterator)));

                                Some(Ok(inner))
                            }
                            Err(err) => {
                                // tell the iterator we want it to end
                                self.0 = Some(Http11ResState::End);
                                Some(Err(err.into()))
                            }
                        }
                    } else {
                        // tell the iterator we want it to end
                        self.0 = Some(Http11ResState::End);
                        Some(Ok(b"".to_vec()))
                    }
                } else {
                    // tell the iterator we want it to end
                    self.0 = Some(Http11ResState::End);
                    Some(Ok(b"".to_vec()))
                }
            }

            // Ends the iterator
            Http11ResState::End => None,
        }
    }
}

pub enum Http11 {
    Request(SimpleIncomingRequest),
    Response(SimpleOutgoingResponse),
}

impl Http11 {
    #[must_use]
    pub fn request(req: SimpleIncomingRequest) -> Self {
        Self::Request(req)
    }

    #[must_use]
    pub fn response(res: SimpleOutgoingResponse) -> Self {
        Self::Response(res)
    }
}

impl RenderHttp for Http11 {
    type Error = Http11RenderError;

    fn http_render(
        self,
    ) -> std::result::Result<BoxedResultIterator<Vec<u8>, Self::Error>, Self::Error> {
        match self {
            Http11::Request(request) => Ok(Box::new(Http11RequestIterator(Some(
                Http11ReqState::Intro(request),
            )))),
            Http11::Response(response) => Ok(Box::new(Http11ResponseIterator(Some(
                Http11ResState::Intro(response),
            )))),
        }
    }
}

#[cfg(test)]
mod simple_incoming_tests {
    use super::*;

    #[test]
    fn should_convert_to_get_request_with_custom_header() {
        let request = Http11::request(
            SimpleIncomingRequest::builder()
                .with_plain_url("/")
                .with_method(SimpleMethod::GET)
                .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
                .add_header(SimpleHeader::HOST, "localhost:8000")
                .add_header(SimpleHeader::Custom("X-VILLA".into()), "YES")
                .with_body_string("Hello")
                .build()
                .unwrap(),
        );

        assert_eq!(
            request.http_render_string().unwrap(),
            "GET / HTTP/1.1\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\nX-VILLA: YES\r\n\r\nHello"
        );
    }

    #[test]
    fn should_convert_to_get_request() {
        let request = Http11::request(
            SimpleIncomingRequest::builder()
                .with_plain_url("/")
                .with_method(SimpleMethod::GET)
                .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
                .add_header(SimpleHeader::HOST, "localhost:8000")
                .with_body_string("Hello")
                .build()
                .unwrap(),
        );

        assert_eq!(
            request.http_render_string().unwrap(),
            "GET / HTTP/1.1\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
        );
    }

    #[test]
    fn should_convert_to_get_response() {
        let request = Http11::response(
            SimpleOutgoingResponse::builder()
                .with_status(Status::OK)
                .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
                .add_header(SimpleHeader::HOST, "localhost:8000")
                .with_body_string("Hello")
                .build()
                .unwrap(),
        );

        assert_eq!(
            request.http_render_string().unwrap(),
            "HTTP/1.1 200 Ok\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
        );
    }

    #[test]
    fn should_convert_to_get_response_with_custom_status() {
        let request = Http11::response(
            SimpleOutgoingResponse::builder()
                .with_status(Status::Numbered(666, "Custom status".into()))
                .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
                .add_header(SimpleHeader::HOST, "localhost:8000")
                .with_body_string("Hello")
                .build()
                .unwrap(),
        );

        assert_eq!(
            request.http_render_string().unwrap(),
            "HTTP/1.1 666 Custom status\r\nCONTENT-LENGTH: 5\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\nHello"
        );
    }
}

pub struct SimpleResponse<T>(Status, SimpleHeaders, T);

impl SimpleResponse<()> {
    #[must_use]
    pub fn no_body(status: Status, headers: SimpleHeaders) -> Self {
        Self(status, headers, ())
    }
}

impl<T> SimpleResponse<T> {
    pub fn new(status: Status, headers: SimpleHeaders, body: T) -> Self {
        Self(status, headers, body)
    }

    pub fn get_status(&self) -> Status {
        self.0.clone()
    }

    pub fn get_headers_ref(&self) -> &SimpleHeaders {
        &self.1
    }

    pub fn get_headers_mut(&mut self) -> &mut SimpleHeaders {
        &mut self.1
    }

    pub fn get_body_ref(&self) -> &T {
        &self.2
    }

    pub fn get_body_mut(&mut self) -> &mut T {
        &mut self.2
    }
}

pub type Protocol = String;

#[derive(Debug, PartialEq, Eq)]
pub enum IncomingResponseParts {
    SKIP,
    NoBody,
    Intro(Status, Proto, Option<String>),
    Headers(SimpleHeaders),
    SizedBody(SimpleBody),
    StreamedBody(SimpleBody),
}

impl core::fmt::Display for IncomingResponseParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intro(status, proto, text) => {
                write!(f, "Intro({status:?}, {proto:?}, {text:?})")
            }
            Self::Headers(headers) => write!(f, "Headers({headers:?})"),
            Self::SizedBody(_) => write!(f, "SizedBody(_)"),
            Self::StreamedBody(_) => write!(f, "StreamedBody(_)"),
            Self::NoBody => write!(f, "NoBody"),
            Self::SKIP => write!(f, "SKIP"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum IncomingRequestParts {
    SKIP,
    NoBody,
    Intro(SimpleMethod, SimpleUrl, Proto),
    Headers(SimpleHeaders),
    SizedBody(SimpleBody),
    StreamedBody(SimpleBody),
}

impl core::fmt::Display for IncomingRequestParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intro(method, url, proto) => {
                write!(f, "Intro({method:?}, {url:?}, {proto})")
            }
            Self::Headers(headers) => write!(f, "Headers({headers:?})"),
            Self::SizedBody(_) => write!(f, "SizedBody(_)"),
            Self::StreamedBody(_) => write!(f, "StreamedBody(_)"),
            Self::NoBody => write!(f, "NoBody"),
            Self::SKIP => write!(f, "SKIP"),
        }
    }
}

pub trait BodyExtractor {
    /// extract will attempt to extract the relevant Body of a `TcpStream` shared
    /// stream by doing whatever internal logic is required to extract the necessary
    /// tcp body content required.
    ///
    /// This allows custom implementation of Tcp/Http body extractors.
    ///
    /// See sample implementation in `SimpleHttpBody`.
    fn extract<T: Read + 'static>(
        &self,
        body: Body,
        stream: SharedByteBufferStream<T>,
    ) -> Result<SimpleBody, BoxedError>;
}

const CHUNKED_VALUE: &str = "chunked";
const MAX_HEADER_NAME_LEN: usize = (1 << 16) - 1;
const TEXT_STREAM_MIME_TYPE: &str = "text/event-stream";

static SPACE_CHARS: &[char] = &[' ', '\n', '\t', '\r'];
static ALLOWED_HEADER_NAME_CHARS: &[char] = &[
    '!', '#', '$', '%', '&', '\'', '*', '+', '-', '.', '^', '_', '`', '|', '~',
];

static TRANSFER_ENCODING_VALUES: &[&str] = &["chunked", "compress", "deflate", "gzip"];

static NO_SPLIT_HEADERS: &[SimpleHeader] = &[
    SimpleHeader::DATE,
    SimpleHeader::ETAG,
    SimpleHeader::SERVER,
    SimpleHeader::LAST_MODIFIED,
];

#[derive(Clone)]
pub struct HeaderReader<T: std::io::Read> {
    reader: SharedByteBufferStream<T>,
    max_header_key_length: Option<usize>,
    max_header_value_length: Option<usize>,
    max_header_values_count: Option<usize>,
}

impl<T> HeaderReader<T>
where
    T: std::io::Read,
{
    #[must_use]
    pub fn new(
        reader: SharedByteBufferStream<T>,
        max_header_key_length: Option<usize>,
        max_header_values_count: Option<usize>,
        max_header_value_length: Option<usize>,
    ) -> Self {
        Self {
            reader,
            max_header_key_length,
            max_header_value_length,
            max_header_values_count,
        }
    }
}

impl<T> HeaderReader<T>
where
    T: std::io::Read,
{
    fn parse_headers(&mut self) -> Result<SimpleHeaders, HttpReaderError> {
        let mut headers: SimpleHeaders = BTreeMap::new();

        let mut line = String::new();

        // let mut borrowed_reader = match self.reader.write() {
        //     Ok(borrowed_reader) => borrowed_reader,
        //     Err(_) => return Err(HttpReaderError::GuardedResourceAccess),
        // };

        let mut last_header: Option<String> = None;

        loop {
            let line_read_result = self
                .reader
                .do_once_mut(|binding| binding.read_line(&mut line))
                .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

            if line_read_result.is_err() {
                return Err(line_read_result.unwrap_err());
            }

            tracing::debug!("HeaderLine: {:?}", &line);

            if line.trim().is_empty()
                && (line == "\n" || line == "\r\n" || line == "\n\n" || line.is_empty())
            {
                line.clear();
                break;
            }

            if !line.contains(':') && last_header.is_none() {
                return Err(HttpReaderError::InvalidHeaderLine);
            }

            let line_parts: Vec<&str> = line.splitn(2, ':').collect();

            tracing::debug!("HeaderLineParts: {:?}", &line_parts);

            // if its start with an invalid character then indicate error
            if line_parts.len() == 2 && line_parts[1].starts_with('\r') {
                return Err(HttpReaderError::HeaderValueStartingWithCR);
            }

            // Breaks line folding handling
            // if line_parts[1] == "\n" || line_parts[1].trim().is_empty() {
            //     return Err(HttpReaderError::HeaderValueStartingWithLF);
            // }

            let (header_key, header_value) = if !line.contains(':') && last_header.is_some() {
                (last_header.clone().unwrap(), line.clone())
            } else {
                tracing::debug!(
                    "HeaderLinePartsUnicodeTrim: {:?} -- {:?}",
                    &line_parts[0],
                    line_parts[1].trim_matches(|c: char| c.is_whitespace() || c.is_control()),
                );
                (line_parts[0].to_string(), line_parts[1].trim().to_string())
            };

            last_header = Some(header_key.clone());

            let max_header_key_length: usize = match self.max_header_key_length {
                Some(max_value) => max_value,
                None => MAX_HEADER_NAME_LEN,
            };

            if !header_key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ALLOWED_HEADER_NAME_CHARS.contains(&c))
            {
                return Err(HttpReaderError::HeaderKeyContainsNotAllowedChars);
            }

            if header_key.trim() == "" {
                return Err(HttpReaderError::InvalidHeaderKey);
            }

            if header_key.len() > max_header_key_length {
                return Err(HttpReaderError::HeaderKeyGreaterThanLimit(
                    MAX_HEADER_NAME_LEN,
                ));
            }

            // disallow encoded CR: "%0D"
            if header_key.contains("%0D") {
                return Err(HttpReaderError::HeaderKeyContainsEncodedCRLF);
            }

            // disallow encoded LF: "%0A"
            if header_key.contains("%0A") {
                return Err(HttpReaderError::HeaderKeyContainsEncodedCRLF);
            }

            if let Some(allowed_max_key_length) = self.max_header_key_length {
                if header_key.len() > allowed_max_key_length {
                    return Err(HttpReaderError::HeaderKeyTooLong);
                }
            }

            if let Some(allowed_max_key_length) = self.max_header_value_length {
                if header_value.len() > allowed_max_key_length {
                    return Err(HttpReaderError::HeaderKeyTooLong);
                }
            }

            tracing::debug!("HeaderKey: {:?}", &header_key);
            tracing::debug!(
                "HeaderValue: {:?} -> trimmed: {:?}",
                &header_value,
                header_value.trim()
            );

            if header_value.starts_with('\r') {
                return Err(HttpReaderError::HeaderValueStartingWithCR);
            }

            for space_char in SPACE_CHARS {
                if header_key.contains(*space_char) {
                    return Err(HttpReaderError::InvalidHeaderKey);
                }
            }

            if let Some(max_value) = self.max_header_value_length {
                if header_value.len() > max_value {
                    return Err(HttpReaderError::HeaderValueGreaterThanLimit(
                        MAX_HEADER_NAME_LEN,
                    ));
                }
            }

            // ALLOW empty header value
            if header_value.is_empty() {
                // return Err(HttpReaderError::InvalidHeaderValue);
                line.clear();
                continue;
            }

            if header_value == "," {
                return Err(HttpReaderError::InvalidHeaderValue);
            }
            if header_value.starts_with(',') {
                return Err(HttpReaderError::InvalidHeaderValueStarter);
            }
            if header_value.ends_with(" ,") {
                return Err(HttpReaderError::InvalidHeaderValueEnder);
            }

            // disallow encoded CR: "%0D"
            if header_value.contains("%0D") {
                return Err(HttpReaderError::HeaderValueContainsEncodedCRLF);
            }

            // disallow encoded LF: "%0A"
            if header_value.contains("%0A") {
                return Err(HttpReaderError::HeaderValueContainsEncodedCRLF);
            }

            // check if there is any funny business with headers
            // for header_value_part in header_value.split(','). {}

            tracing::debug!("[2] HeaderKey: {:?}", &header_key);
            tracing::debug!("[2] HeaderValue: {:?}", &header_value);

            let actual_key = SimpleHeader::from(header_key);
            if let Some(values) = headers.get_mut(&actual_key) {
                if header_value.trim() == "" {
                    line.clear();
                    continue;
                }

                if NO_SPLIT_HEADERS.iter().any(|n| n == &actual_key) {
                    tracing::debug!("[2] ExtendHeader: {:?}", &header_value);
                    values.push(header_value);
                } else {
                    tracing::debug!("[2] ExtendAndSplitHeader: {:?}", &header_value);
                    values.extend(header_value.split(',').map(|t| t.trim().into()));
                }

                if let Some(allowed_max_value_count) = self.max_header_values_count {
                    if values.len() > allowed_max_value_count {
                        return Err(HttpReaderError::HeaderValuesHasTooManyItems);
                    }
                }

                line.clear();
                continue;
            }

            if header_value.trim() == "" {
                headers.insert(actual_key, vec![]);

                line.clear();
                continue;
            }

            if NO_SPLIT_HEADERS.iter().any(|n| n == &actual_key) {
                tracing::debug!("[2] InsertHeader: {:?}", &header_value);

                headers.insert(actual_key, vec![header_value]);
            } else {
                let header_values: Vec<String> =
                    header_value.split(',').map(|t| t.trim().into()).collect();
                tracing::debug!(
                    "[2] InsertAndSplitHeader: {:?} -> values: {:?}",
                    &header_value,
                    &header_values
                );

                if let Some(allowed_max_value_count) = self.max_header_values_count {
                    if header_values.len() > allowed_max_value_count {
                        return Err(HttpReaderError::HeaderValuesHasTooManyItems);
                    }
                }

                headers.insert(actual_key, header_values);
            }

            line.clear();
        }

        Ok(headers)
    }
}

#[derive(Clone, Debug)]
pub enum HttpReadState {
    Intro,
    Headers,
    OnlyHeaders,
    Body(Body),
    NoBody,
    Finished,
}

#[derive(Clone)]
pub struct HttpRequestReader<F: BodyExtractor, T: std::io::Read> {
    reader: SharedByteBufferStream<T>,
    state: HttpReadState,
    bodies: F,
    max_body_length: Option<usize>,
    max_header_key_length: Option<usize>,
    max_header_value_length: Option<usize>,
    max_header_values_count: Option<usize>,
}

impl<F, T> HttpRequestReader<F, T>
where
    F: BodyExtractor,
    T: std::io::Read,
{
    pub fn new(reader: SharedByteBufferStream<T>, bodies: F) -> Self {
        Self {
            bodies,
            max_body_length: None,
            max_header_key_length: None,
            max_header_value_length: None,
            max_header_values_count: None,
            state: HttpReadState::Intro,
            reader,
        }
    }

    pub fn limited_body(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_body_length: usize,
    ) -> Self {
        Self {
            bodies,
            max_header_key_length: None,
            max_header_value_length: None,
            max_header_values_count: None,
            max_body_length: Some(max_body_length),
            state: HttpReadState::Intro,
            reader,
        }
    }

    pub fn limited_headers(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_header_key_length: usize,
        max_header_values_count: usize,
        max_header_value_length: usize,
    ) -> Self {
        Self {
            bodies,
            reader,
            max_body_length: None,
            max_header_key_length: Some(max_header_key_length),
            max_header_values_count: Some(max_header_values_count),
            max_header_value_length: Some(max_header_value_length),
            state: HttpReadState::Intro,
        }
    }

    pub fn limited(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_body_length: usize,
        max_header_key_length: usize,
        max_header_values_count: usize,
        max_header_value_length: usize,
    ) -> Self {
        Self {
            bodies,
            reader,
            max_body_length: Some(max_body_length),
            max_header_key_length: Some(max_header_key_length),
            max_header_values_count: Some(max_header_values_count),
            max_header_value_length: Some(max_header_value_length),
            state: HttpReadState::Intro,
        }
    }
}

static NO_BODY_METHODS: &[SimpleMethod] = &[SimpleMethod::HEAD, SimpleMethod::CONNECT];

impl<F, T> Iterator for HttpRequestReader<F, T>
where
    F: BodyExtractor,
    T: std::io::Read + 'static,
{
    type Item = Result<IncomingRequestParts, HttpReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        let no_body = match &self.state {
            HttpReadState::OnlyHeaders => true,
            _ => false,
        };

        match &self.state {
            HttpReadState::Intro => {
                let mut line = String::new();
                // let mut borrowed_reader = match self.reader.write() {
                //     Ok(borrowed_reader) => borrowed_reader,
                //     Err(_) => return Some(Err(HttpReaderError::GuardedResourceAccess)),
                // };

                let line_read_result = self
                    .reader
                    .do_once_mut(|binding| binding.read_line(&mut line))
                    .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

                if line_read_result.is_err() {
                    tracing::debug!("Http read error: {:?}", &line_read_result);

                    self.state = HttpReadState::Finished;
                    return Some(Err(line_read_result.unwrap_err()));
                }

                let intro_parts: Vec<&str> = line
                    .split_whitespace()
                    .filter(|item| !item.trim().is_empty())
                    .collect();

                if intro_parts.is_empty() {
                    self.state = HttpReadState::Intro;
                    return Some(Ok(IncomingRequestParts::SKIP));
                }

                // if the lines is more than two then this is not
                // allowed or wanted, so fail immediately.
                tracing::debug!(
                    "Http Starter with line: {:?} from {:?}",
                    &intro_parts,
                    &line
                );

                if intro_parts.len() != 2 && intro_parts.len() != 3 {
                    self.state = HttpReadState::Finished;
                    return Some(Err(HttpReaderError::InvalidLine(line.clone())));
                }

                let method = SimpleMethod::from(intro_parts[0].to_string());

                // ensure to capture and skip methods that should not have a body attached.
                self.state = if NO_BODY_METHODS.iter().any(|n| n == &method) {
                    HttpReadState::OnlyHeaders
                } else {
                    HttpReadState::Headers
                };

                // this means no protocol is provided, by default use HTTP11
                if intro_parts.len() == 2 {
                    tracing::debug!("Creating intro part from 2 components: {:?}", &intro_parts);

                    return Some(Ok(IncomingRequestParts::Intro(
                        method,
                        SimpleUrl::url_with_query(intro_parts[1].to_string()),
                        Proto::HTTP11,
                    )));
                }

                match Proto::from_str(intro_parts[2]) {
                    Ok(proto) => {
                        tracing::debug!("Creating intro part for: {:?}", proto);

                        Some(Ok(IncomingRequestParts::Intro(
                            method,
                            SimpleUrl::url_with_query(intro_parts[1].to_string()),
                            proto,
                        )))
                    }
                    Err(err) => {
                        tracing::error!(
                            "Error generating proto: {:?} from {:?}",
                            err,
                            &intro_parts
                        );

                        Some(Err(HttpReaderError::ProtoBuildFailed(Box::new(err))))
                    }
                }
            }
            HttpReadState::Headers | HttpReadState::OnlyHeaders => {
                let mut header_reader = HeaderReader::new(
                    self.reader.clone(),
                    self.max_header_key_length,
                    self.max_header_values_count,
                    self.max_header_value_length,
                );

                let headers = match header_reader.parse_headers() {
                    Ok(header) => header,
                    Err(err) => {
                        self.state = HttpReadState::Finished;
                        return Some(Err(err));
                    }
                };

                if no_body {
                    tracing::debug!("No body flag is set to true");
                    self.state = HttpReadState::NoBody;
                    return Some(Ok(IncomingRequestParts::Headers(headers)));
                }

                // if header has content type that is equal to text/event-stream
                // then set state to line feed streaming body.
                if let Some(content_types) = headers.get(&SimpleHeader::CONTENT_TYPE) {
                    if content_types
                        .iter()
                        .map(|item| item.to_lowercase())
                        .filter(|item| item == TEXT_STREAM_MIME_TYPE)
                        .count()
                        != 0
                    {
                        self.state = HttpReadState::Body(Body::LineFeedBody(headers.clone()));

                        return Some(Ok(IncomingRequestParts::Headers(headers)));
                    }
                }

                // if its a chunked body then send and move state to chunked body state
                if let Some(transfer_encodings) = headers.get(&SimpleHeader::TRANSFER_ENCODING) {
                    tracing::debug!("Transfer Encoding value: {:?}", &transfer_encodings);

                    let content_length_header = headers.get(&SimpleHeader::CONTENT_LENGTH);

                    if content_length_header.is_some() {
                        return Some(Err(
                            HttpReaderError::BothTransferEncodingAndContentLengthNotAllowed,
                        ));
                    }

                    let allowed_values: HashSet<String> = TRANSFER_ENCODING_VALUES
                        .iter()
                        .map(|item| (*item).into())
                        .collect();

                    let current_values: HashSet<String> = transfer_encodings
                        .iter()
                        .map(|item| item.to_lowercase())
                        .collect();

                    let difference: HashSet<_> =
                        current_values.difference(&allowed_values).collect();

                    if !difference.is_empty() {
                        return Some(Err(HttpReaderError::UnknownTransferEncodingHeaderValue));
                    }

                    if current_values.len() == 1 && current_values.get(CHUNKED_VALUE).is_none() {
                        return Some(Err(HttpReaderError::UnsupportedTransferEncodingType));
                    }

                    if current_values.len() > 1 {
                        if let Some(chunked_index) =
                            transfer_encodings.iter().position(|n| n == CHUNKED_VALUE)
                        {
                            tracing::debug!("Chunked header index: {}", chunked_index);
                            if chunked_index != (current_values.len() - 1) {
                                return Some(Err(HttpReaderError::ChunkedEncodingMustBeLast));
                            }
                        } else {
                            return Some(Err(HttpReaderError::UnknownTransferEncodingHeaderValue));
                        }
                    }

                    self.state = HttpReadState::Body(Body::ChunkedBody(
                        transfer_encodings.clone(),
                        headers.clone(),
                    ));
                    return Some(Ok(IncomingRequestParts::Headers(headers)));
                }

                // Since it does not have a TRANSFER_ENCODING header then it
                // must have a CONTENT_LENGTH
                // header.
                if let Some(content_size_headers) = headers.get(&SimpleHeader::CONTENT_LENGTH) {
                    if content_size_headers.is_empty() {
                        self.state = HttpReadState::NoBody;
                        return Some(Ok(IncomingRequestParts::Headers(headers)));
                    }

                    let selected = content_size_headers.len() - 1;
                    let content_size_str = content_size_headers
                        .get(selected)
                        .expect("get content size");
                    match content_size_str.parse::<u64>() {
                        Ok(value) => {
                            if let Some(max_value) = self.max_body_length {
                                if value > (max_value as u64) {
                                    return Some(Err(
                                        HttpReaderError::BodyContentSizeIsGreaterThanLimit(
                                            max_value,
                                        ),
                                    ));
                                }
                            }

                            if value == 0 {
                                self.state = HttpReadState::NoBody;
                            } else {
                                self.state =
                                    HttpReadState::Body(Body::LimitedBody(value, headers.clone()));
                            }

                            Some(Ok(IncomingRequestParts::Headers(headers)))
                        }
                        Err(err) => {
                            self.state = HttpReadState::Finished;
                            Some(Err(HttpReaderError::InvalidContentSizeValue(Box::new(err))))
                        }
                    }
                } else {
                    self.state = HttpReadState::NoBody;
                    Some(Ok(IncomingRequestParts::Headers(headers)))
                }
            }
            HttpReadState::NoBody => {
                self.state = HttpReadState::Finished;
                Some(Ok(IncomingRequestParts::NoBody))
            }
            HttpReadState::Body(body) => {
                let cloned_stream = self.reader.clone();
                match self.bodies.extract(body.clone(), cloned_stream) {
                    Ok(generated_body) => {
                        // once we've gotten a body iterator and gives it to the user
                        // the next state is finished.
                        self.state = HttpReadState::Finished;

                        match generated_body {
                            SimpleBody::None => Some(Ok(IncomingRequestParts::NoBody)),
                            SimpleBody::LineFeedStream(inner) => {
                                Some(Ok(IncomingRequestParts::StreamedBody(
                                    SimpleBody::LineFeedStream(inner),
                                )))
                            }
                            SimpleBody::Stream(inner) => Some(Ok(
                                IncomingRequestParts::StreamedBody(SimpleBody::Stream(inner)),
                            )),
                            SimpleBody::ChunkedStream(inner) => {
                                Some(Ok(IncomingRequestParts::StreamedBody(
                                    SimpleBody::ChunkedStream(inner),
                                )))
                            }
                            SimpleBody::Bytes(inner) => Some(Ok(IncomingRequestParts::SizedBody(
                                SimpleBody::Bytes(inner),
                            ))),
                            SimpleBody::Text(inner) => {
                                Some(Ok(IncomingRequestParts::SizedBody(SimpleBody::Text(inner))))
                            }
                        }
                    }
                    Err(err) => {
                        self.state = HttpReadState::Finished;
                        Some(Err(HttpReaderError::BodyBuildFailed(err)))
                    }
                }
            }
            HttpReadState::Finished => None,
        }
    }
}

#[derive(Clone)]
pub struct HttpResponseReader<F: BodyExtractor, T: std::io::Read + 'static> {
    reader: SharedByteBufferStream<T>,
    state: HttpReadState,
    bodies: F,
    max_body_length: Option<usize>,
    max_header_key_length: Option<usize>,
    max_header_value_length: Option<usize>,
    max_header_values_count: Option<usize>,
}

impl<F, T> HttpResponseReader<F, T>
where
    F: BodyExtractor,
    T: std::io::Read + 'static,
{
    pub fn new(reader: SharedByteBufferStream<T>, bodies: F) -> Self {
        Self {
            bodies,
            max_body_length: None,
            max_header_key_length: None,
            max_header_value_length: None,
            max_header_values_count: None,
            state: HttpReadState::Intro,
            reader,
        }
    }

    pub fn limited_body(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_body_length: usize,
    ) -> Self {
        Self {
            bodies,
            max_header_key_length: None,
            max_header_value_length: None,
            max_header_values_count: None,
            max_body_length: Some(max_body_length),
            state: HttpReadState::Intro,
            reader,
        }
    }

    pub fn limited_headers(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_header_key_length: usize,
        max_header_values_count: usize,
        max_header_value_length: usize,
    ) -> Self {
        Self {
            bodies,
            reader,
            max_body_length: None,
            max_header_key_length: Some(max_header_key_length),
            max_header_values_count: Some(max_header_values_count),
            max_header_value_length: Some(max_header_value_length),
            state: HttpReadState::Intro,
        }
    }

    pub fn limited(
        reader: SharedByteBufferStream<T>,
        bodies: F,
        max_body_length: usize,
        max_header_key_length: usize,
        max_header_values_count: usize,
        max_header_value_length: usize,
    ) -> Self {
        Self {
            bodies,
            reader,
            max_body_length: Some(max_body_length),
            max_header_key_length: Some(max_header_key_length),
            max_header_values_count: Some(max_header_values_count),
            max_header_value_length: Some(max_header_value_length),
            state: HttpReadState::Intro,
        }
    }

    /// Returns a mutable reference to the underlying stream.
    ///
    /// WHY: Allows access to the stream for operations like writing additional
    /// data or inspecting stream state while maintaining ownership in the reader.
    ///
    /// WHAT: Provides mutable access to the `SharedByteBufferStream` wrapped
    /// by this reader.
    ///
    /// # Returns
    ///
    /// A mutable reference to the underlying `SharedByteBufferStream<T>`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut reader = HttpResponseReader::new(stream, body_extractor);
    /// let stream = reader.stream_mut();
    /// // Can now write to or manipulate the stream
    /// ```
    pub fn stream_mut(&mut self) -> &mut SharedByteBufferStream<T> {
        &mut self.reader
    }
}

impl<F, T> Iterator for HttpResponseReader<F, T>
where
    F: BodyExtractor,
    T: std::io::Read + 'static,
{
    type Item = Result<IncomingResponseParts, HttpReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        let no_body = match &self.state {
            HttpReadState::OnlyHeaders => true,
            _ => false,
        };

        match &self.state {
            HttpReadState::Intro => {
                let mut line = String::new();

                let line_read_result = self
                    .reader
                    .do_once_mut(|binding| binding.read_line(&mut line))
                    .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

                if line_read_result.is_err() {
                    tracing::debug!("Http read error: {:?}", &line_read_result);

                    self.state = HttpReadState::Finished;
                    return Some(Err(line_read_result.unwrap_err()));
                }

                let intro_parts: Vec<&str> = line
                    .splitn(2, ' ')
                    .filter(|item| !item.trim().is_empty())
                    .collect();

                if intro_parts.is_empty() {
                    self.state = HttpReadState::Intro;
                    return Some(Ok(IncomingResponseParts::SKIP));
                }

                // if the lines is more than two then this is not
                // allowed or wanted, so fail immediately.
                tracing::debug!(
                    "Http Starter with line: {:?} from {:?}",
                    &intro_parts,
                    &line
                );

                if intro_parts.len() != 2 && intro_parts.len() != 3 {
                    self.state = HttpReadState::Finished;
                    return Some(Err(HttpReaderError::InvalidLine(line.clone())));
                }

                let status_parts: Vec<&str> = intro_parts[1]
                    .splitn(2, ' ')
                    .filter(|item| !item.trim().is_empty())
                    .collect();

                // ignore the last part, we do not care
                let status = Status::from(status_parts[0].to_string());
                tracing::debug!(
                    "Http Starter status: {:?} from {:?} (intro: {:?}",
                    &status,
                    &status_parts,
                    &intro_parts,
                );

                // ensure to capture and skip methods that should not have a body attached.
                self.state = HttpReadState::Headers;

                // this means no protocol is provided, by default use HTTP11
                let third_line: Option<String> = if intro_parts.len() == 3 {
                    tracing::debug!("Creating intro part from 3 components: {:?}", &intro_parts);
                    Some(String::from(intro_parts[2].trim()))
                } else if status_parts.len() > 1 {
                    tracing::debug!(
                        "Creating status text part from remaining status parts: {:?}",
                        &status_parts
                    );
                    Some(String::from(status_parts[1].trim()))
                } else {
                    None
                };

                match Proto::from_str(intro_parts[0]) {
                    Ok(proto) => {
                        tracing::debug!("Creating intro part for: {:?}", proto);

                        Some(Ok(IncomingResponseParts::Intro(status, proto, third_line)))
                    }
                    Err(err) => {
                        tracing::error!(
                            "Error generating proto: {:?} from {:?}",
                            err,
                            &intro_parts
                        );

                        Some(Err(HttpReaderError::ProtoBuildFailed(Box::new(err))))
                    }
                }
            }
            HttpReadState::Headers | HttpReadState::OnlyHeaders => {
                let mut header_reader = HeaderReader::new(
                    self.reader.clone(),
                    self.max_header_key_length,
                    self.max_header_values_count,
                    self.max_header_value_length,
                );

                let headers = match header_reader.parse_headers() {
                    Ok(header) => header,
                    Err(err) => {
                        self.state = HttpReadState::Finished;
                        return Some(Err(err));
                    }
                };

                if no_body {
                    tracing::debug!("No body flag is set to true");
                    self.state = HttpReadState::NoBody;
                    return Some(Ok(IncomingResponseParts::Headers(headers)));
                }

                // if header has content type that is equal to text/event-stream
                // then set state to line feed streaming body.
                if let Some(content_types) = headers.get(&SimpleHeader::CONTENT_TYPE) {
                    if content_types
                        .iter()
                        .map(|item| item.to_lowercase())
                        .filter(|item| item == TEXT_STREAM_MIME_TYPE)
                        .count()
                        != 0
                    {
                        self.state = HttpReadState::Body(Body::LineFeedBody(headers.clone()));

                        return Some(Ok(IncomingResponseParts::Headers(headers)));
                    }
                }

                // if no transfer encoding and content length provided then we wont fail but
                // read the body till EOF
                if headers.get(&SimpleHeader::TRANSFER_ENCODING).is_none()
                    && headers.get(&SimpleHeader::CONTENT_LENGTH).is_none()
                {
                    self.state =
                        HttpReadState::Body(Body::FullBody(headers.clone(), self.max_body_length));

                    return Some(Ok(IncomingResponseParts::Headers(headers)));
                }

                // if its a chunked body then send and move state to chunked body state
                if let Some(transfer_encodings) = headers.get(&SimpleHeader::TRANSFER_ENCODING) {
                    tracing::debug!("Transfer Encoding value: {:?}", &transfer_encodings);

                    let content_length_header = headers.get(&SimpleHeader::CONTENT_LENGTH);

                    if content_length_header.is_some() {
                        return Some(Err(
                            HttpReaderError::BothTransferEncodingAndContentLengthNotAllowed,
                        ));
                    }

                    let allowed_values: HashSet<String> = TRANSFER_ENCODING_VALUES
                        .iter()
                        .map(|item| (*item).into())
                        .collect();

                    let current_values: HashSet<String> = transfer_encodings
                        .iter()
                        .map(|item| item.to_lowercase())
                        .collect();

                    let difference: HashSet<_> =
                        current_values.difference(&allowed_values).collect();

                    if !difference.is_empty() {
                        return Some(Err(HttpReaderError::UnknownTransferEncodingHeaderValue));
                    }

                    if current_values.len() == 1 && current_values.get(CHUNKED_VALUE).is_none() {
                        return Some(Err(HttpReaderError::UnsupportedTransferEncodingType));
                    }

                    if current_values.len() > 1 {
                        if let Some(chunked_index) =
                            transfer_encodings.iter().position(|n| n == CHUNKED_VALUE)
                        {
                            tracing::debug!("Chunked header index: {}", chunked_index);
                            if chunked_index != (current_values.len() - 1) {
                                return Some(Err(HttpReaderError::ChunkedEncodingMustBeLast));
                            }
                        } else {
                            return Some(Err(HttpReaderError::UnknownTransferEncodingHeaderValue));
                        }
                    }

                    self.state = HttpReadState::Body(Body::ChunkedBody(
                        transfer_encodings.clone(),
                        headers.clone(),
                    ));
                    return Some(Ok(IncomingResponseParts::Headers(headers)));
                }

                // Since it does not have a TRANSFER_ENCODING header then it
                // must have a CONTENT_LENGTH
                // header.
                if let Some(content_size_headers) = headers.get(&SimpleHeader::CONTENT_LENGTH) {
                    if content_size_headers.is_empty() {
                        self.state = HttpReadState::NoBody;
                        return Some(Ok(IncomingResponseParts::Headers(headers)));
                    }

                    let selected = content_size_headers.len() - 1;
                    let content_size_str = content_size_headers
                        .get(selected)
                        .expect("get content size");
                    match content_size_str.parse::<u64>() {
                        Ok(value) => {
                            if let Some(max_value) = self.max_body_length {
                                if value > (max_value as u64) {
                                    return Some(Err(
                                        HttpReaderError::BodyContentSizeIsGreaterThanLimit(
                                            max_value,
                                        ),
                                    ));
                                }
                            }

                            if value == 0 {
                                self.state = HttpReadState::NoBody;
                            } else {
                                self.state =
                                    HttpReadState::Body(Body::LimitedBody(value, headers.clone()));
                            }

                            Some(Ok(IncomingResponseParts::Headers(headers)))
                        }
                        Err(err) => {
                            self.state = HttpReadState::Finished;
                            Some(Err(HttpReaderError::InvalidContentSizeValue(Box::new(err))))
                        }
                    }
                } else {
                    self.state = HttpReadState::NoBody;
                    Some(Ok(IncomingResponseParts::Headers(headers)))
                }
            }
            HttpReadState::NoBody => {
                self.state = HttpReadState::Finished;
                Some(Ok(IncomingResponseParts::NoBody))
            }
            HttpReadState::Body(body) => {
                let cloned_stream = self.reader.clone();
                match self.bodies.extract(body.clone(), cloned_stream) {
                    Ok(generated_body) => {
                        // once we've gotten a body iterator and gives it to the user
                        // the next state is finished.
                        self.state = HttpReadState::Finished;

                        match generated_body {
                            SimpleBody::None => Some(Ok(IncomingResponseParts::NoBody)),
                            SimpleBody::Stream(inner) => Some(Ok(
                                IncomingResponseParts::StreamedBody(SimpleBody::Stream(inner)),
                            )),
                            SimpleBody::LineFeedStream(inner) => {
                                Some(Ok(IncomingResponseParts::StreamedBody(
                                    SimpleBody::LineFeedStream(inner),
                                )))
                            }
                            SimpleBody::ChunkedStream(inner) => {
                                Some(Ok(IncomingResponseParts::StreamedBody(
                                    SimpleBody::ChunkedStream(inner),
                                )))
                            }
                            SimpleBody::Bytes(inner) => Some(Ok(IncomingResponseParts::SizedBody(
                                SimpleBody::Bytes(inner),
                            ))),
                            SimpleBody::Text(inner) => Some(Ok(IncomingResponseParts::SizedBody(
                                SimpleBody::Text(inner),
                            ))),
                        }
                    }
                    Err(err) => {
                        self.state = HttpReadState::Finished;
                        Some(Err(HttpReaderError::BodyBuildFailed(err)))
                    }
                }
            }
            HttpReadState::Finished => None,
        }
    }
}

pub type Line = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LineFeed {
    Line(Line),
    SKIP,
    END,
}

impl LineFeed {
    /// Parses line feeds from a byte string.
    ///
    /// # Errors
    /// Returns an error if parsing the line feeds fails.
    pub fn stream_line_feeds_from_string(chunk_text: &[u8]) -> Result<Self, LineFeedError> {
        let cursor = Cursor::new(chunk_text.to_vec());
        let reader = SharedByteBufferStream::rwrite(cursor);
        Self::stream_line_feeds(reader)
    }

    pub fn stream_line_feeds<T: Read>(
        pointer: SharedByteBufferStream<T>,
    ) -> Result<Self, LineFeedError> {
        while pointer.do_once_mut(|binding| {
            if let Ok(b) = binding.nextby2(1) {
                match b {
                    b"\r" => {
                        // if 3 forward peeks reveal additional CLRF, two or three newlines
                        // then we've gotten to end of trailer, simply just end it there without
                        // moving back.

                        if crate::is_ok!(binding.peekby2(3), b"\n\r\n", b"\n\n\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }
                        if crate::is_ok!(binding.peekby2(3), b"\n\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }
                        // if even with 3 bytes forward peeks, if its still only more newline, then
                        // consider this has end of chunk and move back by 1.
                        if crate::is_ok!(binding.peekby2(3), b"\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }

                        // NOTE: We only want to capture one line at a time
                        if crate::is_ok!(binding.peekby2(1), b"\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }

                        return true;
                    }
                    b"\n" => {
                        // if 3 forward peeks reveal additional CLRF, two or three newlines
                        // then we've gotten to end of trailer, simply just end it there without
                        // moving back.
                        if crate::is_ok!(binding.peekby2(3), b"\r\n", b"\n\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }
                        if crate::is_ok!(binding.peekby2(3), b"\n\n\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }
                        // if even with 3 bytes forward peeks, if its still only more newline, then
                        // consider this has end of chunk and move back by 1.
                        if crate::is_ok!(binding.peekby2(3), b"\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }

                        // NOTE: We only want to capture one line at a time
                        if crate::is_ok!(binding.peekby2(1), b"\n") {
                            let _ = binding.unforward_by(1);
                            return false;
                        }
                        return true;
                    }
                    _ => return true,
                }
            }
            false
        }) {
            continue;
        }

        let line_feed_result =
            match pointer.do_once_mut(crate::io::ioutils::ByteBufferPointer::consume_some) {
                Some(value) => match String::from_utf8(value.clone()) {
                    Ok(converted_string) => Ok(if converted_string.trim().is_empty() {
                        LineFeed::SKIP
                    } else {
                        tracing::debug!("Processing::LineFeed::Line: {:?} ", &converted_string);

                        // We could process here but maybe instead process else where:
                        // let trailers: Vec<(String, Option<String>)> = converted_string
                        //     .split("\r\n")
                        //     .filter(|item| !item.trim().is_empty())
                        //     .collect();

                        LineFeed::Line(converted_string)
                    }),
                    Err(err) => return Err(LineFeedError::InvalidUTF(err)),
                },
                None => Ok(LineFeed::END),
            };

        // eat all the space
        let () = Self::eat_space(pointer.clone())?;

        // eat all newlines
        let () = Self::eat_newlines(pointer.clone())?;

        // eat all crlf
        let () = Self::eat_crlf(pointer.clone())?;

        // eat all crlf
        let () = Self::eat_escaped_crlf(pointer.clone())?;

        line_feed_result
    }

    fn eat_newlines_pointer<T: Read>(acc: &mut ByteBufferPointer<T>) -> Result<(), LineFeedError> {
        let newline = b"\n";
        while let Ok(b) = acc.nextby2(1) {
            if b[0] == newline[0] {
                continue;
            }

            // move backwards
            let _ = acc.unforward();
            acc.skip();

            return Ok(());
        }
        Ok(())
    }

    fn eat_escaped_crlf_pointer<T: Read>(
        acc: &mut ByteBufferPointer<T>,
    ) -> Result<(), LineFeedError> {
        while let Ok(b) = acc.nextby2(2) {
            if b != b"\\r" && b != b"\\n" {
                let _ = acc.unforward_by(2);
                acc.skip();
                break;
            }
        }
        Ok(())
    }

    fn eat_crlf_pointer<T: Read>(acc: &mut ByteBufferPointer<T>) -> Result<(), LineFeedError> {
        while let Ok(b) = acc.nextby2(1) {
            if b[0] != b'\r' && b[0] != b'\n' {
                let _ = acc.unforward();
                acc.skip();
                break;
            }
        }
        Ok(())
    }

    fn eat_space_pointer<T: Read>(acc: &mut ByteBufferPointer<T>) -> Result<(), LineFeedError> {
        while let Ok(b) = acc.nextby2(1) {
            if b[0] == b' ' {
                continue;
            }

            // move backwards
            let _ = acc.unforward();
            acc.skip();
            return Ok(());
        }
        Ok(())
    }

    fn eat_newlines<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), LineFeedError> {
        pointer.do_once_mut(|binding| Self::eat_newlines_pointer(binding))
    }

    fn eat_escaped_crlf<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), LineFeedError> {
        pointer.do_once_mut(|binding| Self::eat_escaped_crlf_pointer(binding))
    }

    fn eat_crlf<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), LineFeedError> {
        pointer.do_once_mut(|binding| Self::eat_crlf_pointer(binding))
    }

    fn eat_space<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), LineFeedError> {
        pointer.do_once_mut(|binding| Self::eat_space_pointer(binding))
    }
}

#[cfg(test)]
mod test_line_feed_parser {
    use super::*;
    use tracing_test::traced_test;

    struct LineFeedSample {
        content: &'static [&'static str],
        expected: Vec<LineFeed>,
    }

    #[test]
    #[traced_test]
    fn test_chunk_state_parse_http_trailers() {
        let test_cases: Vec<LineFeedSample> = vec![LineFeedSample {
            expected: vec![
                LineFeed::Line("Farm: FarmValue".into()),
                LineFeed::Line("Farm:FarmValue".into()),
                LineFeed::Line("Farm:FarmValue".into()),
                LineFeed::Line("Farm:\rFarmValue".into()),
                LineFeed::Line("Farm:\nFarmValue".into()),
                LineFeed::END,
                // Will not capture joined lines but should in fact parse them one by one
                // Some(LineFeed::Line("Farm: FarmValue\r\nFarm: FarmValue".into())),
                //
                // So instead, we will see:
                LineFeed::Line("Farm: FarmValue".into()),
            ],
            content: &[
                "Farm: FarmValue\r\n",
                "Farm:FarmValue\n\n",
                "Farm:FarmValue\n\n\n",
                "Farm:\rFarmValue\r\n\r\n",
                "Farm:\nFarmValue\r\n\r\n",
                "\r\n",
                // only one portion is extracted here,
                // caller should call the stream again to
                // pull the next chunk within the stream
                "Farm: FarmValue\r\nFarm: FarmValue\r\n",
            ][..],
        }];

        for sample in test_cases {
            let chunks: Result<Vec<LineFeed>, LineFeedError> = sample
                .content
                .iter()
                .map(|t| LineFeed::stream_line_feeds_from_string(t.as_bytes()))
                .collect();

            assert!(chunks.is_ok());
            assert_eq!(chunks.unwrap(), sample.expected);
        }
    }
}

pub type ChunkSize = u64;
pub type ChunkSizeOctet = String;

// ChunkState provides a series of parsing functions that help process the Chunked Transfer Coding
// specification for Http 1.1.
//
// See https://datatracker.ietf.org/doc/html/rfc7230#[cfg(all(feature = "ssl-rustls", not(feature="ssl-openssl"), not(feature="ssl-native-tls"))))]ection-4.1:
//
//  4.1.  Chunked Transfer Coding
//
//   The chunked transfer coding wraps the payload body in order to
//    transfer it as a series of chunks, each with its own size indicator,
//    followed by an OPTIONAL trailer containing header fields.  Chunked
//    enables content streams of unknown size to be transferred as a
//    sequence of length-delimited buffers, which enables the sender to
//    retain connection persistence and the recipient to know when it has
//    received the entire message.
//
//      chunked-body   = *chunk
//                       last-chunk
//                       trailer-part
//                       CRLF
//
//      chunk          = chunk-size [ chunk-ext ] CRLF
//                       chunk-data CRLF
//      chunk-size     = 1*HEXDIG
//      last-chunk     = 1*("0") [ chunk-ext ] CRLF
//
//      chunk-data     = 1*OCTET ; a sequence of chunk-size octets
//
//    The chunk-size field is a string of hex digits indicating the size of
//    the chunk-data in octets.  The chunked transfer coding is complete
//    when a chunk with a chunk-size of zero is received, possibly followed
//    by a trailer, and finally terminated by an empty line.
//
//    A recipient MUST be able to parse and decode the chunked transfer
//    coding.
//
// 4.1.1.  Chunk Extensions
//
//    The chunked encoding allows each chunk to include zero or more chunk
//    extensions, immediately following the chunk-size, for the sake of
//    supplying per-chunk metadata (such as a signature or hash),
//    mid-message control information, or randomization of message body
//    size.
//
//      chunk-ext      = *( ";" chunk-ext-name [ "=" chunk-ext-val ] )
//
//      chunk-ext-name = token
//      chunk-ext-val  = token / quoted-string
//
//    The chunked encoding is specific to each connection and is likely to
//    be removed or recoded by each recipient (including intermediaries)
//    before any higher-level application would have a chance to inspect
//    the extensions.  Hence, use of chunk extensions is generally limited
//
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkState {
    Chunk(ChunkSize, ChunkSizeOctet, Option<Extensions>),
    LastChunk,
    Trailer(String),
}

impl ChunkState {
    #[must_use]
    pub fn new(chunk_size_octet: String, chunk_extension: Option<Extensions>) -> Self {
        Self::try_new(chunk_size_octet, chunk_extension).expect("should parse octet string")
    }

    pub fn try_new(
        chunk_size_octet: String,
        chunk_extension: Option<Extensions>,
    ) -> Result<Self, ChunkStateError> {
        match Self::parse_chunk_octet(chunk_size_octet.as_bytes()) {
            Ok(size) => Ok(Self::Chunk(size, chunk_size_octet, chunk_extension)),
            Err(err) => Err(err),
        }
    }

    /// Parses an HTTP trailer chunk from bytes.
    ///
    /// # Errors
    /// Returns an error if parsing the trailer chunk fails.
    pub fn parse_http_trailer_chunk(chunk_text: &[u8]) -> Result<Option<Self>, ChunkStateError> {
        let cursor = Cursor::new(chunk_text.to_vec());
        let reader = SharedByteBufferStream::ref_cell(cursor);
        Self::parse_http_trailer_from_pointer(reader)
    }

    /// Parses an HTTP chunk from bytes.
    ///
    /// # Errors
    /// Returns an error if parsing the chunk fails.
    pub fn parse_http_chunk(chunk_text: &[u8]) -> Result<Self, ChunkStateError> {
        let cursor = Cursor::new(chunk_text.to_vec());
        let reader = SharedByteBufferStream::ref_cell(cursor);
        Self::parse_http_chunk_from_pointer(reader)
    }

    /// Gets the length of an HTTP chunk header from bytes.
    ///
    /// # Errors
    /// Returns an error if getting the header length fails.
    pub fn get_http_chunk_header_length(chunk_text: &[u8]) -> Result<usize, ChunkStateError> {
        let cursor = Cursor::new(chunk_text.to_vec());
        let reader = SharedByteBufferStream::ref_cell(cursor);
        Self::get_http_chunk_header_length_from_pointer(reader)
    }

    pub fn parse_http_trailer_from_pointer<T: Read>(
        pointer: SharedByteBufferStream<T>,
    ) -> Result<Option<Self>, ChunkStateError> {
        // let mut acc = pointer.write().map_err(|_| ChunkStateError::ReadErrors)?;

        // eat all the space
        let () = Self::eat_space(pointer.clone())?;

        while pointer.do_once_mut(|acc| {
            if let Ok(b) = acc.nextby2(1) {
                match b {
                    b"\r" => {
                        // if 3 forward peeks reveal additional CLRF, two or three newlines
                        // then we've gotten to end of trailer, simply just end it there without
                        // moving back.
                        if crate::is_ok!(acc.peekby2(3), b"\n\r\n", b"\n\n\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }
                        if crate::is_ok!(acc.peekby2(3), b"\n\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }
                        // if even with 3 bytes forward peeks, if its still only more newline, then
                        // consider this has end of chunk and move back by 1.
                        if crate::is_ok!(acc.peekby2(3), b"\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }

                        // NOTE: We do not do this hear because we want to capture the whole trailer
                        // regardless of parts and then chunk up later.
                        //
                        // if crate::is_ok!(acc.peekby2(1), b"\n") {
                        //     let _ = acc.unforward_by(1);
                        //     break;
                        // }

                        return true;
                    }
                    b"\n" => {
                        // if 3 forward peeks reveal additional CLRF, two or three newlines
                        // then we've gotten to end of trailer, simply just end it there without
                        // moving back.
                        if crate::is_ok!(acc.peekby2(3), b"\r\n", b"\n\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }
                        if crate::is_ok!(acc.peekby2(3), b"\n\n\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }
                        // if even with 3 bytes forward peeks, if its still only more newline, then
                        // consider this has end of chunk and move back by 1.
                        if crate::is_ok!(acc.peekby2(3), b"\n") {
                            let _ = acc.unforward_by(1);
                            return false;
                        }

                        // NOTE: We do not do this hear because we want to capture the whole trailer
                        // regardless of parts and then chunk up later.
                        //
                        // if crate::is_ok!(acc.peekby2(1), b"\n") {
                        //     let _ = acc.unforward_by(1);
                        //     break;
                        // }
                        return true;
                    }
                    _ => return true,
                }
            }

            false
        }) {
            continue;
        }

        match pointer.do_once_mut(crate::io::ioutils::ByteBufferPointer::consume) {
            Ok(value) => match String::from_utf8(value.clone()) {
                Ok(converted_string) => Ok(
                    if converted_string.is_empty() || converted_string.trim().is_empty() {
                        None
                    } else {
                        Some(ChunkState::Trailer(converted_string))
                    },
                ),
                Err(err) => Err(ChunkStateError::InvalidOctetBytes(err)),
            },
            Err(_) => Ok(None),
        }
    }

    /// `get_http_chunk_header_length_with_pointer` lets you count the amount of bytes of chunked transfer
    /// body chunk just right till the last CRLF before the actual data it refers to.
    /// This allows you easily know how far to read out from a stream reader so you know how much
    /// data to skip to get to the actual data of the chunk.
    pub fn get_http_chunk_header_length_from_pointer<T: Read>(
        pointer: SharedByteBufferStream<T>,
    ) -> Result<usize, ChunkStateError> {
        // let mut acc = pointer.write().map_err(|_| ChunkStateError::ReadErrors)?;
        let mut total_bytes = 0;

        // are we starting out with a CRLF, if so, count and skip it
        let _: Result<(), ChunkStateError> = pointer.do_once_mut(|acc| {
            if acc.nextby2(2)? != b"\r\n" {
                acc.skip();

                total_bytes += 2;
            }
            Ok(())
        });

        // fetch chunk_size_octet
        while pointer.do_once_mut(|acc| {
            if let Ok(content) = acc.nextby2(1) {
                match content {
                    b"\r" => {
                        let _ = acc.unforward();
                        return false;
                    }
                    // incase they use newline instead, http spec not respected
                    b"\n" => {
                        let _ = acc.unforward();
                        return false;
                    }
                    _ => {
                        total_bytes += 1;
                        return true;
                    }
                }
            }
            false
        }) {
            continue;
        }

        // NOTE: Some chunk encoding use \r\n and others \n\n for
        // chunk encoding to have newline instead both \r\n?
        // for now allowing this to work since nodejs handles it ok.
        //
        // if data_pointer.peek(2) != Some(b"\r\n") {
        //     return Err(ChunkStateError::InvalidChunkEndingExpectedCRLF);
        // }

        pointer.do_once_mut(|acc| {
            if acc.peekby2(1)? == b"\r" {
                // once we hit a CRLF then it means we have no extensions,
                // so we return just the size and its string representation.
                if acc.peekby2(2)? != b"\r\n" {
                    return Err(ChunkStateError::InvalidChunkEndingExpectedCRLF);
                }

                _ = acc.nextby(2);
                total_bytes += 1;
            }

            // is it just a newline here, then lets manage the madness
            if acc.peekby2(1)? == b"\n" {
                _ = acc.nextby2(1)?;
                total_bytes += 1;
            }

            // once we hit a CRLF then it means we have no extensions,
            // so we return just the size and its string representation.
            if acc.peekby2(1)? == b"\r" || acc.peekby2(1)? == b" " || acc.peekby2(1)? == b"\n" {
                return Err(ChunkStateError::InvalidChunkEndingExpectedCRLF);
            }

            acc.skip();

            Ok(total_bytes)
        })
    }

    pub fn parse_http_chunk_from_pointer<T: Read>(
        pointer: SharedByteBufferStream<T>,
    ) -> Result<Self, ChunkStateError> {
        // let mut acc = pointer.write()?;

        // eat up any space (except CRLF)
        Self::eat_space(pointer.clone())?;

        // are we starting out with a CRLF, if so, skip it
        let _: Result<(), ChunkStateError> = pointer.do_once_mut(|acc| {
            if acc.peekby2(2)? == b"\r\n" {
                acc.nextby2(2)?;
                acc.skip();
            }

            if acc.peekby2(2)? == b"\n\n" {
                acc.nextby2(2)?;
                acc.skip();
            }

            if acc.peekby2(1)? == b"\n" {
                acc.nextby(1)?;
                acc.skip();
            }

            Ok(())
        });

        // fetch chunk_size_octet
        let mut chunk_size_octet: Option<Vec<u8>> = pointer.do_once_mut(|acc| {
            let mut chunk_size_octet: Option<Vec<u8>> = None;
            while let Ok(content) = acc.nextby2(1) {
                let b = content[0];
                match b {
                    b'0'..=b'9' => continue,
                    b'a'..=b'f' => continue,
                    b'A'..=b'F' => continue,
                    b' ' | b'\r' | b'\n' | b';' => {
                        let _ = acc.unforward();
                        chunk_size_octet = Some(acc.consume()?);
                        break;
                    }
                    _ => {
                        return Err(ChunkStateError::InvalidOctetSizeByte(b));
                    }
                }
            }

            Ok(chunk_size_octet)
        })?;

        if chunk_size_octet.is_none() {
            return Err(ChunkStateError::ChunkSizeNotFound);
        }

        let (chunk_size, chunk_string): (u64, String) = match chunk_size_octet.take() {
            Some(value) => match Self::parse_chunk_octet(&value) {
                Ok(converted) => match String::from_utf8(value.clone()) {
                    Ok(converted_string) => (converted, converted_string),
                    Err(err) => return Err(ChunkStateError::InvalidOctetBytes(err)),
                },
                Err(err) => return Err(err),
            },
            None => return Err(ChunkStateError::ChunkSizeNotFound),
        };

        tracing::debug!(
            "Reading chunk size: {:?} to {:?}",
            &chunk_size,
            &chunk_string
        );

        Self::eat_space(pointer.clone())?;
        Self::eat_crlf(pointer.clone())?;
        Self::eat_escaped_crlf(pointer.clone())?;
        Self::eat_newlines(pointer.clone())?;

        // do w have the extension starter marker (a semicolon)
        let extensions: Extensions = pointer.do_once_mut(|acc| {
            let mut extensions: Extensions = Vec::new();
            if crate::is_ok!(acc.peekby2(1), b";") {
                while let Ok(value) = acc.peekby2(1) {
                    if value == b"\r" || value == b"\n" {
                        break;
                    }

                    match Self::parse_http_chunk_extension(acc) {
                        Ok(extension) => extensions.push(extension),
                        Err(err) => return Err(err),
                    }
                }
            }

            Ok(extensions)
        })?;

        tracing::debug!("Extensions : {:?}", &extensions);

        // are we starting out with a CRLF, if so, skip it
        pointer.do_once_mut(|acc| {
            if crate::is_ok!(acc.peekby2(2), b"\r\n") {
                let _ = acc.nextby(2);
                acc.skip();
            }

            if crate::is_ok!(acc.peekby2(2), b"\n\n") {
                let _ = acc.nextby2(2);
                acc.skip();
            }

            // eat all the space
            Self::eat_crlf_pointer(acc)?;
            Self::eat_newlines_pointer(acc)?;

            if chunk_size == 0 {
                return Ok(Self::LastChunk);
            }

            if extensions.is_empty() {
                return Ok(Self::Chunk(chunk_size, chunk_string, None));
            }

            Ok(Self::Chunk(chunk_size, chunk_string, Some(extensions)))
        })
    }

    pub fn parse_http_chunk_extension<T: Read>(
        acc: &mut ByteBufferPointer<T>,
    ) -> Result<(String, Option<String>), ChunkStateError> {
        // skip first extension starter
        if crate::is_ok!(acc.peekby2(1), b";") {
            acc.nextby2(1)?;
            acc.skip();
        }

        while let Ok(b) = acc.nextby2(1) {
            match b {
                b"=" | b";" | b"\r" | b"\n" => {
                    let _ = acc.unforward();
                    break;
                }
                _ => continue,
            }
        }

        let extension_key = acc.consume_some();

        // if we see a semicolon, this means this a
        // an extension without a value, stop and return as is
        if crate::is_ok!(acc.peekby2(1), b";") {
            acc.nextby2(1)?;
            acc.skip();

            if let Some(ext) = extension_key {
                return match String::from_utf8(ext) {
                    Ok(converted_string) => Ok((converted_string, None)),
                    Err(err) => Err(ChunkStateError::InvalidOctetBytes(err)),
                };
            }
        }

        if crate::is_ok!(acc.peekby2(1), b"\n") {
            if let Some(ext) = extension_key {
                return match String::from_utf8(ext) {
                    Ok(converted_string) => Ok((converted_string, None)),
                    Err(err) => Err(ChunkStateError::InvalidOctetBytes(err)),
                };
            }
        }

        // eat all the space
        Self::eat_space_pointer(acc)?;

        // skip first extension starter
        if !crate::is_ok!(acc.nextby2(1), b"=") {
            if let Some(ext) = extension_key {
                return match String::from_utf8(ext) {
                    Ok(converted_string) => Ok((converted_string, None)),
                    Err(err) => Err(ChunkStateError::InvalidOctetBytes(err)),
                };
            }
            return Err(ChunkStateError::ChunkSizeNotFound);
        }

        // eat the "=" (equal sign)
        acc.skip();

        // eat all the space
        Self::eat_space_pointer(acc)?;

        let is_quoted = crate::is_ok!(acc.peekby2(1), b"\"");

        // move pointer forward for quoted value
        let mut quoted = 0;
        if is_quoted {
            let _ = acc.forward();

            quoted += 1;
        }

        while let Ok(b) = acc.nextby2(1) {
            if is_quoted {
                match b {
                    b"\"" => {
                        // if the next one is not a semiconlon then increase
                        // quote count as this can be a embedded token.
                        if !crate::is_ok!(acc.peekby2(1), b";", b"\r", b"\n") {
                            quoted += 1;
                            continue;
                        }

                        // if we see another quote and we just one then break here
                        // should be the end of the extension;
                        if quoted == 1 {
                            break;
                        }

                        quoted -= 1;
                        continue;
                    }
                    _ => continue,
                }
            }

            match b {
                b";" | b"\r" | b"\n" => {
                    let _ = acc.unforward();
                    break;
                }
                _ => continue,
            }
        }

        let extension_value = acc.consume_some();

        if is_quoted {
            let _ = acc.forward();
            acc.skip();
        }

        if crate::is_ok!(acc.peekby2(1), b";") {
            acc.nextby2(1)?;
            acc.skip();
        }

        match (extension_key, extension_value) {
            (Some(key), Some(value)) => {
                match (
                    String::from_utf8(key.clone()),
                    String::from_utf8(value.clone()),
                ) {
                    (Ok(key_string), Ok(value_string)) => Ok((key_string, Some(value_string))),
                    (Ok(_), Err(err)) => Err(ChunkStateError::InvalidOctetBytes(err)),
                    (Err(err), Ok(_)) => Err(ChunkStateError::InvalidOctetBytes(err)),
                    (Err(err), Err(_)) => Err(ChunkStateError::InvalidOctetBytes(err)),
                }
            }
            (Some(key), None) => match String::from_utf8(key.clone()) {
                Ok(converted_string) => Ok((converted_string, None)),
                Err(err) => Err(ChunkStateError::InvalidOctetBytes(err)),
            },
            (None, Some(_)) => Err(ChunkStateError::ExtensionWithNoValue),
            (None, None) => Err(ChunkStateError::ParseFailed),
        }
    }

    fn eat_newlines_pointer<T: Read>(
        acc: &mut ByteBufferPointer<T>,
    ) -> Result<(), ChunkStateError> {
        let newline = b"\n";
        while let Ok(b) = acc.nextby2(1) {
            if b[0] == newline[0] {
                continue;
            }

            // move backwards
            let _ = acc.unforward();
            acc.skip();

            return Ok(());
        }
        Ok(())
    }

    fn eat_escaped_crlf_pointer<T: Read>(
        acc: &mut ByteBufferPointer<T>,
    ) -> Result<(), ChunkStateError> {
        while let Ok(b) = acc.nextby2(2) {
            if b != b"\\r" && b != b"\\n" {
                let _ = acc.unforward_by(2);
                acc.skip();
                break;
            }
        }
        Ok(())
    }

    fn eat_crlf_pointer<T: Read>(acc: &mut ByteBufferPointer<T>) -> Result<(), ChunkStateError> {
        while let Ok(b) = acc.nextby2(1) {
            if b[0] != b'\r' && b[0] != b'\n' {
                let _ = acc.unforward();
                acc.skip();
                break;
            }
        }
        Ok(())
    }

    fn eat_space_pointer<T: Read>(acc: &mut ByteBufferPointer<T>) -> Result<(), ChunkStateError> {
        while let Ok(b) = acc.nextby2(1) {
            if b[0] == b' ' {
                continue;
            }

            // move backwards
            let _ = acc.unforward();
            acc.skip();
            return Ok(());
        }
        Ok(())
    }

    fn eat_newlines<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), ChunkStateError> {
        pointer.do_once_mut(|binding| Self::eat_newlines_pointer(binding))
    }

    fn eat_escaped_crlf<T: Read>(
        pointer: SharedByteBufferStream<T>,
    ) -> Result<(), ChunkStateError> {
        pointer.do_once_mut(|binding| Self::eat_escaped_crlf_pointer(binding))
    }

    fn eat_crlf<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), ChunkStateError> {
        pointer.do_once_mut(|binding| Self::eat_crlf_pointer(binding))
    }

    fn eat_space<T: Read>(pointer: SharedByteBufferStream<T>) -> Result<(), ChunkStateError> {
        pointer.do_once_mut(|binding| Self::eat_space_pointer(binding))
    }

    /// Parse a buffer of bytes that should contain a hex string of the size of chunk.
    ///
    /// This is taking from the [httpparse](ttps://github.com/seanmonstar/httparse) crate.
    ///
    /// It uses math trics by using the positional int value of a byte from the characters in
    /// a hexadecimal (octet) number.
    ///
    /// For each byte we review, the underlying algothmn is as follows:
    ///
    /// 1. If its a 0-9 unicode byte, for each iteration, we take the previous size (default: 0)
    ///    then take the position of the first hex code `0` then we use the formula:
    ///    => size = (size * 16) + (`b::int` - `byte(0)::int`)
    ///    We then do the above formula for every number we see.
    /// 2. If its a alphabet (a-f) or (A-F), we also take the previous size (default: 0)
    ///    then take the position of the first hex code `0` then we use the formula:
    ///
    ///    => size = ((size * 16) + 10) + (`b::int` - byte('a')`::int`)
    ///
    ///    OR
    ///
    ///    => size = ((size * 16) + 10) + (`b::int` - byte('A')`::int`)
    ///
    /// This formulas ensure we can correctly map our hexadecimal octet string into
    /// the relevant value in numbers.
    ///
    /// # Errors
    /// Returns an error if the chunk size octet contains invalid hexadecimal characters.
    pub fn parse_chunk_octet(chunk_size_octet: &[u8]) -> Result<u64, ChunkStateError> {
        const RADIX: u64 = 16;
        let mut size: u64 = 0;

        let mut data_pointer = ubytes::BytesPointer::new(chunk_size_octet);
        while let Some(content) = data_pointer.peek_next() {
            let b = content[0];
            match b {
                b'0'..=b'9' => {
                    size *= RADIX;
                    size += u64::from(b - b'0');
                }
                b'a'..=b'f' => {
                    size *= RADIX;
                    size += u64::from(b + 10 - b'a');
                }
                b'A'..=b'F' => {
                    size *= RADIX;
                    size += u64::from(b + 10 - b'A');
                }
                _ => return Err(ChunkStateError::InvalidByte(b)),
            }
        }

        Ok(size)
    }
}

#[cfg(test)]
mod test_chunk_parser {
    use super::*;
    use tracing_test::traced_test;

    struct ChunkSample {
        content: &'static [&'static str],
        expected: Vec<ChunkState>,
    }

    struct TrailerSample {
        content: &'static [&'static str],
        expected: Vec<Option<ChunkState>>,
    }

    #[test]
    fn test_chunk_state_parse_http_trailers() {
        let test_cases: Vec<TrailerSample> = vec![TrailerSample {
            expected: vec![
                Some(ChunkState::Trailer("Farm: FarmValue".into())),
                Some(ChunkState::Trailer("Farm:FarmValue".into())),
                Some(ChunkState::Trailer("Farm:FarmValue".into())),
                None,
            ],
            content: &[
                "Farm: FarmValue\r\n",
                "Farm:FarmValue\r\n",
                "Farm:FarmValue\n\n",
                "\r\n",
            ][..],
        }];

        for sample in test_cases {
            let chunks: Result<Vec<Option<ChunkState>>, ChunkStateError> = sample
                .content
                .iter()
                .map(|t| ChunkState::parse_http_trailer_chunk(t.as_bytes()))
                .collect();

            assert!(chunks.is_ok());
            assert_eq!(chunks.unwrap(), sample.expected);
        }
    }

    #[test]
    #[traced_test]
    fn test_chunk_state_parse_http_chunk_code() {
        let test_cases: Vec<ChunkSample> = vec![
            ChunkSample {
                expected: vec![
                    ChunkState::Chunk(7, "7".into(), None),
                    ChunkState::Chunk(17, "11".into(), None),
                    ChunkState::LastChunk,
                ],
                content: &["7\r\nMozilla\r\n", "11\r\nDeveloper Network\r\n", "0\r\n"][..],
            },
            ChunkSample {
                expected: vec![
                    ChunkState::Chunk(
                        5,
                        "5".into(),
                        Some(vec![
                            (" comment".into(), Some("\"first chunk\"".into())),
                            ("day".into(), Some("1".into())),
                        ]),
                    ),
                    ChunkState::Chunk(
                        5,
                        "5".into(),
                        Some(vec![
                            (" comment".into(), Some("\"first chunk\"".into())),
                            (" age".into(), Some("1".into())),
                        ]),
                    ),
                    ChunkState::Chunk(
                        5,
                        "5".into(),
                        Some(vec![(" comment".into(), Some("\"first chunk\"".into()))]),
                    ),
                    ChunkState::Chunk(
                        5,
                        "5".into(),
                        Some(vec![(" comment".into(), Some("\"second chunk\"".into()))]),
                    ),
                    ChunkState::Chunk(
                        5,
                        "5".into(),
                        Some(vec![(" name".into(), Some("second".into()))]),
                    ),
                    ChunkState::Chunk(5, "5".into(), Some(vec![(" ranger".into(), None)])),
                    ChunkState::LastChunk,
                ],
                content: &[
                    "5; comment=\"first chunk\";day=1\r\nhello",
                    "5; comment=\"first chunk\"; age=1\r\nhello",
                    "5; comment=\"first chunk\"\r\nhello",
                    "5; comment=\"second chunk\"\r\nworld",
                    "5; name=second\r\nworld",
                    "5; ranger\r\nworld",
                    "0\r\n",
                ][..],
            },
        ];

        for sample in test_cases {
            let chunks: Result<Vec<ChunkState>, ChunkStateError> = sample
                .content
                .iter()
                .map(|t| ChunkState::parse_http_chunk(t.as_bytes()))
                .collect();

            dbg!(&chunks);
            assert!(chunks.is_ok());
            assert_eq!(chunks.unwrap(), sample.expected);
        }
    }

    #[test]
    fn test_chunk_state_octet_string_parsing() {
        assert!(matches!(
            ChunkState::try_new("0".into(), None),
            Ok(ChunkState::Chunk(0, _, _))
        ));
        assert!(matches!(
            ChunkState::try_new("12".into(), None),
            Ok(ChunkState::Chunk(18, _, _))
        ));
        assert!(matches!(
            ChunkState::try_new("35".into(), None),
            Ok(ChunkState::Chunk(53, _, _))
        ));
        assert!(matches!(
            ChunkState::try_new("3086d".into(), None),
            Ok(ChunkState::Chunk(198765, _, _))
        ));
    }
}

pub struct SimpleLineFeedIterator<T: std::io::Read>(SimpleHeaders, SharedByteBufferStream<T>);

impl<T: std::io::Read> Clone for SimpleLineFeedIterator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<T: std::io::Read> SimpleLineFeedIterator<T> {
    #[must_use]
    pub fn new(headers: SimpleHeaders, stream: SharedByteBufferStream<T>) -> Self {
        Self(headers, stream)
    }
}

impl<T: std::io::Read> Iterator for SimpleLineFeedIterator<T> {
    type Item = Result<LineFeed, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        match LineFeed::stream_line_feeds(self.1.clone()) {
            Ok(line) => {
                tracing::debug!("LineFeed::next_line: {:?}", &line);
                match &line {
                    LineFeed::END => None,
                    _ => Some(Ok(line)),
                }
            }
            Err(err) => Some(Err(Box::new(err))),
        }
    }
}

pub struct SimpleHttpChunkIterator<T: std::io::Read>(
    Vec<String>,
    SimpleHeaders,
    SharedByteBufferStream<T>,
    Arc<AtomicBool>,
);

impl<T: std::io::Read> Clone for SimpleHttpChunkIterator<T> {
    fn clone(&self) -> Self {
        Self(
            self.0.clone(),
            self.1.clone(),
            self.2.clone(),
            self.3.clone(),
        )
    }
}

impl<T: std::io::Read> SimpleHttpChunkIterator<T> {
    #[must_use]
    pub fn new(
        transfer_encoding: Vec<String>,
        headers: SimpleHeaders,
        stream: SharedByteBufferStream<T>,
    ) -> Self {
        Self(
            transfer_encoding,
            headers,
            stream,
            Arc::new(AtomicBool::new(false)),
        )
    }
}

impl<T: std::io::Read> Iterator for SimpleHttpChunkIterator<T> {
    type Item = Result<ChunkedData, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        let ending_indicator = self.3.clone();

        if ending_indicator.load(Ordering::Acquire) {
            tracing::debug!("ChunKState::ParsingTrailer");

            return match ChunkState::parse_http_trailer_from_pointer(self.2.clone()) {
                Ok(value) => {
                    tracing::debug!("ChunkTrailer::Chunk::DataRead: {:?} ", &value);
                    match value {
                        Some(item) => match item {
                            ChunkState::Trailer(inner) => {
                                tracing::debug!("Processing::Chunk::Trailer: {:?} ", &inner);
                                let trailers: Vec<(String, Option<String>)> = inner
                                    .split('\n')
                                    .filter(|item| !item.trim().is_empty())
                                    .map(|item| match item.find(':') {
                                        Some(index) => {
                                            let (key, value) = item.split_at(index);
                                            tracing::debug!("Processing::Chunk::Trailer::parts: key={:?} value={:?}", &key, value);
                                            (key.into(), Some(value[1..].trim().into()))
                                        }
                                        None => (item.into(), None),
                                    })
                                    .collect();

                                Some(Ok(ChunkedData::Trailers(trailers)))
                            }
                            _ => Some(Err(Box::new(HttpReaderError::OnlyTrailersAreAllowedHere))),
                        },
                        None => None,
                    }
                }
                Err(err) => Some(Err(Box::new(err))),
            };
        }

        tracing::debug!("ChunKState::StillParsingChunks");
        match ChunkState::parse_http_chunk_from_pointer(self.2.clone()) {
            Ok(chunk) => {
                match chunk {
                    ChunkState::Chunk(size, _, opt_exts) => {
                        // // calculate whats left in our in-mem pointer
                        // let remaining_bytes = head_pointer.rem_len();
                        //
                        // // how much exactly did it take to get the length of the chunk
                        // let total_header_bytes_used = total_header_read - remaining_bytes;
                        //
                        // // so add that to the size, so we can pull the chunk size + actual
                        // // data together since before we peeked.
                        // let bytes_we_need = total_header_bytes_used + (size as usize);

                        match self.2.do_once_mut(|reader| {
                            tracing::debug!("ChunkState::Chunk::GetSize: {:?}", size);

                            let mut chunk_data = vec![0; size as usize];
                            if let Err(err) = reader.read_exact(&mut chunk_data) {
                                return Err(Box::new(err));
                            }

                            tracing::debug!(
                                "ChunkState::Chunk::DataRead: {:?} | {:?}",
                                &chunk_data,
                                str::from_utf8(&chunk_data),
                            );

                            Ok(ChunkedData::Data(chunk_data, opt_exts))
                        }) {
                            Ok(value) => Some(Ok(value)),
                            Err(err) => Some(Err(Box::new(err))),
                        }
                    }
                    ChunkState::LastChunk => {
                        // set the state store as false
                        ending_indicator.store(true, Ordering::Release);

                        Some(Ok(ChunkedData::DataEnded))
                    }
                    ChunkState::Trailer(_) => {
                        Some(Err(Box::new(HttpReaderError::TrailerShouldNotOccurHere)))
                    }
                }
            }
            Err(err) => Some(Err(Box::new(err))),
        }
    }
}

#[derive(Default)]
pub struct SimpleHttpBody;

impl BodyExtractor for SimpleHttpBody {
    fn extract<T: std::io::Read + 'static>(
        &self,
        body: Body,
        stream: SharedByteBufferStream<T>,
    ) -> Result<SimpleBody, BoxedError> {
        match body {
            Body::LineFeedBody(headers) => {
                let line_feed_iterator = Box::new(SimpleLineFeedIterator::new(headers, stream));
                Ok(SimpleBody::LineFeedStream(Some(line_feed_iterator)))
            }
            Body::FullBody(_, optional_max_body_size) => {
                match stream.do_once_mut(|borrowed_stream| {
                    let mut body_content = Vec::with_capacity(1024);
                    borrowed_stream
                        .read_all(&mut body_content, optional_max_body_size)
                        .map(|_| SimpleBody::Bytes(body_content))
                }) {
                    Ok(inner) => Ok(inner),
                    Err(err) => Err(Box::new(err)),
                }
            }
            Body::LimitedBody(content_length, _) => {
                if content_length == 0 {
                    return Err(Box::new(HttpReaderError::ZeroBodySizeNotAllowed));
                }

                match stream.do_once_mut(|borrowed_stream| {
                    let mut body_content = vec![0; content_length as usize];
                    borrowed_stream
                        .read_exact(&mut body_content)
                        .map(|()| SimpleBody::Bytes(body_content))
                }) {
                    Ok(inner) => Ok(inner),
                    Err(err) => Err(Box::new(err)),
                }
            }
            Body::ChunkedBody(transfer_encoding, headers) => {
                let chunked_iterator = Box::new(SimpleHttpChunkIterator::new(
                    transfer_encoding,
                    headers,
                    stream,
                ));
                Ok(SimpleBody::ChunkedStream(Some(chunked_iterator)))
            }
        }
    }
}

impl<T: std::io::Read + 'static> HttpRequestReader<SimpleHttpBody, T> {
    #[must_use]
    pub fn simple_tcp_stream(
        reader: SharedByteBufferStream<T>,
    ) -> HttpRequestReader<SimpleHttpBody, T> {
        HttpRequestReader::<SimpleHttpBody, T>::new(reader, SimpleHttpBody)
    }
}

impl<T: std::io::Read + 'static> HttpResponseReader<SimpleHttpBody, T> {
    #[must_use]
    pub fn simple_tcp_stream(
        reader: SharedByteBufferStream<T>,
    ) -> HttpResponseReader<SimpleHttpBody, T> {
        HttpResponseReader::<SimpleHttpBody, T>::new(reader, SimpleHttpBody)
    }
}

/// [`HTTPStreams`] is a http reader that can handle multiple streams of http requests/responses where
/// it will yield an instance of [`HttpRequestReader`] or [`HttpResponseReader`] each time
/// it's [`HTTPStreams::read`] method is called.
///
/// It is expected that the returned reader is fully exhausted before the next http reader
/// is requested because the stream does not automatically know when the previous data of the last
/// http request has been fully read from the underlying [`RawStream`], specifically due to cases
/// where the underlying http request is a chunked or streaming body where we specifically do not
/// know where it ends.
pub struct HTTPStreams<T: std::io::Read + 'static> {
    source: SharedByteBufferStream<T>,
}

// Constructors

impl<T: std::io::Read + 'static> HTTPStreams<T> {
    #[must_use]
    pub fn new(source: SharedByteBufferStream<T>) -> Self {
        Self { source }
    }
}

// Methods

impl<T: std::io::Read + Send + 'static> HTTPStreams<T> {
    /// [`next_request`] returns a new [`HttpRequestReader`] to read the next read http request from the
    /// underlying stream allowing you to stream each request as a consecutive unit containing all
    /// its data parts.
    #[must_use]
    pub fn next_request(&self) -> HttpRequestReader<SimpleHttpBody, T> {
        HttpRequestReader::<SimpleHttpBody, T>::new(self.source.clone(), SimpleHttpBody)
    }

    /// [`next_response`] returns a new [`HttpResponse`] to read the next read http response from the
    /// underlying stream allowing you to stream each response as a consecutive unit containing all
    /// its data parts.
    #[must_use]
    pub fn next_response(&self) -> HttpResponseReader<SimpleHttpBody, T> {
        HttpResponseReader::<SimpleHttpBody, T>::new(self.source.clone(), SimpleHttpBody)
    }
}

pub mod http_streams {
    use super::{
        ioutils, HTTPStreams, HttpRequestReader, HttpResponseReader, Read, SimpleHttpBody,
    };

    pub mod no_send {
        use super::{
            ioutils, HTTPStreams, HttpRequestReader, HttpResponseReader, Read, SimpleHttpBody,
        };

        pub fn request_reader<T: Read + 'static>(
            reader: T,
        ) -> HttpRequestReader<SimpleHttpBody, T> {
            let byte_reader = ioutils::SharedByteBufferStream::ref_cell(reader);
            HttpRequestReader::<SimpleHttpBody, T>::new(byte_reader, SimpleHttpBody)
        }

        pub fn response_reader<T: Read + 'static>(
            reader: T,
        ) -> HttpResponseReader<SimpleHttpBody, T> {
            let byte_reader = ioutils::SharedByteBufferStream::ref_cell(reader);
            HttpResponseReader::<SimpleHttpBody, T>::new(byte_reader, SimpleHttpBody)
        }

        pub fn http_streams<T: Read + 'static>(reader: T) -> HTTPStreams<T> {
            let source = ioutils::SharedByteBufferStream::ref_cell(reader);
            HTTPStreams::new(source)
        }
    }

    pub mod send {
        use super::{
            ioutils, HTTPStreams, HttpRequestReader, HttpResponseReader, Read, SimpleHttpBody,
        };

        pub fn request_reader<T: Read + 'static>(
            reader: T,
        ) -> HttpRequestReader<SimpleHttpBody, T> {
            let byte_reader = ioutils::SharedByteBufferStream::rwrite(reader);
            HttpRequestReader::<SimpleHttpBody, T>::new(byte_reader, SimpleHttpBody)
        }

        pub fn response_reader<T: Read + 'static>(
            reader: T,
        ) -> HttpResponseReader<SimpleHttpBody, T> {
            let byte_reader = ioutils::SharedByteBufferStream::rwrite(reader);
            HttpResponseReader::<SimpleHttpBody, T>::new(byte_reader, SimpleHttpBody)
        }

        pub fn http_streams<T: Read + 'static>(reader: T) -> HTTPStreams<T> {
            let source = ioutils::SharedByteBufferStream::rwrite(reader);
            HTTPStreams::new(source)
        }
    }
}

pub trait SimpleServer {
    fn handle(&self, req: SimpleIncomingRequest) -> Result<SimpleOutgoingResponse, BoxedError>;
}

pub trait CloneableSimpleServer: SimpleServer + Send {
    fn clone_box(&self) -> Box<dyn CloneableSimpleServer>;
}

impl<F> CloneableSimpleServer for F
where
    F: 'static + Clone + Send + SimpleServer,
{
    fn clone_box(&self) -> Box<dyn CloneableSimpleServer> {
        Box::new(self.clone())
    }
}

pub type SimpleFunc = Box<
    dyn CloneableFn<SimpleIncomingRequest, Result<SimpleOutgoingResponse, BoxedError>>
        + Send
        + 'static,
>;

pub struct FuncSimpleServer {
    handler: SimpleFunc,
}

impl FuncSimpleServer {
    pub fn new<F>(f: F) -> Self
    where
        F: CloneableFn<SimpleIncomingRequest, Result<SimpleOutgoingResponse, BoxedError>>
            + Send
            + 'static,
    {
        Self {
            handler: Box::new(f),
        }
    }
}

impl Clone for FuncSimpleServer {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone_box(),
        }
    }
}

impl SimpleServer for FuncSimpleServer {
    fn handle(&self, req: SimpleIncomingRequest) -> Result<SimpleOutgoingResponse, BoxedError> {
        (self.handler)(req)
    }
}

pub struct ServiceActionList(Vec<ServiceAction>);

impl ServiceActionList {
    #[must_use]
    pub fn get_one_matching2(
        &self,
        url: &SimpleUrl,
        method: SimpleMethod,
    ) -> Option<ServiceAction> {
        for endpoint in &self.0 {
            if endpoint.match_head2(url, method.clone()) {
                return Some(endpoint.clone());
            }
        }

        None
    }

    #[must_use]
    pub fn get_matching2(
        &self,
        url: &SimpleUrl,
        method: SimpleMethod,
    ) -> Option<Vec<ServiceAction>> {
        let mut matches = Vec::new();

        for endpoint in &self.0 {
            if !endpoint.match_head2(url, method.clone()) {
                continue;
            }
            matches.push(endpoint.clone());
        }

        if matches.is_empty() {
            return None;
        }

        Some(matches)
    }

    #[must_use]
    pub fn get_one_matching(&self, url: &str, method: SimpleMethod) -> Option<ServiceAction> {
        for endpoint in &self.0 {
            if endpoint.match_head(url, method.clone()) {
                return Some(endpoint.clone());
            }
        }
        None
    }

    #[must_use]
    pub fn get_matching(&self, url: &str, method: SimpleMethod) -> Option<Vec<ServiceAction>> {
        let mut matches = Vec::new();

        for endpoint in &self.0 {
            if !endpoint.match_head(url, method.clone()) {
                continue;
            }
            matches.push(endpoint.clone());
        }

        if matches.is_empty() {
            return None;
        }

        Some(matches)
    }

    #[must_use]
    pub fn new(actions: Vec<ServiceAction>) -> Self {
        Self(actions)
    }
}

#[derive(Clone, Default)]
pub struct DefaultSimpleServer {}

impl SimpleServer for DefaultSimpleServer {
    fn handle(&self, _: SimpleIncomingRequest) -> Result<SimpleOutgoingResponse, BoxedError> {
        SimpleOutgoingResponse::builder()
            .with_status(Status::NoContent)
            .build()
            .map_err(|err| Box::new(err) as BoxedError)
    }
}

pub struct ServiceAction {
    pub route: SimpleUrl,
    pub method: SimpleMethod,
    pub headers: Option<SimpleHeaders>,
    pub body: Box<dyn CloneableSimpleServer + 'static>,
}

impl std::fmt::Debug for ServiceAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAction")
            .field("method", &self.method)
            .field("headers", &self.headers)
            .field("Body", &"Body(CloneableSimpleServer)")
            .finish()
    }
}

impl Clone for ServiceAction {
    fn clone(&self) -> Self {
        Self {
            body: self.body.clone_box(),
            method: self.method.clone(),
            route: self.route.clone(),
            headers: self.headers.clone(),
        }
    }
}

impl ServiceAction {
    #[must_use]
    pub fn builder() -> ServiceActionBuilder {
        ServiceActionBuilder::new()
    }

    #[must_use]
    pub fn match_head2(&self, url: &SimpleUrl, method: SimpleMethod) -> bool {
        if self.method != method {
            return false;
        }

        self.route.matches_other(url)
    }

    #[must_use]
    pub fn match_head(&self, url: &str, method: SimpleMethod) -> bool {
        if self.method != method {
            return false;
        }

        self.route.matches_url(url)
    }

    #[must_use]
    pub fn extract_match(
        &self,
        url: &str,
        method: SimpleMethod,
        headers: Option<SimpleHeaders>,
    ) -> (bool, Option<BTreeMap<String, String>>) {
        if self.method != method {
            return (false, None);
        }

        let (matched_url, extracted_params) = self.route.extract_matched_url(url);
        if !matched_url {
            return (false, None);
        }

        match (&self.headers, headers) {
            (Some(inner), Some(expected)) => {
                if inner == &expected {
                    return (matched_url, extracted_params);
                }
                (false, None)
            }
            (Some(_), None) => (false, None),
            (None, Some(_)) => (matched_url, extracted_params),
            (None, None) => (matched_url, extracted_params),
        }
    }
}

pub struct ServiceActionBuilder {
    method: Option<SimpleMethod>,
    route: Option<SimpleUrl>,
    headers: Option<SimpleHeaders>,
    body: Option<Box<dyn CloneableSimpleServer + Send + 'static>>,
}

impl Default for ServiceActionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceActionBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            method: None,
            route: None,
            headers: None,
            body: None,
        }
    }

    #[must_use]
    pub fn with_headers(mut self, headers: BTreeMap<SimpleHeader, Vec<String>>) -> Self {
        self.headers = Some(headers);
        self
    }

    #[must_use]
    pub fn with_method(mut self, method: SimpleMethod) -> Self {
        self.method = Some(method);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, J: Into<String>>(mut self, key: H, value: J) -> Self {
        let mut headers = self.headers.unwrap_or_default();

        let key_str: SimpleHeader = key.into();
        let value_str: String = value.into();
        if let Some(header_values) = headers.get_mut(&key_str) {
            header_values.push(value_str);
        } else {
            headers.insert(key_str, vec![value_str]);
        }

        self.headers = Some(headers);
        self
    }

    pub fn with_body(mut self, body: impl CloneableSimpleServer + 'static) -> Self {
        self.body = Some(Box::new(body));
        self
    }

    pub fn with_route<I: Into<String>>(mut self, route: I) -> Self {
        self.route = Some(SimpleUrl::url_with_query(route.into()));
        self
    }

    /// Builds the service action.
    ///
    /// # Errors
    /// Returns an error if the route is not provided or if building fails.
    pub fn build(self) -> SimpleHttpResult<ServiceAction> {
        let route = match self.route {
            Some(inner) => inner,
            None => return Err(SimpleHttpError::NoRouteProvided),
        };

        let method = match self.method {
            Some(inner) => inner,
            None => SimpleMethod::GET,
        };

        let body = match self.body {
            Some(inner) => inner,
            None => return Err(SimpleHttpError::NoBodyProvided),
        };

        Ok(ServiceAction {
            headers: self.headers,
            method,
            route,
            body,
        })
    }
}

#[cfg(test)]
mod service_action_test {
    use crate::extensions::result_ext::BoxedResult;

    use super::*;

    #[test]
    fn test_service_action_with_function_simple_server_can_clone() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_method(SimpleMethod::GET)
            .with_body(FuncSimpleServer::new(|_req| {
                SimpleOutgoingResponse::builder()
                    .with_status(Status::BadRequest)
                    .build()
                    .map_err(|err| err.into_boxed_error())
            }))
            .build()
            .expect("should generate service action");

        let cloned_resource = resource.clone();
        _ = cloned_resource;
    }

    #[test]
    fn test_service_action_can_clone() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_method(SimpleMethod::GET)
            .with_body(DefaultSimpleServer::default())
            .build()
            .expect("should generate service action");

        let cloned_resource = resource.clone();
        _ = cloned_resource;
    }

    #[test]
    fn test_service_action_match_url_with_headers() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_method(SimpleMethod::GET)
            .with_body(DefaultSimpleServer::default())
            .build()
            .expect("should generate service action");

        let mut headers = SimpleHeaders::new();
        let _ = headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/json".into()]);

        let (matched_url, params) =
            resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, Some(headers));

        assert!(matched_url);
        assert!(params.is_none());
    }

    #[test]
    fn test_service_action_match_url_only() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .with_method(SimpleMethod::GET)
            .with_body(DefaultSimpleServer::default())
            .build()
            .expect("should generate service action");

        let (matched_url, params) =
            resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, None);

        assert!(matched_url);
        assert!(params.is_none());
    }
}
