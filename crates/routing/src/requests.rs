use core::fmt;
use std::{collections::HashMap, convert::Infallible, fmt::Debug, str::FromStr};

use axum::body;

use http::{
    header::{InvalidHeaderName, InvalidHeaderValue, ToStrError},
    method::InvalidMethod,
    uri::InvalidUri,
};

pub use http::{Extensions, HeaderMap, Uri, Version};
use thiserror::Error;

use crate::{field_method, field_method_as_mut, set_field_method_as_mut};

pub type RouteURL = String;

#[derive(Error, Debug, Clone)]
pub enum MethodError {
    #[error("unknown http method: {0}")]
    Unknown(String),

    #[error("unknown http::Method: {0}")]
    UnknownMethod(http::Method),

    #[error("Method has no Servicer")]
    NoServer,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Method {
    OPTIONS,
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    TRACE,
    CONNECT,
    PATCH,
    CUSTOM(String),
}

impl fmt::Debug for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OPTIONS => write!(f, "OPTIONS"),
            Self::GET => write!(f, "GET"),
            Self::POST => write!(f, "POST"),
            Self::PUT => write!(f, "PUT"),
            Self::DELETE => write!(f, "DELETE"),
            Self::HEAD => write!(f, "HEAD"),
            Self::TRACE => write!(f, "TRACE"),
            Self::CONNECT => write!(f, "CONNECT"),
            Self::PATCH => write!(f, "PATCH"),
            Self::CUSTOM(arg0) => f.debug_tuple("CUSTOM").field(arg0).finish(),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OPTIONS => write!(f, "OPTIONS"),
            Self::GET => write!(f, "GET"),
            Self::POST => write!(f, "POST"),
            Self::PUT => write!(f, "PUT"),
            Self::DELETE => write!(f, "DELETE"),
            Self::HEAD => write!(f, "HEAD"),
            Self::TRACE => write!(f, "TRACE"),
            Self::CONNECT => write!(f, "CONNECT"),
            Self::PATCH => write!(f, "PATCH"),
            Self::CUSTOM(arg0) => f.debug_tuple("CUSTOM").field(arg0).finish(),
        }
    }
}

impl Method {
    fn into_http_method(self) -> Result<http::Method, http::method::InvalidMethod> {
        match self {
            Method::CONNECT => Ok(http::Method::CONNECT),
            Method::PUT => Ok(http::Method::PUT),
            Method::GET => Ok(http::Method::GET),
            Method::POST => Ok(http::Method::POST),
            Method::HEAD => Ok(http::Method::HEAD),
            Method::PATCH => Ok(http::Method::PATCH),
            Method::TRACE => Ok(http::Method::TRACE),
            Method::DELETE => Ok(http::Method::DELETE),
            Method::OPTIONS => Ok(http::Method::OPTIONS),
            Method::CUSTOM(name) => http::Method::from_bytes(name.as_bytes()),
        }
    }
}

impl From<Method> for http::Method {
    fn from(val: Method) -> Self {
        val.into_http_method()
            .expect("should convert into http method")
    }
}

impl TryFrom<http::Method> for Method {
    type Error = MethodError;

    fn try_from(value: http::Method) -> Result<Self, Self::Error> {
        match value {
            http::Method::CONNECT => Ok(Method::CONNECT),
            http::Method::PUT => Ok(Method::PUT),
            http::Method::GET => Ok(Method::GET),
            http::Method::POST => Ok(Method::POST),
            http::Method::HEAD => Ok(Method::HEAD),
            http::Method::PATCH => Ok(Method::PATCH),
            http::Method::TRACE => Ok(Method::TRACE),
            http::Method::DELETE => Ok(Method::DELETE),
            http::Method::OPTIONS => Ok(Method::OPTIONS),
            _ => Err(MethodError::UnknownMethod(value)),
        }
    }
}

#[derive(Clone)]
pub struct RequestHead {
    // headers for the giving request/response.
    headers: HeaderMap,

