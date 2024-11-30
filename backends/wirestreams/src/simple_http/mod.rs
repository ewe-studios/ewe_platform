use clonables::{
    ClonableFnMut, ClonableStringIterator, ClonableVecIterator, WrappedClonableFnMut,
    WrappedIterator,
};
use derive_more::From;
use regex::Regex;
use std::{
    collections::BTreeMap,
    convert::Infallible,
    str::FromStr,
    string::{FromUtf16Error, FromUtf8Error},
};

pub type BoxedError = Box<dyn std::error::Error + Send>;

pub type BoxedResult<T, E> = std::result::Result<T, E>;

pub enum SimpleBody {
    None,
    Text(String),
    Bytes(Vec<u8>),
    Stream(Option<ClonableVecIterator<BoxedError>>),
}

impl Clone for SimpleBody {
    fn clone(&self) -> Self {
        match self {
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
trait RenderHttp: Send {
    type Error: From<FromUtf8Error> + From<BoxedError> + Send + 'static;

    fn http_render(
        &self,
    ) -> std::result::Result<WrappedIterator<BoxedResult<Vec<u8>, Self::Error>>, Self::Error>;

    /// http_render_encoded_string attempts to render the results of calling
    /// `RenderHttp::http_render()` as a custom encoded strings.
    fn http_render_encoded_string<E>(
        &self,
        encoder: E,
    ) -> std::result::Result<ClonableStringIterator<Self::Error>, Self::Error>
    where
        E: Fn(BoxedResult<Vec<u8>, Self::Error>) -> BoxedResult<String, Self::Error>
            + Send
            + Clone
            + 'static,
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
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl FromStr for SimpleHeader {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        match upper.as_str() {
            "ACCEPT" => Ok(Self::ACCEPT),
            "ACCEPT-CHARSET" => Ok(Self::ACCEPT_CHARSET),
            "ACCEPT-ENCODING" => Ok(Self::ACCEPT_ENCODING),
            "ACCEPT-LANGUAGE" => Ok(Self::ACCEPT_LANGUAGE),
            "ACCEPT-RANGES" => Ok(Self::ACCEPT_RANGES),
            "ACCESS-CONTROL-ALLOW-CREDENTIALS" => Ok(Self::ACCESS_CONTROL_ALLOW_CREDENTIALS),
            "ACCESS-CONTROL-ALLOW-HEADERS" => Ok(Self::ACCESS_CONTROL_ALLOW_HEADERS),
            "ACCESS-CONTROL-ALLOW-METHODS" => Ok(Self::ACCESS_CONTROL_ALLOW_METHODS),
            "ACCESS-CONTROL-ALLOW-ORIGIN" => Ok(Self::ACCESS_CONTROL_ALLOW_ORIGIN),
            "ACCESS-CONTROL-EXPOSE-HEADERS" => Ok(Self::ACCESS_CONTROL_EXPOSE_HEADERS),
            "ACCESS-CONTROL-MAX-AGE" => Ok(Self::ACCESS_CONTROL_MAX_AGE),
            "ACCESS-CONTROL-REQUEST-HEADERS" => Ok(Self::ACCESS_CONTROL_REQUEST_HEADERS),
            "ACCESS-CONTROL-REQUEST-METHOD" => Ok(Self::ACCESS_CONTROL_REQUEST_METHOD),
            "AGE" => Ok(Self::AGE),
            "ALLOW" => Ok(Self::ALLOW),
            "ALT-SVC" => Ok(Self::ALT_SVC),
            "AUTHORIZATION" => Ok(Self::AUTHORIZATION),
            "CACHE-CONTROL" => Ok(Self::CACHE_CONTROL),
            "CACHE-STATUS" => Ok(Self::CACHE_STATUS),
            "CDN-CACHE-CONTROL" => Ok(Self::CDN_CACHE_CONTROL),
            "CONNECTION" => Ok(Self::CONNECTION),
            "CONTENT-DISPOSITION" => Ok(Self::CONTENT_DISPOSITION),
            "CONTENT-ENCODING" => Ok(Self::CONTENT_ENCODING),
            "CONTENT-LANGUAGE" => Ok(Self::CONTENT_LANGUAGE),
            "CONTENT-LENGTH" => Ok(Self::CONTENT_LENGTH),
            "CONTENT-LOCATION" => Ok(Self::CONTENT_LOCATION),
            "CONTENT-RANGE" => Ok(Self::CONTENT_RANGE),
            "CONTENT-SECURITY-POLICY" => Ok(Self::CONTENT_SECURITY_POLICY),
            "CONTENT-SECURITY-POLICY-REPORT-ONLY" => Ok(Self::CONTENT_SECURITY_POLICY_REPORT_ONLY),
            "CONTENT-TYPE" => Ok(Self::CONTENT_TYPE),
            "COOKIE" => Ok(Self::COOKIE),
            "DNT" => Ok(Self::DNT),
            "DATE" => Ok(Self::DATE),
            "ETAG" => Ok(Self::ETAG),
            "EXPECT" => Ok(Self::EXPECT),
            "EXPIRES" => Ok(Self::EXPIRES),
            "FORWARDED" => Ok(Self::FORWARDED),
            "FROM" => Ok(Self::FROM),
            "HOST" => Ok(Self::HOST),
            "IF-MATCH" => Ok(Self::IF_MATCH),
            "IF-MODIFIED-SINCE" => Ok(Self::IF_MODIFIED_SINCE),
            "IF-NONE-MATCH" => Ok(Self::IF_NONE_MATCH),
            "IF-RANGE" => Ok(Self::IF_RANGE),
            "IF-UNMODIFIED-SINCE" => Ok(Self::IF_UNMODIFIED_SINCE),
            "LAST-MODIFIED" => Ok(Self::LAST_MODIFIED),
            "LINK" => Ok(Self::LINK),
            "LOCATION" => Ok(Self::LOCATION),
            "MAX-FORWARDS" => Ok(Self::MAX_FORWARDS),
            "ORIGIN" => Ok(Self::ORIGIN),
            "PRAGMA" => Ok(Self::PRAGMA),
            "PROXY-AUTHENTICATE" => Ok(Self::PROXY_AUTHENTICATE),
            "PROXY-AUTHORIZATION" => Ok(Self::PROXY_AUTHORIZATION),
            "PUBLIC-KEY-PINS" => Ok(Self::PUBLIC_KEY_PINS),
            "PUBLIC-KEY-PINS-REPORT-ONLY" => Ok(Self::PUBLIC_KEY_PINS_REPORT_ONLY),
            "RANGE" => Ok(Self::RANGE),
            "REFERER" => Ok(Self::REFERER),
            "REFERRER-POLICY" => Ok(Self::REFERRER_POLICY),
            "REFRESH" => Ok(Self::REFRESH),
            "RETRY-AFTER" => Ok(Self::RETRY_AFTER),
            "SEC-WEBSOCKET-ACCEPT" => Ok(Self::SEC_WEBSOCKET_ACCEPT),
            "SEC-WEBSOCKET-EXTENSIONS" => Ok(Self::SEC_WEBSOCKET_EXTENSIONS),
            "SEC-WEBSOCKET-KEY" => Ok(Self::SEC_WEBSOCKET_KEY),
            "SEC-WEBSOCKET-PROTOCOL" => Ok(Self::SEC_WEBSOCKET_PROTOCOL),
            "SEC-WEBSOCKET-VERSION" => Ok(Self::SEC_WEBSOCKET_VERSION),
            "SERVER" => Ok(Self::SERVER),
            "SET-COOKIE" => Ok(Self::SET_COOKIE),
            "STRICT-TRANSPORT-SECURITY" => Ok(Self::STRICT_TRANSPORT_SECURITY),
            "TE" => Ok(Self::TE),
            "TRAILER" => Ok(Self::TRAILER),
            "TRANSFER-ENCODING" => Ok(Self::TRANSFER_ENCODING),
            "UPGRADE" => Ok(Self::UPGRADE),
            "UPGRADE-INSECURE-REQUESTS" => Ok(Self::UPGRADE_INSECURE_REQUESTS),
            "USER-AGENT" => Ok(Self::USER_AGENT),
            "VARY" => Ok(Self::VARY),
            "VIA" => Ok(Self::VIA),
            "WARNING" => Ok(Self::WARNING),
            "WWW-AUTHENTICATE" => Ok(Self::WWW_AUTHENTICATE),
            "X-CONTENT-TYPE-OPTIONS" => Ok(Self::X_CONTENT_TYPE_OPTIONS),
            "X-DNS-PREFETCH-CONTROL" => Ok(Self::X_DNS_PREFETCH_CONTROL),
            "X-FRAME-OPTIONS" => Ok(Self::X_FRAME_OPTIONS),
            "X-XSS-PROTECTION" => Ok(Self::X_XSS_PROTECTION),
            _ => Ok(Self::Custom(upper)),
        }
    }
}

impl core::fmt::Display for SimpleHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(inner) => write!(f, "{}", inner),
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
}

impl core::fmt::Display for SimpleMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value())
    }
}

