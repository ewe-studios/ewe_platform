use clonables::{
    ClonableBoxIterator, ClonableFnMut, ClonableStringIterator, ClonableVecIterator,
    WrappedIterator,
};
use derive_more::From;
use foundations_ext::strings_ext::{TryIntoString, TryIntoStringError};
use regex::Regex;
use std::{
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    io::{self, BufRead, Read},
    net::TcpStream,
    str::FromStr,
    string::{FromUtf16Error, FromUtf8Error},
};

pub type BoxedError = Box<dyn std::error::Error + Send>;

pub type Result<T, E> = std::result::Result<T, E>;

pub type Trailer = String;
pub type Extensions = HashMap<String, String>;

#[derive(Clone, Debug)]
pub enum ChunkedData {
    Data((Vec<u8>, Extensions)),
    End(Option<Trailer>),
}

impl ChunkedData {
    pub fn into_bytes(&mut self) -> Vec<u8> {
        match self {
            ChunkedData::Data((data, extensions)) => {
                let hexa_octect = format!("{:x}", data.len());
                let extension_string: Vec<String> = extensions
                    .into_iter()
                    .map(|(key, value)| {
                        if value == "" {
                            format!("; {}", key)
                        } else {
                            format!("; {}=\"{}\"", key, value)
                        }
                    })
                    .collect();

                let mut chunk_data: Vec<u8> = Vec::new();
                chunk_data.append(
                    &mut format!("{} {}", hexa_octect, extension_string.join("")).into_bytes(),
                );

                chunk_data.append(data);
                chunk_data
            }
            ChunkedData::End(optional_trailer) => match optional_trailer {
                Some(trailer) => trailer.clone().into_bytes(),
                None => b"\r\n".to_vec(),
            },
        }
    }
}

pub type ChunkedClonableVecIterator<E> = ClonableBoxIterator<ChunkedData, E>;

pub enum SimpleBody {
    None,
    Text(String),
    Bytes(Vec<u8>),
    Stream(Option<ClonableVecIterator<BoxedError>>),
    ChunkedStream(Option<ChunkedClonableVecIterator<BoxedError>>),
}

impl Clone for SimpleBody {
    fn clone(&self) -> Self {
        match self {
            Self::ChunkedStream(inner) => match inner {
                Some(item) => Self::ChunkedStream(Some(item.clone_box())),
                None => Self::Stream(None),
            },
            Self::Stream(inner) => match inner {
                Some(item) => Self::Stream(Some(item.clone_box())),
                None => Self::Stream(None),
            },
            Self::Text(inner) => Self::Text(inner.clone()),
            Self::Bytes(inner) => Self::Bytes(inner.clone()),
            Self::None => Self::None,
        }
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

/// RenderHttp lets types implement the ability to be rendered into
/// http protocol which makes it easily for more structured types.
#[allow(unused)]
trait RenderHttp: Send {
    type Error: From<FromUtf8Error> + From<BoxedError> + Send + 'static;

    fn http_render(
        &self,
    ) -> std::result::Result<WrappedIterator<Result<Vec<u8>, Self::Error>>, Self::Error>;

    /// http_render_encoded_string attempts to render the results of calling
    /// `RenderHttp::http_render()` as a custom encoded strings.
    fn http_render_encoded_string<E>(
        &self,
        encoder: E,
    ) -> std::result::Result<ClonableStringIterator<Self::Error>, Self::Error>
    where
        E: Fn(Result<Vec<u8>, Self::Error>) -> Result<String, Self::Error> + Send + Clone + 'static,
    {
        let render_bytes = self.http_render()?;
        let transformed = render_bytes.map(encoder);
        Ok(Box::new(transformed))
    }

    /// http_render_utf8_string attempts to render the results of calling
    /// `RenderHttp::http_render()` as utf8 strings.
    fn http_render_utf8_string(
        &self,
    ) -> std::result::Result<ClonableStringIterator<Self::Error>, Self::Error> {
        self.http_render_encoded_string(|part_result| match part_result {
            Ok(part) => match String::from_utf8(part) {
                Ok(inner) => Ok(inner),
                Err(err) => Err(err.into()),
            },
            Err(err) => Err(err.into()),
        })
    }

    /// allows implementing string representation of the http constructs
    /// as a string. You can override to implement a custom render but by
    /// default it calls `RenderHttp::http_render_utf8_string`.
    fn http_render_string(&self) -> std::result::Result<String, Self::Error> {
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
            _ => Self::Custom(upper),
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
            Self::Custom(inner) => write!(f, "{}", inner.to_uppercase()),
            Self::ACCEPT => write!(f, "ACCEPT"),
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
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
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
            "GET" => Self::GET,
            "POST" => Self::POST,
            "PUT" => Self::PUT,
            "DELETE" => Self::DELETE,
            "PATCH" => Self::PATCH,
            _ => Self::Custom(value.into()),
        }
    }
}

impl From<String> for SimpleMethod {
    fn from(value: String) -> Self {
        match value.to_uppercase().as_str() {
            "GET" => Self::GET,
            "POST" => Self::POST,
            "PUT" => Self::PUT,
            "DELETE" => Self::DELETE,
            "PATCH" => Self::PATCH,
            _ => Self::Custom(value),
        }
    }
}

impl SimpleMethod {
    fn value(&self) -> String {
        match self {
            SimpleMethod::GET => "GET".into(),
            SimpleMethod::POST => "POST".into(),
            SimpleMethod::PUT => "PUT".into(),
            SimpleMethod::DELETE => "DELETE".into(),
            SimpleMethod::PATCH => "PATCH".into(),
            SimpleMethod::Custom(inner) => inner.clone(),
        }
    }

    /// compares with string equivalent
    pub fn equal(&self, value: &str) -> bool {
        self.value() == value
    }
}

/// HTTP status
///
/// Can be converted to its numeral equivalent.
#[derive(Debug, Clone)]
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
    Custom(usize, &'static str),
}

impl core::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(code, _) => write!(f, "{}", code),
            _ => write!(f, "{}", self),
        }
    }
}