    /// The HTTP method of the request.
    method: Method,

    /// The HTTP version used by the request.
    version: Version,

    /// The [target](https://datatracker.ietf.org/doc/html/rfc7230#section-5.3) of the request.
    url: Uri,

    /// Extensions related to the underlying http request.
    extensions: Extensions,

    /// The route for the giving request being
    route_path: Option<RouteURL>,
}

impl RequestHead {
    pub fn new(method: Method, version: Version, url: Uri, route_path: Option<RouteURL>) -> Self {
        Self {
            url,
            headers: HeaderMap::new(),
            extensions: Extensions::new(),
            route_path,
            method,
            version,
        }
    }

    pub fn get(url: Uri) -> Self {
        Self::new(Method::GET, Version::HTTP_11, url, None)
    }

    pub fn post(url: Uri) -> Self {
        Self::new(Method::POST, Version::HTTP_11, url, None)
    }

    pub fn put(url: Uri) -> Self {
        Self::new(Method::PUT, Version::HTTP_11, url, None)
    }

    pub fn delete(url: Uri) -> Self {
        Self::new(Method::DELETE, Version::HTTP_11, url, None)
    }

    pub fn options(url: Uri) -> Self {
        Self::new(Method::OPTIONS, Version::HTTP_11, url, None)
    }

    pub fn connect(url: Uri) -> Self {
        Self::new(Method::CONNECT, Version::HTTP_11, url, None)
    }

    pub fn patch(url: Uri) -> Self {
        Self::new(Method::PATCH, Version::HTTP_11, url, None)
    }

    pub fn custom(method_name: String, url: Uri) -> Self {
        Self::new(Method::CUSTOM(method_name), Version::HTTP_11, url, None)
    }

    /// `and_then` will consume the request head generating a returned
    /// `RequetHead` modified to the underlying desire and needs of the function provided.
    ///
    /// # Example:
    ///
    /// ```
    /// # use ewe_routing::requests::{RequestHead, Method, Version, Uri, RouteURL};
    ///
    /// let head = RequestHead::new(
    ///         Method::GET,
    ///         Version::HTTP_11,
    ///         Uri::from_static("/head/1"),
    ///         Some(String::from("/head/:id")),
    /// );
    /// ```
    ///
    pub fn add_then<F>(self, f: F) -> RequestHead
    where
        F: FnOnce(Self) -> Self,
    {
        f(self)
    }

    pub fn as_method(&self) -> http::Method {
        self.method.clone().into()
    }

    pub fn clone_method(&self) -> Method {
        self.method.clone()
    }

    field_method!(headers, HeaderMap);
    field_method_as_mut!(headers_mut, headers, HeaderMap);
    set_field_method_as_mut!(set_headers, headers, HeaderMap);

    field_method!(route_path, Option<RouteURL>);
    field_method_as_mut!(route_path_mut, route_path, Option<RouteURL>);
    set_field_method_as_mut!(set_route_path, route_path, Option<RouteURL>);

    field_method!(extensions, Extensions);
    field_method_as_mut!(extensions_mut, extensions, Extensions);
    set_field_method_as_mut!(set_extensions, extensions, Extensions);

    field_method!(url, Uri);
    field_method_as_mut!(url_mut, url, Uri);
    set_field_method_as_mut!(set_url, url, Uri);

    field_method!(method, Method);
    field_method_as_mut!(method_mut, method, Method);
    set_field_method_as_mut!(set_method, method, Method);

    field_method!(version, Version);
    field_method_as_mut!(version_mut, version, Version);
    set_field_method_as_mut!(set_version, version, Version);
}

impl fmt::Debug for RequestHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestHead")
            .field("headers", &self.headers)
            .field("method", &self.method)
            .field("version", &self.version)
            .field("target", &self.url)
            .field("extensions", &self.extensions)
            .field("route_path", &self.route_path)
            .finish()
    }
}