impl SimpleMethod {
    fn value(&self) -> &'static str {
        match self {
            SimpleMethod::GET => "GET",
            SimpleMethod::POST => "POST",
            SimpleMethod::PUT => "PUT",
            SimpleMethod::DELETE => "DELETE",
            SimpleMethod::PATCH => "PATCH",
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
}

impl core::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Status {
    /// Returns status' full description
    pub fn description(&self) -> &'static str {
        match self {
            Status::Continue => "100 Continue",
            Status::SwitchingProtocols => "101 Switching Protocols",
            Status::Processing => "102 Processing",
            Status::OK => "200 Ok",
            Status::Created => "201 Created",
            Status::Accepted => "202 Accepted",
            Status::NonAuthoritativeInformation => "203 Non Authoritative Information",
            Status::NoContent => "204 No Content",
            Status::ResetContent => "205 Reset Content",
            Status::PartialContent => "206 Partial Content",
            Status::MultiStatus => "207 Multi Status",
            Status::MultipleChoices => "300 Multiple Choices",
            Status::MovedPermanently => "301 Moved Permanently",
            Status::Found => "302 Found",
            Status::SeeOther => "303 See Other",
            Status::NotModified => "304 Not Modified",
            Status::UseProxy => "305 Use Proxy",
            Status::TemporaryRedirect => "307 Temporary Redirect",
            Status::PermanentRedirect => "308 Permanent Redirect",
            Status::BadRequest => "400 Bad Request",
            Status::Unauthorized => "401 Unauthorized",
            Status::PaymentRequired => "402 Payment Required",
            Status::Forbidden => "403 Forbidden",
            Status::NotFound => "404 Not Found",
            Status::MethodNotAllowed => "405 Method Not Allowed",
            Status::NotAcceptable => "406 Not Acceptable",
            Status::ProxyAuthenticationRequired => "407 Proxy Authentication Required",
            Status::RequestTimeout => "408 Request Timeout",
            Status::Conflict => "409 Conflict",
            Status::Gone => "410 Gone",
            Status::LengthRequired => "411 Length Required",
            Status::PreconditionFailed => "412 Precondition Failed",
            Status::PayloadTooLarge => "413 Payload Too Large",
            Status::UriTooLong => "414 URI Too Long",
            Status::UnsupportedMediaType => "415 Unsupported Media Type",
            Status::RangeNotSatisfiable => "416 Range Not Satisfiable",
            Status::ExpectationFailed => "417 Expectation Failed",
            Status::ImATeapot => "418 I'm A Teapot",
            Status::UnprocessableEntity => "422 Unprocessable Entity",
            Status::Locked => "423 Locked",
            Status::FailedDependency => "424 Failed Dependency",
            Status::UpgradeRequired => "426 Upgrade Required",
            Status::PreconditionRequired => "428 Precondition Required",
            Status::TooManyRequests => "429 Too Many Requests",
            Status::RequestHeaderFieldsTooLarge => "431 Request Header Fields Too Large",
            Status::InternalServerError => "500 Internal Server Error",
            Status::NotImplemented => "501 Not Implemented",
            Status::BadGateway => "502 Bad Gateway",
            Status::ServiceUnavailable => "503 Service Unavailable",
            Status::GatewayTimeout => "504 Gateway Timeout",
            Status::HttpVersionNotSupported => "505 Http Version Not Supported",
            Status::InsufficientStorage => "507 Insufficient Storage",
            Status::NetworkAuthenticationRequired => "511 Network Authentication Required",
        }
    }
}