impl Status {
    /// Returns status' full description
    pub fn status_line(&self) -> String {
        match self {
            Status::Continue => "100 Continue".into(),
            Status::SwitchingProtocols => "101 Switching Protocols".into(),
            Status::Processing => "102 Processing".into(),
            Status::OK => "200 Ok".into(),
            Status::Created => "201 Created".into(),
            Status::Accepted => "202 Accepted".into(),
            Status::NonAuthoritativeInformation => "203 Non Authoritative Information".into(),
            Status::NoContent => "204 No Content".into(),
            Status::ResetContent => "205 Reset Content".into(),
            Status::PartialContent => "206 Partial Content".into(),
            Status::MultiStatus => "207 Multi Status".into(),
            Status::MultipleChoices => "300 Multiple Choices".into(),
            Status::MovedPermanently => "301 Moved Permanently".into(),
            Status::Found => "302 Found".into(),
            Status::SeeOther => "303 See Other".into(),
            Status::NotModified => "304 Not Modified".into(),
            Status::UseProxy => "305 Use Proxy".into(),
            Status::TemporaryRedirect => "307 Temporary Redirect".into(),
            Status::PermanentRedirect => "308 Permanent Redirect".into(),
            Status::BadRequest => "400 Bad Request".into(),
            Status::Unauthorized => "401 Unauthorized".into(),
            Status::PaymentRequired => "402 Payment Required".into(),
            Status::Forbidden => "403 Forbidden".into(),
            Status::NotFound => "404 Not Found".into(),
            Status::MethodNotAllowed => "405 Method Not Allowed".into(),
            Status::NotAcceptable => "406 Not Acceptable".into(),
            Status::ProxyAuthenticationRequired => "407 Proxy Authentication Required".into(),
            Status::RequestTimeout => "408 Request Timeout".into(),
            Status::Conflict => "409 Conflict".into(),
            Status::Gone => "410 Gone".into(),
            Status::LengthRequired => "411 Length Required".into(),
            Status::PreconditionFailed => "412 Precondition Failed".into(),
            Status::PayloadTooLarge => "413 Payload Too Large".into(),
            Status::UriTooLong => "414 URI Too Long".into(),
            Status::UnsupportedMediaType => "415 Unsupported Media Type".into(),
            Status::RangeNotSatisfiable => "416 Range Not Satisfiable".into(),
            Status::ExpectationFailed => "417 Expectation Failed".into(),
            Status::ImATeapot => "418 I'm A Teapot".into(),
            Status::UnprocessableEntity => "422 Unprocessable Entity".into(),
            Status::Locked => "423 Locked".into(),
            Status::FailedDependency => "424 Failed Dependency".into(),
            Status::UpgradeRequired => "426 Upgrade Required".into(),
            Status::PreconditionRequired => "428 Precondition Required".into(),
            Status::TooManyRequests => "429 Too Many Requests".into(),
            Status::RequestHeaderFieldsTooLarge => "431 Request Header Fields Too Large".into(),
            Status::InternalServerError => "500 Internal Server Error".into(),
            Status::NotImplemented => "501 Not Implemented".into(),
            Status::BadGateway => "502 Bad Gateway".into(),
            Status::ServiceUnavailable => "503 Service Unavailable".into(),
            Status::GatewayTimeout => "504 Gateway Timeout".into(),
            Status::HttpVersionNotSupported => "505 Http Version Not Supported".into(),
            Status::InsufficientStorage => "507 Insufficient Storage".into(),
            Status::NetworkAuthenticationRequired => "511 Network Authentication Required".into(),
            Self::Custom(code, description) => format!("{} {}", code, description),
        }
    }
}

/// ActUrl represents a url string and query parameters hashmap
#[derive(Clone, Debug)]
pub struct SimpleUrl {
    pub url: String,
    pub url_only: bool,
    pub matcher: Option<regex::Regex>,
    pub params: Option<Vec<String>>,
    pub queries: Option<BTreeMap<String, String>>,
}

static CAPTURE_QUERY: &'static str = r"\?.*";
static CAPTURE_PATH: &'static str = r".*\?";
static QUERY_REPLACER: &'static str = r"(?P<$p>[^//|/?]+)";
static CAPTURE_PARAM_STR: &'static str = r"\{(?P<p>([A-z|0-9|_])+)\}";
static CAPTURE_QUERY_KEY_VALUE: &'static str = r"((?P<qk>[^&]+)=(?P<qv>[^&]+))*";