impl From<http::request::Parts> for RequestHead {
    fn from(value: http::request::Parts) -> Self {
        let original_route = match value.extensions.get::<RouteURL>() {
            Some(content) => String::from(&**content),
            None => String::from(value.uri.path()),
        };

        Self {
            method: value.method.try_into().expect("should map into method"),
            headers: value.headers,
            version: value.version,
            url: value.uri.clone(),
            route_path: Some(original_route),
            extensions: value.extensions.clone(),
        }
    }
}

/// Params is the list of extracted route parameters
/// that a request comes with.
pub type Params = HashMap<String, String>;

#[derive(Clone)]
pub struct Request<T, S = ()> {
    pub head: RequestHead,
    pub body: Option<T>,
    pub params: Params,
    pub state: S,
}

impl<T: fmt::Debug, S> fmt::Debug for Request<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("head", &self.head)
            .field("body", &self.body)
            .finish()
    }
}

impl Request<()> {
    pub fn nobody(head: RequestHead) -> Self {
        Request::with(None, head, Params::new())
    }
}

impl<T, S: Default> Request<T, S> {
    pub fn new(t: T, head: RequestHead) -> Self {
        Self {
            head,
            state: S::default(),
            body: Some(t),
            params: HashMap::default(),
        }
    }

    pub fn with_state(t: Option<T>, state: S, head: RequestHead, params: Params) -> Self {
        Self {
            head,
            state,
            params,
            body: t,
        }
    }

    pub fn with(t: Option<T>, head: RequestHead, params: Params) -> Self {
        Self {
            head,
            params,
            body: t,
            state: S::default(),
        }
    }

    pub fn from(t: Option<T>, head: RequestHead) -> Self {
        Self {
            head,
            body: t,
            state: S::default(),
            params: HashMap::default(),
        }
    }

    #[inline]
    pub fn from_head(head: RequestHead) -> Self {
        Self {
            head,
            body: None,
            state: S::default(),
            params: HashMap::default(),
        }
    }

    /// `and_then` will consume the request generating a new
    /// request instance with whatever changes the underlying function
    /// generates.
    pub fn add_then<F>(self, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        f(self)
    }

    /// Consumes the request creating a new request which has the body
    /// mapped to the new type using the provided function.
    ///
    pub fn map_self<F>(self, f: F) -> Request<T, S>
    where
        F: FnOnce(Request<T, S>) -> Request<T, S>,
    {
        f(Request {
            head: self.head,
            body: self.body,
            state: self.state,
            params: self.params,
        })
    }

    /// Consumes the request creating a new request which has the body
    /// mapped to the new type using the provided function.
    ///
    pub fn map<F, U>(self, f: F) -> Request<U, S>
    where
        F: FnOnce(Option<T>) -> Option<U>,
    {
        Request {
            head: self.head,
            body: f(self.body),
            state: self.state,
            params: self.params,
        }
    }

    /// `map_params` consumes this request setting the `Params` to the new
    /// value, returning a new requesting using that `Params`.
    pub fn map_params(self, p: Params) -> Request<T, S> {
        self.map_self(|mut req| {
            req.params = p;
            req
        })
    }

    /// Consumes the request, returning just the body.
    ///
    /// # Examples
    ///
    /// ```no
    /// # use routing::{Request, RequestHead, Uri};
    /// let request = Request::from_head(
    ///     RequestHead::new(
    ///         http::http::Method::GET, Version::HTTP_2,
    ///         Uri::from("/head/1"),
    ///         RouteURL("/head/:id"),
    ///     )
    /// )
    /// let body = request.into_body();
    /// assert_eq(body, None);
    /// ```
    ///
    #[inline]
    pub fn into_body(self) -> Option<T> {
        self.body
    }

    #[inline]
    pub fn into_parts(self) -> (RequestHead, Option<T>) {
        (self.head, self.body)
    }

    #[inline]
    pub fn url(&self) -> &Uri {
        &self.head.url
    }