/// ActUrl represents a url string and query parameters hashmap
#[derive(Clone)]
pub struct SimpleUrl {
    pub url: String,
    pub matcher: Option<regex::Regex>,
    pub params: Option<Vec<String>>,
    pub query: Option<BTreeMap<String, String>>,
}

static CAPTURE_QUERY: &'static str = r"\?.*";
static CAPTURE_PATH: &'static str = r".*\?";
static QUERY_REPLACER: &'static str = r"(?P<$p>[^//|/?]+)";
static CAPTURE_PARAM_STR: &'static str = r"\{(?P<p>([A-z|0-9|_])+)\}";
static CAPTURE_QUERY_KEY_VALUE: &'static str = r"((?P<qk>[^&]+)=(?P<qv>[^&]+))*";

impl SimpleUrl {
    pub(crate) fn new(
        request_url: String,
        matcher: regex::Regex,
        params: Vec<String>,
        query: BTreeMap<String, String>,
    ) -> SimpleUrl {
        Self {
            url: request_url,
            query: Some(query),
            params: Some(params),
            matcher: Some(matcher),
        }
    }

    /// url_only indicates you wish to represent a URL only where the Url
    /// will not have queries or parameters to be extracted.
    /// Generally you will use this on the server side when representing
    /// a request with no queries or parameters.
    pub fn url_only(request_url: String) -> SimpleUrl {
        Self {
            url: request_url,
            matcher: None,
            query: None,
            params: None,
        }
    }