#[allow(unused)]
impl SimpleUrl {
    pub(crate) fn new(
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

    /// url_only indicates you wish to represent a URL only where the Url
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

    /// url_with_query is used when parsing a url with queries
    /// e.g service.com/path/{param1}/{param2}?key=value&..
    /// this will extract these out into the `SimpleUrl` constructs.
    ///
    /// This is the method to use when constructing your ServiceAction
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

                        dbg!(&extracted_params);
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

        return (false, params);
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

    pub(crate) fn match_queries(&self, target: &str) -> bool {
        let target_queries = Self::capture_query_hashmap(target);
        self.match_queries_tree(&target_queries)
    }

    pub(crate) fn match_queries_tree(
        &self,
        target_queries: &Option<BTreeMap<String, String>>,
    ) -> bool {
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
                    for (expected_key, expected_value) in inner.iter() {
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

    pub fn capture_url_params(url: &str) -> Option<Vec<String>> {
        let re = Regex::new(CAPTURE_PARAM_STR).unwrap();
        let params: Vec<String> = re
            .captures_iter(url)
            .filter_map(|cap| match cap.name("p") {
                Some(p) => Some(String::from(p.as_str())),
                None => None,
            })
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
        Regex::new(&pattern).unwrap()
    }

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
                        None => String::from(""),
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
        assert!(matches!(params, None));
    }

    #[test]
    fn test_parsed_url_with_multi_params_extracted() {
        let content = "/v1/service/endpoint/{user_id}/{message}";

        let params: Vec<String> = vec!["user_id".into(), "message".into()];

        let resource_url = SimpleUrl::url_with_query(content);

        assert_eq!(resource_url.url, content);
        assert_eq!(resource_url.queries, None);
        assert_eq!(resource_url.params, Some(params));
        assert!(matches!(resource_url.matcher, Some(_)));

        let (matched, params) = resource_url.extract_matched_url("/v1/service/endpoint/123/hello");

        assert!(matched);
        assert!(matches!(params, Some(_)));

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
        assert!(matches!(resource_url.matcher, Some(_)));

        let (matched, params) =
            resource_url.extract_matched_url("/v1/service/endpoint/123/message");

        assert!(matched);
        assert!(matches!(params, Some(_)));

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
        assert!(matches!(resource_url.matcher, Some(_)));

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
        assert!(matches!(resource_url.matcher, Some(_)));
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
        assert!(matches!(resource_url.matcher, None));
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

pub type SimpleHeaders = BTreeMap<SimpleHeader, String>;

#[derive(Clone)]
pub struct SimpleOutgoingResponse {
    pub status: Status,
    pub headers: SimpleHeaders,
    pub body: Option<SimpleBody>,
}

impl SimpleOutgoingResponse {
    pub fn builder() -> SimpleOutgoingResponseBuilder {
        SimpleOutgoingResponseBuilder::default()
    }

    pub fn empty() -> SimpleOutgoingResponse {
        SimpleOutgoingResponseBuilder::default()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_LENGTH, "0")
            .build()
            .unwrap()
    }
}

#[derive(Clone, Default)]
pub struct SimpleOutgoingResponseBuilder {
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
    pub fn with_status(mut self, status: Status) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_body(mut self, body: SimpleBody) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_body_stream(mut self, body: ClonableVecIterator<BoxedError>) -> Self {
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

    pub fn with_headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, S: Into<String>>(mut self, key: H, value: S) -> Self {
        let mut headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        headers.insert(key.into(), value.into());
        self.headers = Some(headers);
        self
    }

    pub fn build(self) -> SimpleResponseResult<SimpleOutgoingResponse> {
        let status = match self.status {
            Some(inner) => inner,
            None => return Err(SimpleResponseError::StatusIsRequired),
        };

        let mut headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        let body = match self.body {
            Some(inner) => inner,
            None => SimpleBody::None,
        };

        let _ = match &body {
            SimpleBody::None => {
                headers.insert(SimpleHeader::CONTENT_LENGTH, String::from("0"));
            }
            SimpleBody::Bytes(inner) => {
                headers.insert(
                    SimpleHeader::CONTENT_LENGTH,
                    inner
                        .len()
                        .try_into_string()
                        .map_err(SimpleResponseError::StringConversion)?,
                );
            }
            SimpleBody::Text(inner) => {
                headers.insert(
                    SimpleHeader::CONTENT_LENGTH,
                    inner
                        .len()
                        .try_into_string()
                        .map_err(SimpleResponseError::StringConversion)?,
                );
            }
            _ => {}
        };

        Ok(SimpleOutgoingResponse {
            body: Some(body),
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

#[derive(Clone)]
pub struct SimpleIncomingRequest {
    pub request_url: String,
    pub body: Option<SimpleBody>,
    pub headers: SimpleHeaders,
    pub method: SimpleMethod,
}

impl SimpleIncomingRequest {
    pub fn builder() -> SimpleIncomingRequestBuilder {
        SimpleIncomingRequestBuilder::default()
    }
}

#[derive(Default)]
pub struct SimpleIncomingRequestBuilder {
    url: Option<String>,
    body: Option<SimpleBody>,
    method: Option<SimpleMethod>,
    headers: Option<SimpleHeaders>,
}

impl SimpleIncomingRequestBuilder {
    pub fn with_url<S: Into<String>>(mut self, url: S) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn with_body(mut self, body: SimpleBody) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_body_stream(mut self, body: ClonableVecIterator<BoxedError>) -> Self {
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

    pub fn with_headers(mut self, headers: SimpleHeaders) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, S: Into<String>>(mut self, key: H, value: S) -> Self {
        let mut headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        headers.insert(key.into(), value.into());
        self.headers = Some(headers);
        self
    }

    pub fn with_method(mut self, method: SimpleMethod) -> Self {
        self.method = Some(method);
        self
    }

    pub fn build(self) -> SimpleRequestResult<SimpleIncomingRequest> {
        let request_url = match self.url {
            Some(inner) => inner,
            None => return Err(SimpleRequestError::NoURLProvided),
        };

        let mut headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        let method = match self.method {
            Some(inner) => inner,
            None => SimpleMethod::GET,
        };

        let body = match self.body {
            Some(inner) => inner,
            None => SimpleBody::None,
        };

        let _ = match &body {
            SimpleBody::None => {
                headers.insert(SimpleHeader::CONTENT_LENGTH, String::from("0"));
            }
            SimpleBody::Bytes(inner) => {
                headers.insert(
                    SimpleHeader::CONTENT_LENGTH,
                    inner
                        .len()
                        .try_into_string()
                        .map_err(SimpleRequestError::StringConversion)?,
                );
            }
            SimpleBody::Text(inner) => {
                headers.insert(
                    SimpleHeader::CONTENT_LENGTH,
                    inner
                        .len()
                        .try_into_string()
                        .map_err(SimpleRequestError::StringConversion)?,
                );
            }
            _ => {}
        };

        Ok(SimpleIncomingRequest {
            body: Some(body),
            request_url,
            method,
            headers,
        })
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
        value.into()
    }
}

impl From<FromUtf8Error> for Http11RenderError {
    fn from(value: FromUtf8Error) -> Self {
        value.into()
    }
}

impl From<FromUtf16Error> for Http11RenderError {
    fn from(value: FromUtf16Error) -> Self {
        value.into()
    }
}

impl std::error::Error for Http11RenderError {}

impl core::fmt::Display for Http11RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Http11ReqState is an interesting pattern I am playing with
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
/// `Http11RequestIterator` can swap out the state and use this to decide
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
    /// go to the End vairant or the BodyStreaming variant.
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
    BodyStreaming(Option<ClonableVecIterator<BoxedError>>),

    /// ChunkedBodyStreaming like BodyStreaming is meant to support
    /// handling of a chunked body parts where
    ChunkedBodyStreaming(Option<ChunkedClonableVecIterator<BoxedError>>),

    /// The final state of the rendering which once read ends the iterator.
    End,
}

impl Clone for Http11ReqState {
    fn clone(&self) -> Self {
        match self {
            Self::Intro(inner) => Self::Intro(inner.clone()),
            Self::Headers(inner) => Self::Headers(inner.clone()),
            Self::Body(inner) => Self::Body(inner.clone()),
            Self::BodyStreaming(inner) => match inner {
                Some(inner2) => Self::BodyStreaming(Some(inner2.clone_box())),
                None => Self::BodyStreaming(None),
            },
            Self::ChunkedBodyStreaming(inner) => match inner {
                Some(inner2) => Self::ChunkedBodyStreaming(Some(inner2.clone_box())),
                None => Self::ChunkedBodyStreaming(None),
            },
            Self::End => Self::End,
        }
    }
}

/// `Http11RequestIterator` represents the rendering of a `HTTP`
/// request via an Iterator pattern that supports both sync and async
/// contexts.
pub struct Http11RequestIterator(Http11ReqState);

impl Clone for Http11RequestIterator {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Iterator for Http11RequestIterator {
    type Item = Result<Vec<u8>, Http11RenderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.clone() {
            Http11ReqState::Intro(request) => {
                // switch state to headers
                self.0 = Http11ReqState::Headers(request.clone());

                // generate HTTP 1.1 intro
                let http_intro_string =
                    format!("{} {} HTTP/1.1\r\n", request.method, request.request_url);

                Some(Ok(http_intro_string.into_bytes()))
            }
            Http11ReqState::Headers(request) => {
                // HTTP 1.1 requires atleast 1 header in the request being generated
                if request.headers.is_empty() {
                    // tell the iterator we want it to end
                    self.0 = Http11ReqState::End;

                    return Some(Err(Http11RenderError::HeadersRequired));
                }

                // switch state to body rendering next
                self.0 = Http11ReqState::Body(request.clone());

                let borrowed_headers = &request.headers;

                let mut encoded_headers: Vec<String> = borrowed_headers
                    .into_iter()
                    .map(|(key, value)| format!("{}: {}\r\n", key, value))
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // join all intermediate with CLRF (last
                // element does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("").into_bytes()))
            }
            Http11ReqState::Body(mut request) => {
                if request.body.is_none() {
                    // tell the iterator we want it to end
                    self.0 = Http11ReqState::End;

                    return Some(Err(Http11RenderError::InvalidSituationUsedIterator));
                }

                let body = request.body.take().unwrap();
                match body {
                    SimpleBody::None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(b"".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(inner.into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(inner.to_vec()))
                    }
                    SimpleBody::ChunkedStream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(inner) => {
                                self.0 = Http11ReqState::ChunkedBodyStreaming(Some(inner));
                                Some(Ok(b"".to_vec()))
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ReqState::End;
                                Some(Ok(b"\r\n".to_vec()))
                            }
                        }
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(inner) => {
                                self.0 = Http11ReqState::BodyStreaming(Some(inner));
                                Some(Ok(b"".to_vec()))
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ReqState::End;
                                Some(Ok(b"\r\n".to_vec()))
                            }
                        }
                    }
                }
            }
            Http11ReqState::ChunkedBodyStreaming(container) => {
                match container {
                    Some(mut body_iterator) => {
                        match body_iterator.next() {
                            Some(collected) => match collected {
                                Ok(mut inner) => {
                                    self.0 =
                                        Http11ReqState::ChunkedBodyStreaming(Some(body_iterator));
                                    Some(Ok(inner.into_bytes()))
                                }
                                Err(err) => {
                                    // tell the iterator we want it to end
                                    self.0 = Http11ReqState::End;
                                    Some(Err(err.into()))
                                }
                            },
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ReqState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                    None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(b"".to_vec()))
                    }
                }
            }
            Http11ReqState::BodyStreaming(container) => {
                match container {
                    Some(mut body_iterator) => {
                        match body_iterator.next() {
                            Some(collected) => match collected {
                                Ok(inner) => {
                                    self.0 = Http11ReqState::BodyStreaming(Some(body_iterator));
                                    Some(Ok(inner))
                                }
                                Err(err) => {
                                    // tell the iterator we want it to end
                                    self.0 = Http11ReqState::End;
                                    Some(Err(err.into()))
                                }
                            },
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ReqState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                    None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(b"".to_vec()))
                    }
                }
            }

            // Ends the iterator
            Http11ReqState::End => return None,
        }
    }
}

/// State representing the varying rendering status of a http response into
/// the final HTTP message.
pub enum Http11ResState {
    Intro(SimpleOutgoingResponse),
    Headers(SimpleOutgoingResponse),
    Body(SimpleOutgoingResponse),
    BodyStreaming(Option<ClonableVecIterator<BoxedError>>),
    ChunkedBodyStreaming(Option<ChunkedClonableVecIterator<BoxedError>>),
    End,
}

impl Clone for Http11ResState {
    fn clone(&self) -> Self {
        match self {
            Self::Intro(inner) => Self::Intro(inner.clone()),
            Self::Headers(inner) => Self::Headers(inner.clone()),
            Self::Body(inner) => Self::Body(inner.clone()),
            Self::BodyStreaming(inner) => match inner {
                Some(inner2) => Self::BodyStreaming(Some(inner2.clone_box())),
                None => Self::BodyStreaming(None),
            },
            Self::ChunkedBodyStreaming(inner) => match inner {
                Some(inner2) => Self::ChunkedBodyStreaming(Some(inner2.clone_box())),
                None => Self::ChunkedBodyStreaming(None),
            },
            Self::End => Self::End,
        }
    }
}

pub struct Http11ResponseIterator(Http11ResState);

impl Clone for Http11ResponseIterator {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// We want to implement an iterator that generates valid HTTP response
/// message like:
///
///   HTTP/1.1 200 OK
///   Date: Sun, 10 Oct 2010 23:26:07 GMT
///   Server: Apache/2.2.8 (Ubuntu) mod_ssl/2.2.8 OpenSSL/0.9.8g
///   Last-Modified: Sun, 26 Sep 2010 22:04:35 GMT
///   ETag: "45b6-834-49130cc1182c0"
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
        match self.0.clone() {
            Http11ResState::Intro(response) => {
                // switch state to headers
                self.0 = Http11ResState::Headers(response.clone());

                // generate HTTP 1.1 intro
                let http_intro_string = format!("HTTP/1.1 {}\r\n", response.status.status_line());

                Some(Ok(http_intro_string.into_bytes()))
            }
            Http11ResState::Headers(response) => {
                // HTTP 1.1 requires atleast 1 header in the response being generated
                if response.headers.is_empty() {
                    // tell the iterator we want it to end
                    self.0 = Http11ResState::End;

                    return Some(Err(Http11RenderError::HeadersRequired));
                }

                // switch state to body rendering next
                self.0 = Http11ResState::Body(response.clone());

                let borrowed_headers = &response.headers;

                let mut encoded_headers: Vec<String> = borrowed_headers
                    .into_iter()
                    .map(|(key, value)| format!("{}: {}\r\n", key, value))
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // join all intermediate with CLRF (last element
                // does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("").into_bytes()))
            }
            Http11ResState::Body(mut response) => {
                if response.body.is_none() {
                    // tell the iterator we want it to end
                    self.0 = Http11ResState::End;

                    return Some(Err(Http11RenderError::InvalidSituationUsedIterator));
                }

                let body = response.body.take().unwrap();
                match body {
                    SimpleBody::None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(b"".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(inner.into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(inner.to_vec()))
                    }
                    SimpleBody::ChunkedStream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(inner) => {
                                self.0 = Http11ResState::ChunkedBodyStreaming(Some(inner));
                                Some(Ok(b"".to_vec()))
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ResState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(inner) => {
                                self.0 = Http11ResState::BodyStreaming(Some(inner));
                                Some(Ok(b"".to_vec()))
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ResState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                }
            }
            Http11ResState::ChunkedBodyStreaming(mut response) => {
                match response.take() {
                    Some(mut actual_iterator) => {
                        match actual_iterator.next() {
                            Some(collected) => {
                                match collected {
                                    Ok(mut chunked) => {
                                        self.0 = Http11ResState::ChunkedBodyStreaming(Some(
                                            actual_iterator,
                                        ));
                                        Some(Ok(chunked.into_bytes()))
                                    }
                                    Err(err) => {
                                        // tell the iterator we want it to end
                                        self.0 = Http11ResState::End;
                                        Some(Err(err.into()))
                                    }
                                }
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ResState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                    None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(b"".to_vec()))
                    }
                }
            }
            Http11ResState::BodyStreaming(mut response) => {
                match response.take() {
                    Some(mut actual_iterator) => {
                        match actual_iterator.next() {
                            Some(collected) => match collected {
                                Ok(inner) => {
                                    self.0 = Http11ResState::BodyStreaming(Some(
                                        actual_iterator.clone_box(),
                                    ));

                                    Some(Ok(inner))
                                }
                                Err(err) => {
                                    // tell the iterator we want it to end
                                    self.0 = Http11ResState::End;
                                    Some(Err(err.into()))
                                }
                            },
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ResState::End;
                                Some(Ok(b"".to_vec()))
                            }
                        }
                    }
                    None => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(b"".to_vec()))
                    }
                }
            }

            // Ends the iterator
            Http11ResState::End => return None,
        }
    }
}

pub enum Http11 {
    Request(SimpleIncomingRequest),
    Response(SimpleOutgoingResponse),
}

impl Http11 {
    pub fn request(req: SimpleIncomingRequest) -> Self {
        Self::Request(req)
    }

    pub fn response(res: SimpleOutgoingResponse) -> Self {
        Self::Response(res)
    }
}

impl RenderHttp for Http11 {
    type Error = Http11RenderError;

    fn http_render(
        &self,
    ) -> std::result::Result<WrappedIterator<Result<Vec<u8>, Self::Error>>, Self::Error> {
        match self {
            Http11::Request(request) => Ok(WrappedIterator::new(Box::new(Http11RequestIterator(
                Http11ReqState::Intro(request.clone()),
            )))),
            Http11::Response(response) => Ok(WrappedIterator::new(Box::new(
                Http11ResponseIterator(Http11ResState::Intro(response.clone())),
            ))),
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
                .with_url("/")
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
                .with_url("/")
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
                .with_status(Status::Custom(666, "Custom status"))
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

pub type SimpleHttpResult<T> = std::result::Result<T, SimpleHttpError>;

#[derive(From, Debug)]
pub enum SimpleHttpError {
    NoRouteProvided,
}

impl std::error::Error for SimpleHttpError {}

impl core::fmt::Display for SimpleHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct SimpleResponse<T>(Status, SimpleHeaders, T);

impl SimpleResponse<()> {
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

pub type SimpleResponseFunc<T> = Box<dyn ClonableFnMut<SimpleIncomingRequest, SimpleResponse<T>>>;

pub fn default_response(_: SimpleIncomingRequest) -> SimpleResponse<()> {
    return SimpleResponse::no_body(Status::OK, BTreeMap::new());
}

#[derive(Clone)]
pub enum SimpleActionResponseBuilder {
    Empty(SimpleResponseFunc<()>),
    String(SimpleResponseFunc<String>),
    Bytes(SimpleResponseFunc<Vec<u8>>),
    Stream(SimpleResponseFunc<ClonableVecIterator<BoxedError>>),
}

impl SimpleActionResponseBuilder {
    pub fn no_content() -> Self {
        Self::Empty(Box::new(default_response))
    }

    pub fn empty(
        func: impl Fn(SimpleIncomingRequest) -> SimpleResponse<()> + Send + Clone + 'static,
    ) -> Self {
        Self::Empty(Box::new(func))
    }

    pub fn string(
        func: impl Fn(SimpleIncomingRequest) -> SimpleResponse<String> + Send + Clone + 'static,
    ) -> Self {
        Self::String(Box::new(func))
    }

    pub fn bytes(
        func: impl Fn(SimpleIncomingRequest) -> SimpleResponse<Vec<u8>> + Send + Clone + 'static,
    ) -> Self {
        Self::Bytes(Box::new(func))
    }

    pub fn stream(
        func: impl Fn(SimpleIncomingRequest) -> SimpleResponse<ClonableVecIterator<BoxedError>>
            + Clone
            + Send
            + 'static,
    ) -> Self {
        Self::Stream(Box::new(func))
    }
}

#[derive(From, Debug)]
pub enum HttpReaderError {
    #[from(ignore)]
    InvalidLine(String),

    #[from(ignore)]
    UnknownLine(String),

    #[from(ignore)]
    BodyBuildFailed(BoxedError),

    InvalidChunkSize,

    ExpectedSizedBodyViaContentLength,

    ReadFailed,

    LineReadFailed(BoxedError),

    InvalidContentSizeValue(Box<std::num::ParseIntError>),

    GuardedResourceAccess,
}

impl std::error::Error for HttpReaderError {}

impl core::fmt::Display for HttpReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub enum IncomingRequestParts {
    Intro(SimpleMethod, SimpleUrl),
    Headers(SimpleHeaders),
    Body(SimpleBody),
}

impl core::fmt::Display for IncomingRequestParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Intro(method, url) => write!(f, "Intro({:?}, {:?})", method, url),
            Self::Headers(headers) => write!(f, "Headers({:?})", headers),
            Self::Body(_) => write!(f, "Body(_)"),
        }
    }
}

impl Clone for IncomingRequestParts {
    fn clone(&self) -> Self {
        match self {
            Self::Intro(method, url) => Self::Intro(method.clone(), url.clone()),
            Self::Headers(headers) => Self::Headers(headers.clone()),
            Self::Body(body) => Self::Body(body.clone()),
        }
    }
}

pub type SharedTCPStream = std::sync::Arc<std::sync::Mutex<io::BufReader<TcpStream>>>;

pub type BodySize = u64;
pub type TransferEncodng = String;

#[derive(Clone, Debug)]
pub enum Body {
    LimitedBody(BodySize, SimpleHeaders),
    ChunkedBody(TransferEncodng, SimpleHeaders),
}

pub trait BodyExtractor {
    /// extract will attempt to extract the relevant Body of a TcpStream shared
    /// stream by doing whatever internal logic is required to extract the necessary
    /// tcp body content required.
    ///
    /// This allows custom implementation of Tcp/Http body extractors.
    ///
    /// See sample implementation in `SimpleHttpBody`.
    fn extract(&self, body: Body, stream: SharedTCPStream) -> Result<SimpleBody, BoxedError>;
}

#[derive(Clone, Debug)]
pub enum HttpReadState {
    Intro,
    Headers,
    Body(Body),
    NoBody,
    Finished,
}

#[derive(Clone)]
pub struct HttpReader<F: BodyExtractor> {
    reader: SharedTCPStream,
    state: HttpReadState,
    bodies: F,
}

impl<F> HttpReader<F>
where
    F: BodyExtractor,
{
    pub fn new(reader: io::BufReader<TcpStream>, bodies: F) -> Self {
        Self {
            bodies,
            state: HttpReadState::Intro,
            reader: std::sync::Arc::new(std::sync::Mutex::new(reader)),
        }
    }
}

impl<F> Iterator for HttpReader<F>
where
    F: BodyExtractor,
{
    type Item = Result<IncomingRequestParts, HttpReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.state {
            HttpReadState::Intro => {
                let mut line = String::new();
                let mut borrowed_reader = match self.reader.try_lock() {
                    Ok(borrowed_reader) => borrowed_reader,
                    Err(_) => return Some(Err(HttpReaderError::GuardedResourceAccess)),
                };

                let line_read_result = borrowed_reader
                    .read_line(&mut line)
                    .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

                if line_read_result.is_err() {
                    self.state = HttpReadState::Finished;
                    return Some(Err(line_read_result.unwrap_err()));
                }

                let intro_parts: Vec<&str> = line.split_whitespace().collect();

                // if the lines is more than two then this is not
                // allowed or wanted, so fail immediately.
                if intro_parts.len() > 2 {
                    self.state = HttpReadState::Finished;
                    return Some(Err(HttpReaderError::InvalidLine(line.clone())));
                }

                self.state = HttpReadState::Headers;

                Some(Ok(IncomingRequestParts::Intro(
                    SimpleMethod::from(intro_parts[0].to_string()),
                    SimpleUrl::url_with_query(intro_parts[1].to_string()),
                )))
            }
            HttpReadState::Headers => {
                let mut headers: SimpleHeaders = BTreeMap::new();

                let mut line = String::new();
                let mut borrowed_reader = match self.reader.try_lock() {
                    Ok(borrowed_reader) => borrowed_reader,
                    Err(_) => return Some(Err(HttpReaderError::GuardedResourceAccess)),
                };

                loop {
                    let line_read_result = borrowed_reader
                        .read_line(&mut line)
                        .map_err(|err| HttpReaderError::LineReadFailed(Box::new(err)));

                    if line_read_result.is_err() {
                        self.state = HttpReadState::Finished;
                        return Some(Err(line_read_result.unwrap_err()));
                    }

                    if line == "" {
                        break;
                    }

                    let line_parts: Vec<&str> = line.splitn(2, ':').collect();
                    headers.insert(
                        SimpleHeader::from(line_parts[0].to_string()),
                        line_parts[1].to_string(),
                    );
                }

                // if its a chunked body then send and move state to chunked body state
                let transfer_encoding = headers.get(&SimpleHeader::TRANSFER_ENCODING);
                if transfer_encoding.is_some() {
                    self.state = HttpReadState::Body(Body::ChunkedBody(
                        transfer_encoding.unwrap().clone(),
                        headers.clone(),
                    ));
                    return Some(Ok(IncomingRequestParts::Headers(headers)));
                }

                // Since it does not have a TRANSFER_ENCODING header then it
                // must have a CONTENT_LENGTH
                // header.
                match headers.get(&SimpleHeader::CONTENT_LENGTH) {
                    Some(content_size_str) => match content_size_str.parse::<u64>() {
                        Ok(value) => {
                            self.state =
                                HttpReadState::Body(Body::LimitedBody(value, headers.clone()));
                            Some(Ok(IncomingRequestParts::Headers(headers)))
                        }
                        Err(err) => {
                            self.state = HttpReadState::Finished;
                            Some(Err(HttpReaderError::InvalidContentSizeValue(Box::new(err))))
                        }
                    },
                    None => {
                        self.state = HttpReadState::NoBody;
                        Some(Ok(IncomingRequestParts::Headers(headers)))
                    }
                }
            }
            HttpReadState::NoBody => {
                self.state = HttpReadState::Finished;
                Some(Ok(IncomingRequestParts::Body(SimpleBody::None)))
            }
            HttpReadState::Body(body) => {
                let cloned_stream = self.reader.clone();
                match self.bodies.extract(body.clone(), cloned_stream) {
                    Ok(generated_body_iterator) => {
                        // once we've gotten a body iterator and gives it to the user
                        // the next state is finished.
                        self.state = HttpReadState::Finished;
                        Some(Ok(IncomingRequestParts::Body(generated_body_iterator)))
                    }
                    Err(err) => {
                        self.state = HttpReadState::Finished;
                        Some(Err(HttpReaderError::BodyBuildFailed(err)))
                    }
                }
            }
            HttpReadState::Finished => return None,
        }
    }
}

#[derive(Default)]
pub struct SimpleHttpBody {}

pub type ChunkSize = u64;
pub type StartIndexForRemaining = usize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkState {
    Invalid,
    Partial,
    Complete(ChunkSize, StartIndexForRemaining),
}

impl SimpleHttpBody {
    /// Parse a buffer of bytes as a chunk size.
    ///
    /// The return value, if complete and successful, includes the index of the
    /// buffer that parsing stopped at, and the size of the following chunk.
    ///
    /// This is taking from the [httpparse](https://github.com/seanmonstar/httparse) crate.
    ///
    /// # Example
    ///
    /// ```
    /// use wirestream::simple_http::{SimpleHttpBody, ChunkState};
    ///
    /// let buf = b"4\r\nRust\r\n0\r\n\r\n";
    /// assert!(matches!(SimpleHttpBody::parse_chunk_size(buf),
    ///            Ok(ChunkState::Complete(4, 3))));
    /// ```
    pub fn parse_chunk_size(buf: &[u8]) -> Result<ChunkState, HttpReaderError> {
        const RADIX: u64 = 16;
        let mut bytes = crate::ioutils::Bytes::new(buf);
        let mut size = 0;
        let mut in_chunk_size = true;
        let mut in_ext = false;
        let mut count = 0;
        loop {
            let b = if let Some(b) = bytes.next() {
                b
            } else {
                return Ok(ChunkState::Partial);
            };

            match b {
                b'0'..=b'9' if in_chunk_size => {
                    if count > 15 {
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    count += 1;
                    if cfg!(debug_assertions) && size > (u64::MAX / RADIX) {
                        // actually unreachable!(), because count stops the loop at 15 digits before
                        // we can reach u64::MAX / RADIX == 0xfffffffffffffff, which requires 15 hex
                        // digits. This stops mirai reporting a false alarm regarding the `size *=
                        // RADIX` multiplication below.
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    size *= RADIX;
                    size += (b - b'0') as u64;
                }
                b'a'..=b'f' if in_chunk_size => {
                    if count > 15 {
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    count += 1;
                    if cfg!(debug_assertions) && size > (u64::MAX / RADIX) {
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    size *= RADIX;
                    size += (b + 10 - b'a') as u64;
                }
                b'A'..=b'F' if in_chunk_size => {
                    if count > 15 {
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    count += 1;
                    if cfg!(debug_assertions) && size > (u64::MAX / RADIX) {
                        return Err(HttpReaderError::InvalidChunkSize);
                    }
                    size *= RADIX;
                    size += (b + 10 - b'A') as u64;
                }
                b'\r' => match if let Some(b) = bytes.next() {
                    b
                } else {
                    return Ok(ChunkState::Partial);
                } {
                    b'\n' => break,
                    _ => return Err(HttpReaderError::InvalidChunkSize),
                },
                // If we weren't in the extension yet, the ";" signals its start
                b';' if !in_ext => {
                    in_ext = true;
                    in_chunk_size = false;
                }
                // "Linear white space" is ignored between the chunk size and the
                // extension separator token (";") due to the "implied *LWS rule".
                b'\t' | b' ' if !in_ext && !in_chunk_size => {}
                // LWS can follow the chunk size, but no more digits can come
                b'\t' | b' ' if in_chunk_size => in_chunk_size = false,
                // We allow any arbitrary octet once we are in the extension, since
                // they all get ignored anyway. According to the HTTP spec, valid
                // extensions would have a more strict syntax:
                //     (token ["=" (token | quoted-string)])
                // but we gain nothing by rejecting an otherwise valid chunk size.
                _ if in_ext => {}
                // Finally, if we aren't in the extension and we're reading any
                // other octet, the chunk size line is invalid!
                _ => return Err(HttpReaderError::InvalidChunkSize),
            }
        }
        Ok(ChunkState::Complete(size, bytes.pos()))
    }
}

pub struct SimpleHttpChunkIterator(String, SimpleHeaders, SharedTCPStream);

impl Clone for SimpleHttpChunkIterator {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone(), self.2.clone())
    }
}

impl Iterator for SimpleHttpChunkIterator {
    type Item = Result<Vec<u8>, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl SimpleHttpChunkIterator {
    pub fn new(transfer_encoding: String, headers: SimpleHeaders, stream: SharedTCPStream) -> Self {
        Self(transfer_encoding, headers, stream)
    }
}

impl BodyExtractor for SimpleHttpBody {
    fn extract(&self, body: Body, stream: SharedTCPStream) -> Result<SimpleBody, BoxedError> {
        match body {
            Body::LimitedBody(content_length, _) => {
                if content_length == 0 {
                    return Ok(SimpleBody::None);
                }

                let mut body_content = Vec::with_capacity(content_length as usize);
                let mut borrowed_stream = match stream.try_lock() {
                    Ok(borrowed_reader) => borrowed_reader,
                    Err(_) => return Err(Box::new(HttpReaderError::GuardedResourceAccess)),
                };

                match borrowed_stream.read_exact(&mut body_content) {
                    Ok(_) => Ok(SimpleBody::Bytes(body_content)),
                    Err(err) => Err(Box::new(err)),
                }
            }
            Body::ChunkedBody(transfer_encoding, headers) => Ok(SimpleBody::Stream(Some(
                Box::new(SimpleHttpChunkIterator(transfer_encoding, headers, stream)),
            ))),
        }
    }
}

#[cfg(test)]
mod test_simple_http_body {
    use super::*;

    #[test]
    fn test_simple_http_body_parse_chunk_size() {
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"0\r\n"),
            Ok(ChunkState::Complete(0, 3))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"12\r\nchunk"),
            Ok(ChunkState::Complete(18, 4))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"3086d\r\n"),
            Ok(ChunkState::Complete(198765, 7))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"3735AB1;foo bar*\r\n"),
            Ok(ChunkState::Complete(57891505, 18))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"3735ab1 ; baz \r\n"),
            Ok(ChunkState::Complete(57891505, 16))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"ffffffffffffffff\r\n"),
            Ok(ChunkState::Complete(u64::MAX, 18))
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"77a65\r"),
            Ok(ChunkState::Partial)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"ab"),
            Ok(ChunkState::Partial)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"567f8a\rfoo"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"567f8a\rfoo"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"567xf8a\r\n"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"1ffffffffffffffff\r\n"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"Affffffffffffffff\r\n"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
        assert!(matches!(
            SimpleHttpBody::parse_chunk_size(b"fffffffffffffffff\r\n"),
            Err(HttpReaderError::InvalidChunkSize)
        ));
    }
}