    #[inline]
    pub fn path(&self) -> String {
        String::from(self.head.url.path())
    }

    #[inline]
    pub fn extensions(&self) -> &Extensions {
        &self.head.extensions
    }

    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.head.headers
    }

    #[inline]
    pub fn version(&self) -> Version {
        self.head.version
    }

    field_method!(state, S);
    field_method_as_mut!(state_mut, state, S);

    field_method!(params, Params);
    field_method_as_mut!(param_mut, params, Params);

    field_method!(head, RequestHead);
    field_method_as_mut!(head_mut, head, RequestHead);

    field_method!(body, Option<T>);
    field_method_as_mut!(body_mut, body, Option<T>);
}

/// `LightRequest` is a definition of request that allows these elements to be passed over
/// to WASM or any other light weight runtime environment that do not require the larger
/// content of a Request object that has more larger details.
pub struct LightRequest<T> {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<T>,
}

impl<T: fmt::Debug> fmt::Debug for LightRequest<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &self.headers)
            .field("body", &self.body)
            .finish()
    }
}

pub enum TryFromLightRequestError {
    FailedURLConversion(InvalidUri),
    FailedMethodConversion(InvalidMethod),
    FailedInvalidHeaderName(InvalidHeaderName),
    FailedInvalidHeaderValue(InvalidHeaderValue),
}

impl From<InvalidHeaderName> for TryFromLightRequestError {
    fn from(value: InvalidHeaderName) -> Self {
        TryFromLightRequestError::FailedInvalidHeaderName(value)
    }
}

impl From<InvalidHeaderValue> for TryFromLightRequestError {
    fn from(value: InvalidHeaderValue) -> Self {
        TryFromLightRequestError::FailedInvalidHeaderValue(value)
    }
}

impl From<InvalidUri> for TryFromLightRequestError {
    fn from(value: InvalidUri) -> Self {
        TryFromLightRequestError::FailedURLConversion(value)
    }
}

impl From<InvalidMethod> for TryFromLightRequestError {
    fn from(value: InvalidMethod) -> Self {
        TryFromLightRequestError::FailedMethodConversion(value)
    }
}

impl<T, S: Default> TryFrom<LightRequest<T>> for Request<T, S> {
    type Error = TryFromLightRequestError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character heders.
    ///
    /// Secondly the underlying body of the Request is also consumed by this
    /// returned `LightRequest`.
    ///
    /// WARNING: Be warned this method will panic if the method or url are invalid.
    fn try_from(value: LightRequest<T>) -> Result<Self, TryFromLightRequestError> {
        let url = value.url.clone();
        let uri = Uri::from_str(value.url.as_str())?;
        let method = http::Method::from_bytes(value.method.as_bytes())?;

        let mut head = RequestHead::new(
            method.try_into().expect("should convert into method"),
            Version::HTTP_10,
            uri,
            Some(url),
        );

        for (key, value) in &value.headers {
            head.headers.insert(
                http::HeaderName::from_str(key.as_ref())?,
                http::HeaderValue::from_str(value.as_ref())?,
            );
        }
        Ok(Request::from(value.body, head))
    }
}

#[derive(Debug)]
pub enum TryFromRequestError {
    NonASCIIHeaderValue(ToStrError),
    InfallibleError(Infallible),
    ImpossibleConversion,
}

impl From<ToStrError> for TryFromRequestError {
    fn from(value: ToStrError) -> Self {
        TryFromRequestError::NonASCIIHeaderValue(value)
    }
}

impl From<Infallible> for TryFromRequestError {
    fn from(value: Infallible) -> Self {
        TryFromRequestError::InfallibleError(value)
    }
}

