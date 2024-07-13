use core::fmt;
use std::any::TypeId;
use std::cmp::Ordering;
use std::hash::Hash;
use std::{
    collections::HashMap, convert::Infallible, fmt::Debug, future::Future, ops::Deref, slice::Iter,
    slice::IterMut, str::FromStr,
};

use ewe_mem::accumulator::Accumulator;

use http::{
    header::{InvalidHeaderName, InvalidHeaderValue, ToStrError},
    method::InvalidMethod,
    status::InvalidStatusCode,
    uri::InvalidUri,
    HeaderValue, StatusCode,
};
/// Implementation of routing and request/response primitives.
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

impl Into<http::Method> for Method {
    fn into(self) -> http::Method {
        self.into_http_method()
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
    target: Uri,

    /// Extensions related to the underlying http request.
    extensions: Extensions,

    /// The route for the giving request being
    route_path: RouteURL,
}

impl RequestHead {
    pub fn new(method: Method, version: Version, url: Uri, route_url: RouteURL) -> Self {
        Self {
            target: url,
            route_path: route_url,
            headers: HeaderMap::new(),
            extensions: Extensions::new(),
            method,
            version,
        }
    }

    /// and_then will consume the request head generating a returned
    /// RequetHead modified to the underlying desire and needs of the function provided.
    ///
    /// # Example:
    ///
    /// ```no
    /// # use routing::RequestHead;
    ///
    /// let head = RequestHead::new(
    /// 		http::http::Method::GET, Version::HTTP_2,
    /// 		Uri::from("/head/1"),
    /// 		RouteURL("/head/:id"),
    /// ).and_then(|h| {
    /// 	h.route_path = RouteURL("/head/:id/10");
    /// 	h
    /// });
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

    field_method!(route_path, RouteURL);
    field_method_as_mut!(route_path_mut, route_path, RouteURL);
    set_field_method_as_mut!(set_route_path, route_path, RouteURL);

    field_method!(extensions, Extensions);
    field_method_as_mut!(extensions_mut, extensions, Extensions);
    set_field_method_as_mut!(set_extensions, extensions, Extensions);

    field_method!(target, Uri);
    field_method_as_mut!(target_mut, target, Uri);
    set_field_method_as_mut!(set_target, target, Uri);

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
            .field("target", &self.target)
            .field("extensions", &self.extensions)
            .field("route_path", &self.route_path)
            .finish()
    }
}

impl From<http::request::Parts> for RequestHead {
    fn from(value: http::request::Parts) -> Self {
        let original_route = match value.extensions.get::<RouteURL>() {
            Some(content) => String::from(content.deref()),
            None => String::from(value.uri.path()),
        };

        Self {
            method: value.method.try_into().expect("should map into method"),
            headers: value.headers,
            version: value.version,
            target: value.uri.clone(),
            route_path: original_route,
            extensions: value.extensions.clone(),
        }
    }
}

/// Params is the list of extracted route parameters
/// that a request comes with.
pub type Params = HashMap<String, String>;

#[derive(Clone)]
pub struct Request<T> {
    pub head: RequestHead,
    pub body: Option<T>,
    pub params: Params,
}

impl<T: fmt::Debug> fmt::Debug for Request<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("head", &self.head)
            .field("body", &self.body)
            .finish()
    }
}

impl<T> Request<T> {
    pub fn new(t: T, head: RequestHead) -> Self {
        Self {
            head,
            body: Some(t),
            params: HashMap::default(),
        }
    }

    pub fn with(t: Option<T>, head: RequestHead, params: Params) -> Self {
        Self {
            head,
            params,
            body: t,
        }
    }

    pub fn from(t: Option<T>, head: RequestHead) -> Self {
        Self {
            head,
            body: t,
            params: HashMap::default(),
        }
    }

    #[inline]
    pub fn from_head(head: RequestHead) -> Self {
        Self {
            head,
            body: None,
            params: HashMap::default(),
        }
    }

    /// and_then will consume the request generating a new
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
    pub fn map_self<F>(self, f: F) -> Request<T>
    where
        F: FnOnce(Request<T>) -> Request<T>,
    {
        f(Request {
            head: self.head,
            body: self.body,
            params: self.params,
        })
    }

    /// Consumes the request creating a new request which has the body
    /// mapped to the new type using the provided function.
    ///
    pub fn map<F, U>(self, f: F) -> Request<U>
    where
        F: FnOnce(Option<T>) -> Option<U>,
    {
        Request {
            head: self.head,
            body: f(self.body),
            params: self.params,
        }
    }

    /// map_params consumes this request setting the `Params` to the new
    /// value, returning a new requesting using that `Params`.
    pub fn map_params(self, p: Params) -> Request<T> {
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
    /// 	RequestHead::new(
    /// 		http::http::Method::GET, Version::HTTP_2,
    /// 		Uri::from("/head/1"),
    /// 		RouteURL("/head/:id"),
    /// 	)
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
        &self.head.target
    }

    #[inline]
    pub fn path(&self) -> String {
        String::from(self.head.target.path())
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

    field_method!(params, Params);
    field_method_as_mut!(param_mut, params, Params);

    field_method!(head, RequestHead);
    field_method_as_mut!(head_mut, head, RequestHead);

    field_method!(body, Option<T>);
    field_method_as_mut!(body_mut, body, Option<T>);
}

/// LightRequest is a definition of request that allows these elements to be passed over
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

impl<T> TryFrom<LightRequest<T>> for Request<T> {
    type Error = TryFromLightRequestError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character heders.
    ///
    /// Secondly the underlying body of the Request is also consumed by this
    /// returned LightRequest.
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
            url,
        );
        for (key, value) in value.headers.iter() {
            head.headers.insert(
                http::HeaderName::from_str(key.as_ref())?,
                http::HeaderValue::from_str(value.as_ref())?,
            );
        }
        return Ok(Request::from(value.body, head));
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

impl<T> TryFrom<Request<T>> for LightRequest<T> {
    type Error = TryFromRequestError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character heders.
    ///
    /// Secondly the underlying body of the Request is also consumed by this
    /// returned LightRequest.
    fn try_from(value: Request<T>) -> Result<Self, Self::Error> {
        let mut headers = HashMap::new();

        for (keyc, value) in value.head.headers {
            match keyc {
                Some(key) => {
                    let key_name = key.to_string();
                    let value_string = String::try_from(value.to_str()?)?;
                    headers.entry(key_name).and_modify(|e| *e = value_string);
                }
                None => continue,
            }
        }

        Ok(Self {
            method: value.head.method.to_string(),
            url: value.head.target.to_string(),
            body: value.body,
            headers,
        })
    }
}

impl<T> TryFrom<http::Request<T>> for Request<T> {
    type Error = TryFromRequestError;

    fn try_from(value: http::Request<T>) -> Result<Self, Self::Error> {
        let (head, body) = value.into_parts();
        Ok(Self {
            head: head.into(),
            body: Some(body),
            params: HashMap::default(),
        })
    }
}