impl HttpReader<SimpleHttpBody> {
    pub fn simple_stream(reader: io::BufReader<TcpStream>) -> HttpReader<SimpleHttpBody> {
        HttpReader::<SimpleHttpBody>::new(reader, SimpleHttpBody::default())
    }
}

pub struct ServiceActionList(Vec<ServiceAction>);

impl ServiceActionList {
    pub fn get_one_matching2(
        &self,
        url: &SimpleUrl,
        method: SimpleMethod,
    ) -> Option<ServiceAction> {
        for endpoint in self.0.iter() {
            if endpoint.match_head2(url, method.clone()) {
                return Some(endpoint.clone());
            }
        }

        None
    }

    pub fn get_matching2(
        &self,
        url: &SimpleUrl,
        method: SimpleMethod,
    ) -> Option<Vec<ServiceAction>> {
        let mut matches = Vec::new();

        for endpoint in self.0.iter() {
            if !endpoint.match_head2(url, method.clone()) {
                continue;
            }
            matches.push(endpoint.clone());
        }

        if matches.len() == 0 {
            return None;
        }

        Some(matches)
    }

    pub fn get_one_matching(&self, url: &str, method: SimpleMethod) -> Option<ServiceAction> {
        for endpoint in self.0.iter() {
            if endpoint.match_head(url, method.clone()) {
                return Some(endpoint.clone());
            }
        }
        None
    }

