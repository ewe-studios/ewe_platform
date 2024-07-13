use core::fmt;
use std::any::TypeId;
use std::cmp::Ordering;
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
pub use http::{Extensions, HeaderMap, Method, Uri, Version};
use thiserror::Error;

use crate::{field_method, field_method_as_mut, set_field_method_as_mut};

pub struct ResponseHead {
    /// The response statis.
    pub status: StatusCode,

    /// The response version.
    pub version: Version,

    /// The response headers.
    pub headers: HeaderMap,

    /// The response extensions.
    pub extensions: Extensions,
}

impl ResponseHead {
    pub fn new(
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
        extensions: Extensions,
    ) -> Self {
        Self {
            headers,
            extensions,
            status,
            version,
        }
    }

    pub fn add_then<F>(self, f: F) -> ResponseHead
    where
        F: FnOnce(Self) -> Self,
    {
        f(self)
    }

    field_method!(status, StatusCode);
    field_method_as_mut!(status_mut, status, StatusCode);

    field_method!(extensions, Extensions);
    field_method_as_mut!(extensions_mut, extensions, Extensions);

    field_method!(headers, HeaderMap);
    field_method_as_mut!(headers_mut, headers, HeaderMap);

    field_method!(version, Version);
    field_method_as_mut!(version_mut, version, Version);
}

impl fmt::Debug for ResponseHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseHead")
            .field("status", &self.status)
            .field("version", &self.version)
            .field("headers", &self.headers)
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl From<http::response::Parts> for ResponseHead {
    fn from(value: http::response::Parts) -> Self {
        Self {
            status: value.status,
            headers: value.headers,
            version: value.version,
            extensions: value.extensions.clone(),
        }
    }
}

/// LightRequest is a definition of request that allows these elements to be passed over
/// to WASM or any other light weight runtime environment that do not require the larger
/// content of a Request object that has more larger details.
pub struct LightResponse<T> {
    pub version: String,
    pub status: String,
    pub headers: HashMap<String, String>,
    pub body: Option<T>,
}

pub struct Response<T> {
    head: ResponseHead,
    body: Option<T>,
}

impl<T: Debug> fmt::Debug for Response<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("head", &self.head)
            .field("body", &self.body)
            .finish()
    }
}

impl<T> Response<T> {
    pub fn new(t: T, head: ResponseHead) -> Self {
        Self {
            head,
            body: Some(t),
        }
    }

    pub fn from(t: Option<T>, head: ResponseHead) -> Self {
        Self { head, body: t }
    }

    pub fn from_head(head: ResponseHead) -> Self {
        Self { head, body: None }
    }