    /// url_with_query is used when parsing a url with queries
    /// e.g service.com/path/{param1}/{param2}?key=value&..
    /// this will extract these out into the `SimpleUrl` constructs.
    ///
    /// This is the method to use when constructing your ServiceAction
    /// has it lets you match against specific paths, queries and parameters.
    pub fn url_with_query(request_url: String) -> SimpleUrl {
        let params = Self::capture_url_params(&request_url);
        let matcher = Self::capture_path_pattern(&request_url);
        let queries = Self::capture_query_hashmap(&request_url);
        SimpleUrl {
            url: request_url,
            query: Some(queries),
            params: Some(params),
            matcher: Some(matcher),
        }
    }

    pub fn capture_url_params(url: &str) -> Vec<String> {
        let re = Regex::new(CAPTURE_PARAM_STR).unwrap();
        re.captures_iter(url)
            .filter_map(|cap| match cap.name("p") {
                Some(p) => Some(String::from(p.as_str())),
                None => None,
            })
            .collect()
    }

    pub fn capture_path_pattern(url: &str) -> regex::Regex {
        let re = Regex::new(CAPTURE_PARAM_STR).unwrap();
        let query_regex = Regex::new(CAPTURE_QUERY).unwrap();
        let pattern = query_regex.replace(url, "");
        let pattern = re.replace_all(&pattern, QUERY_REPLACER);
        Regex::new(&pattern).unwrap()
    }