    pub fn get_matching(&self, url: &str, method: SimpleMethod) -> Option<Vec<ServiceAction>> {
        let mut matches = Vec::new();

        for endpoint in self.0.iter() {
            if !endpoint.match_head(url, method.clone()) {
                continue;
            }
            matches.push(endpoint.clone());
        }

        if matches.len() == 0 {
            return None;
        }

        Some(matches)
    }

    pub fn new(actions: Vec<ServiceAction>) -> Self {
        Self(actions)
    }
}

pub struct ServiceAction {
    pub route: SimpleUrl,
    pub method: SimpleMethod,
    pub body: SimpleActionResponseBuilder,
    pub headers: Option<SimpleHeaders>,
}

impl Clone for ServiceAction {
    fn clone(&self) -> Self {
        Self {
            body: self.body.clone(),
            method: self.method.clone(),
            route: self.route.clone(),
            headers: self.headers.clone(),
        }
    }
}

impl ServiceAction {
    pub fn builder() -> ServiceActionBuilder {
        ServiceActionBuilder::default()
    }

    pub fn match_head2(&self, url: &SimpleUrl, method: SimpleMethod) -> bool {
        if self.method != method {
            return false;
        }

        return self.route.matches_other(&url);
    }

    pub fn match_head(&self, url: &str, method: SimpleMethod) -> bool {
        if self.method != method {
            return false;
        }

        return self.route.matches_url(&url);
    }