    #[inline]
    pub fn into_parts(self) -> (ResponseHead, Option<T>) {
        (self.head, self.body)
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
    pub fn map<F, U>(self, f: F) -> Response<U>
    where
        F: FnOnce(Option<T>) -> Option<U>,
    {
        Response {
            head: self.head,
            body: f(self.body),
        }
    }

    /// Consumes the request, returning just the body.
    ///
    /// # Examples
    ///
    /// ```no
    /// # use routing::{Response, Response, Uri};
    /// let request = Response::from_head(
    /// 	ResponseHead::new(
    /// 		...
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
    pub fn url(&self) -> &StatusCode {
        &self.head.status
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

    field_method!(head, ResponseHead);
    field_method_as_mut!(head_mut, head, ResponseHead);
    set_field_method_as_mut!(set_head, head, ResponseHead);

    field_method!(body, Option<T>);
    field_method_as_mut!(body_mut, body, Option<T>);
    set_field_method_as_mut!(set_body, body, Option<T>);
}

pub enum TryFromLightResponseError {
    FailedURLConversion(InvalidUri),
    FailedMethodConversion(InvalidMethod),
    FailedInvalidStatusCode(InvalidStatusCode),
    FailedInvalidHeaderName(InvalidHeaderName),
    FailedInvalidHeaderValue(InvalidHeaderValue),
}

impl From<InvalidHeaderName> for TryFromLightResponseError {
    fn from(value: InvalidHeaderName) -> Self {
        TryFromLightResponseError::FailedInvalidHeaderName(value)
    }
}

impl From<InvalidHeaderValue> for TryFromLightResponseError {
    fn from(value: InvalidHeaderValue) -> Self {
        TryFromLightResponseError::FailedInvalidHeaderValue(value)
    }
}

impl From<InvalidStatusCode> for TryFromLightResponseError {
    fn from(value: InvalidStatusCode) -> Self {
        TryFromLightResponseError::FailedInvalidStatusCode(value)
    }
}

impl From<InvalidUri> for TryFromLightResponseError {
    fn from(value: InvalidUri) -> Self {
        TryFromLightResponseError::FailedURLConversion(value)
    }
}

impl From<InvalidMethod> for TryFromLightResponseError {
    fn from(value: InvalidMethod) -> Self {
        TryFromLightResponseError::FailedMethodConversion(value)
    }
}

fn get_version<'a>(text: &'a str) -> Version {
    match text {
        "HTTP/0.9" => Version::HTTP_09,
        "HTTP/1.0" => Version::HTTP_10,
        "HTTP/1.1" => Version::HTTP_11,
        "HTTP/2.0" => Version::HTTP_2,
        "HTTP/3.0" => Version::HTTP_3,
        _ => unreachable!(),
    }
}

fn get_version_text(ver: Version) -> String {
    match ver {
        Version::HTTP_09 => String::from("HTTP/0.9"),
        Version::HTTP_10 => String::from("HTTP/1.0"),
        Version::HTTP_11 => String::from("HTTP/1.1"),
        Version::HTTP_2 => String::from("HTTP/2.0"),
        Version::HTTP_3 => String::from("HTTP/3.0"),
        _ => unreachable!(),
    }
}

impl<T> TryFrom<LightResponse<T>> for Response<T> {
    type Error = TryFromLightResponseError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character heders.
    ///
    /// Secondly the underlying body of the Response is also consumed by this
    /// returned LightResponse.
    ///
    /// WARNING: Be warned this method will panic if the method or url are invalid.
    fn try_from(value: LightResponse<T>) -> Result<Self, TryFromLightResponseError> {
        let status = StatusCode::from_str(&value.status)?;
        let version = get_version(&value.version);

        let mut head = ResponseHead::new(status, version, HeaderMap::new(), Extensions::new());
        for (key, value) in value.headers.iter() {
            head.headers.insert(
                http::HeaderName::from_str(key.as_ref())?,
                http::HeaderValue::from_str(value.as_ref())?,
            );
        }
        return Ok(Response::from(value.body, head));
    }
}

pub enum TryFromResponseError {
    NonASCIIHeaderValue(ToStrError),
    InfallibleError(Infallible),
}

impl From<ToStrError> for TryFromResponseError {
    fn from(value: ToStrError) -> Self {
        TryFromResponseError::NonASCIIHeaderValue(value)
    }
}

impl From<Infallible> for TryFromResponseError {
    fn from(value: Infallible) -> Self {
        TryFromResponseError::InfallibleError(value)
    }
}

impl<T> TryFrom<Response<T>> for LightResponse<T> {
    type Error = TryFromResponseError;

    /// This implementation of is unique in that it skips any headers that
    /// to not mappable to a String for as the `HeaderMap` type actually
    /// does some underlying logic to deal with non-ASCII character heders.
    ///
    /// Secondly the underlying body of the Response is also consumed by this
    /// returned LightResponse.
    fn try_from(value: Response<T>) -> Result<Self, Self::Error> {
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
            version: get_version_text(value.head.version),
            status: String::from(value.head.status.as_str()),
            body: value.body,
            headers,
        })
    }
}

impl<T> TryFrom<http::Response<T>> for Response<T> {
    type Error = TryFromResponseError;

    fn try_from(value: http::Response<T>) -> Result<Self, Self::Error> {
        let (head, body) = value.into_parts();
        Ok(Self {
            head: head.into(),
            body: Some(body),
        })
    }
}

impl<T> Into<http::Response<T>> for Response<T> {
    fn into(self) -> http::Response<T> {
        let (mut head, body) = self.into_parts();
        let mut builder = http::Response::builder().status(head.status);
        builder.headers_mut().replace(head.headers_mut());
        builder.extensions_mut().replace(head.extensions_mut());

        builder.body(body.unwrap()).unwrap()
    }
}

pub struct ResponseResult<T, E>(pub Result<Response<T>, E>);

impl<T, E> Into<Result<http::Response<T>, E>> for ResponseResult<T, E> {
    fn into(self) -> Result<http::Response<T>, E> {
        match self.0 {
            Ok(response) => {
                let http_response: http::response::Response<T> = response.into();
                Ok(http_response)
            }
            Err(err) => Err(err),
        }
    }
}