    pub fn capture_query_hashmap(url: &str) -> BTreeMap<String, String> {
        let re = Regex::new(CAPTURE_QUERY_KEY_VALUE).unwrap();
        let path_regex = Regex::new(CAPTURE_PATH).unwrap();
        let only_query_parameters = path_regex.replace(url, "");

        re.captures_iter(&only_query_parameters)
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
            .collect()
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

        let headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        let body = match self.body {
            Some(inner) => inner,
            None => SimpleBody::None,
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

        let headers = match self.headers {
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
    FailedThreadAcquire,
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
    BodyStreaming(ClonableVecIterator<BoxedError>),

    /// The final state of the rendering which once read ends the iterator.
    End,
}

impl Clone for Http11ReqState {
    fn clone(&self) -> Self {
        match self {
            Self::Intro(inner) => Self::Intro(inner.clone()),
            Self::Headers(inner) => Self::Headers(inner.clone()),
            Self::Body(inner) => Self::Body(inner.clone()),
            Self::BodyStreaming(inner) => Self::BodyStreaming(inner.clone_box()),
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
    type Item = BoxedResult<Vec<u8>, Http11RenderError>;

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
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // // add CLRF indicating headers are done
                // encoded_headers.push("\r\n".into());

                // join all intermediate with CLRF (last element does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("\r\n").into_bytes()))
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
                        Some(Ok(b"\r\n".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;
                        Some(Ok(format!("{}\r\n", inner).into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ReqState::End;

                        let mut cloned = inner.clone();
                        cloned.extend(b"\r\n");

                        Some(Ok(cloned.to_vec()))
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(mut inner) => {
                                let collected_container = inner.next();
                                self.0 = Http11ReqState::BodyStreaming(inner.clone_box());

                                match collected_container {
                                    Some(collected) => match collected {
                                        Ok(inner) => Some(Ok(inner)),
                                        Err(err) => {
                                            // tell the iterator we want it to end
                                            self.0 = Http11ReqState::End;
                                            Some(Err(err.into()))
                                        }
                                    },
                                    None => {
                                        // tell the iterator we want it to end
                                        self.0 = Http11ReqState::End;
                                        Some(Ok(b"\r\n".to_vec()))
                                    }
                                }
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
            Http11ReqState::BodyStreaming(mut request) => {
                match request.next() {
                    Some(collected) => match collected {
                        Ok(inner) => {
                            self.0 = Http11ReqState::BodyStreaming(request.clone_box());

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
                        Some(Ok(b"\r\n".to_vec()))
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
    BodyStreaming(ClonableVecIterator<BoxedError>),
    End,
}

impl Clone for Http11ResState {
    fn clone(&self) -> Self {
        match self {
            Self::Intro(inner) => Self::Intro(inner.clone()),
            Self::Headers(inner) => Self::Headers(inner.clone()),
            Self::Body(inner) => Self::Body(inner.clone()),
            Self::BodyStreaming(inner) => Self::BodyStreaming(inner.clone_box()),
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
    type Item = BoxedResult<Vec<u8>, Http11RenderError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.clone() {
            Http11ResState::Intro(response) => {
                // switch state to headers
                self.0 = Http11ResState::Headers(response.clone());

                // generate HTTP 1.1 intro
                let http_intro_string = format!("HTTP/1.1 {}\r\n", response.description());

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
                    .map(|(key, value)| format!("{}: {}", key, value))
                    .collect();

                // add CLRF for ending header
                encoded_headers.push("\r\n".into());

                // // add CLRF indicating headers are done
                // encoded_headers.push("\r\n".into());

                // join all intermediate with CLRF (last element does not get it hence why we do it above)
                Some(Ok(encoded_headers.join("\r\n").into_bytes()))
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
                        Some(Ok(b"\r\n".to_vec()))
                    }
                    SimpleBody::Text(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;
                        Some(Ok(format!("{}\r\n", inner).into_bytes()))
                    }
                    SimpleBody::Bytes(inner) => {
                        // tell the iterator we want it to end
                        self.0 = Http11ResState::End;

                        let mut cloned = inner.clone();
                        cloned.extend(b"\r\n");

                        Some(Ok(cloned.to_vec()))
                    }
                    SimpleBody::Stream(mut streamer_container) => {
                        match streamer_container.take() {
                            Some(mut inner) => {
                                let collected_container = inner.next();
                                self.0 = Http11ResState::BodyStreaming(inner.clone_box());

                                match collected_container {
                                    Some(collected) => match collected {
                                        Ok(inner) => Some(Ok(inner)),
                                        Err(err) => {
                                            // tell the iterator we want it to end
                                            self.0 = Http11ResState::End;
                                            Some(Err(err.into()))
                                        }
                                    },
                                    None => {
                                        // tell the iterator we want it to end
                                        self.0 = Http11ResState::End;
                                        Some(Ok(b"\r\n".to_vec()))
                                    }
                                }
                            }
                            None => {
                                // tell the iterator we want it to end
                                self.0 = Http11ResState::End;
                                Some(Ok(b"\r\n".to_vec()))
                            }
                        }
                    }
                }
            }
            Http11ResState::BodyStreaming(mut response) => {
                match response.next() {
                    Some(collected) => match collected {
                        Ok(inner) => {
                            self.0 = Http11ResState::BodyStreaming(response.clone_box());

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
                        Some(Ok(b"\r\n".to_vec()))
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
    ) -> std::result::Result<WrappedIterator<BoxedResult<Vec<u8>, Self::Error>>, Self::Error> {
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
    fn should_convert_to_get_request() {
        let request = Http11::request(
            SimpleIncomingRequest::builder()
                .with_url("/")
                .with_method(SimpleMethod::GET)
                .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
                .add_header(SimpleHeader::HOST, "localhost:8000")
                .build()
                .unwrap(),
        );

        assert_eq!(
            request.http_render_string().unwrap(),
            "GET / HTTP/1.1\r\nCONTENT-TYPE: application/json\r\nHOST: localhost:8000\r\n\r\n\r\n"
        );
    }

    // #[test]
    // fn should_convert_to_response_with_body() {
    //     let resource = Resource::new("/");
    //     resource.status(Status::Accepted).body("hello!");

    //     assert_eq!(resource.build_response("/"), "HTTP/1.1 202 Accepted\r\n\r\nhello!");
    // }

    // #[test]
    // fn should_allows_custom_status() {
    //     let resource = Resource::new("/");
    //     resource.custom_status(666, "The Number Of The Beast").body("hello!");

    //     assert_eq!(resource.build_response("/"), "HTTP/1.1 666 The Number Of The Beast\r\n\r\nhello!");
    // }

    // #[test]
    // fn should_overwrite_custom_status_with_status() {
    //     let resource = Resource::new("/");
    //     resource.custom_status(666, "The Number Of The Beast").status(Status::Forbidden).body("hello!");

    //     assert_eq!(resource.build_response("/"), "HTTP/1.1 403 Forbidden\r\n\r\nhello!");
    // }

    // #[test]
    // fn should_add_headers() {
    //     let resource = Resource::new("/");
    //     resource
    //         .header("Content-Type", "application/json")
    //         .body("hello!");

    //     assert_eq!(resource.build_response("/"), "HTTP/1.1 200 Ok\r\nContent-Type: application/json\r\n\r\nhello!");
    // }

    // #[test]
    // fn should_append_headers() {
    //     let resource = Resource::new("/");
    //     resource
    //         .header("Content-Type", "application/json")
    //         .header("Connection", "Keep-Alive")
    //         .body("hello!");

    //     let response = resource.build_response("/");

    //     assert!(response.contains("Content-Type: application/json\r\n"));
    //     assert!(response.contains("Connection: Keep-Alive\r\n"));
    // }
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

pub struct ServiceAction {
    pub route: SimpleUrl,
    pub headers: SimpleHeaders,
    pub body: SimpleActionResponseBuilder,
}

impl Clone for ServiceAction {
    fn clone(&self) -> Self {
        Self {
            body: self.body.clone(),
            route: self.route.clone(),
            headers: self.headers.clone(),
        }
    }
}

impl ServiceAction {
    pub fn builder() -> ServiceActionBuilder {
        ServiceActionBuilder::default()
    }
}

#[derive(Default)]
pub struct ServiceActionBuilder {
    route: Option<SimpleUrl>,
    headers: Option<SimpleHeaders>,
    body: Option<SimpleActionResponseBuilder>,
}

impl ServiceActionBuilder {
    pub fn with_headers(mut self, headers: BTreeMap<SimpleHeader, String>) -> Self {
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

        let headers = match self.headers {
            Some(inner) => inner,
            None => BTreeMap::new(),
        };

        let body = match self.body {
            Some(inner) => inner,
            None => SimpleActionResponseBuilder::no_content(),
        };

        Ok(ServiceAction {
            headers,
            route,
            body,
        })
    }
}