    pub fn extract_match(
        &self,
        url: &str,
        method: SimpleMethod,
        headers: Option<SimpleHeaders>,
    ) -> (bool, Option<BTreeMap<String, String>>) {
        if self.method != method {
            return (false, None);
        }

        let (matched_url, extracted_params) = self.route.extract_matched_url(&url);
        if !matched_url {
            return (false, None);
        }

        match (&self.headers, headers) {
            (Some(inner), Some(expected)) => {
                if inner == &expected {
                    return (matched_url, extracted_params);
                }
                return (false, None);
            }
            (Some(_), None) => (false, None),
            (None, Some(_)) => (matched_url, extracted_params),
            (None, None) => (matched_url, extracted_params),
        }
    }
}

#[derive(Default)]
pub struct ServiceActionBuilder {
    method: Option<SimpleMethod>,
    route: Option<SimpleUrl>,
    headers: Option<SimpleHeaders>,
    body: Option<SimpleActionResponseBuilder>,
}

impl ServiceActionBuilder {
    pub fn with_headers(mut self, headers: BTreeMap<SimpleHeader, String>) -> Self {
        self.headers = Some(headers);
        self
    }

    pub fn with_method(mut self, method: SimpleMethod) -> Self {
        self.method = Some(method);
        self
    }

    pub fn add_header<H: Into<SimpleHeader>, S: Into<String>>(mut self, key: H, value: S) -> Self {
        let mut headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        headers.insert(key.into(), value.into());
        self.headers = Some(headers);
        self
    }

    pub fn with_body(mut self, body: SimpleActionResponseBuilder) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_route<S: Into<String>>(mut self, route: S) -> Self {
        self.route = Some(SimpleUrl::url_with_query(route.into()));
        self
    }

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
            None => SimpleActionResponseBuilder::no_content(),
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
    use super::*;

    #[test]
    fn test_service_action_match_url_with_headers() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_method(SimpleMethod::GET)
            .build()
            .expect("should generate service action");

        let mut headers = SimpleHeaders::new();
        let _ = headers.insert(SimpleHeader::CONTENT_TYPE, "application/json".into());

        let (matched_url, params) =
            resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, Some(headers));

        assert!(matched_url);
        assert!(matches!(params, None));
    }

    #[test]
    fn test_service_action_match_url_only() {
        let resource = ServiceAction::builder()
            .with_route("/service/endpoint/v1")
            .with_method(SimpleMethod::GET)
            .build()
            .expect("should generate service action");

        let (matched_url, params) =
            resource.extract_match("/service/endpoint/v1", SimpleMethod::GET, None);

        assert!(matched_url);
        assert!(matches!(params, None));
    }
}