impl<T, S> TryFrom<Request<T, S>> for LightRequest<T> {
    type Error = TryFromRequestError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character headers.
    ///
    /// Secondly the underlying body of the Request is also consumed by this
    /// returned `LightRequest`.
    fn try_from(value: Request<T, S>) -> Result<Self, Self::Error> {
        let mut headers = HashMap::new();

        for (keyc, value) in value.head.headers {
            match keyc {
                Some(key) => {
                    let key_name = key.to_string();
                    let value_string: String = value.to_str()?.to_owned();
                    headers.entry(key_name).and_modify(|e| *e = value_string);
                }
                None => continue,
            }
        }

        Ok(Self {
            method: value.head.method.to_string(),
            url: value.head.url.to_string(),
            body: value.body,
            headers,
        })
    }
}

pub struct TypedHttpRequest<T>(pub http::Request<T>);

impl<T> From<http::Request<T>> for TypedHttpRequest<T> {
    fn from(value: http::Request<T>) -> Self {
        Self(value)
    }
}

impl<T, S: Default> TryFrom<TypedHttpRequest<T>> for Request<T, S> {
    type Error = TryFromRequestError;

    fn try_from(value: TypedHttpRequest<T>) -> Result<Self, Self::Error> {
        let (head, body) = value.0.into_parts();
        Ok(Self {
            state: S::default(),
            head: head.into(),
            body: Some(body),
            params: HashMap::default(),
        })
    }
}

#[derive(Debug, Clone, Error)]
pub enum TryFromBodyRequestError {
    #[error("failed to convert body into type instance")]
    FailedConversion,

    #[error("Infallible error that should not occur, occured")]
    Infallible(Infallible),
}

pub trait FromBody<T> {
    fn from_body(body: axum::body::Body) -> std::result::Result<T, TryFromBodyRequestError>;
}

/// Wrapper type that allows us implement for handling `http::Request` types directly
/// having access to the `axum::body::Body` of a request.
/// This allows us convert a `http::Request`<axum::Body::Body>
/// into a Request object of type `T`.
pub struct BodyHttpRequest(pub http::Request<axum::body::Body>);

impl From<http::Request<axum::body::Body>> for BodyHttpRequest {
    fn from(value: http::Request<axum::body::Body>) -> Self {
        Self(value)
    }
}

impl<T, S> TryFrom<BodyHttpRequest> for Request<T, S>
where
    T: FromBody<T>,
    S: Default,
{
    type Error = TryFromBodyRequestError;

    fn try_from(value: BodyHttpRequest) -> Result<Self, Self::Error> {
        let (head, body) = value.0.into_parts();
        let transmuted_body = T::from_body(body)?;

        Ok(Self {
            state: S::default(),
            head: head.into(),
            body: Some(transmuted_body),
            params: HashMap::default(),
        })
    }
}

pub trait IntoBody<T, E> {
    fn into_body(body: T) -> std::result::Result<body::Body, E>;
}

pub trait IntoBytes<T, E> {
    fn into_bytes(body: T) -> std::result::Result<bytes::Bytes, E>;
}

pub trait FromBytes<T> {
    fn from_body(body: bytes::Bytes) -> std::result::Result<T, TryFromBodyRequestError>;
}

/// Wrapper type that allows us implement for handling `http::Request` types directly
/// having access to the Bytes. This allows us convert a `http::Request`<axum::Body::Body>
/// into a Request object of type `T`.
pub struct BytesHttpRequest(pub http::Request<bytes::Bytes>);

impl From<http::Request<bytes::Bytes>> for BytesHttpRequest {
    fn from(value: http::Request<bytes::Bytes>) -> Self {
        Self(value)
    }
}

impl<T, S> TryFrom<BytesHttpRequest> for Request<T, S>
where
    T: FromBytes<T>,
    S: Default,
{
    type Error = TryFromBodyRequestError;

    fn try_from(value: BytesHttpRequest) -> Result<Self, Self::Error> {
        let (head, body) = value.0.into_parts();
        let transmuted_body = T::from_body(body)?;

        Ok(Self {
            state: S::default(),
            head: head.into(),
            body: Some(transmuted_body),
            params: HashMap::default(),
        })
    }
}
