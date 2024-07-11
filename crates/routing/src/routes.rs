use core::fmt;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::{
    collections::HashMap, fmt::Debug, future::Future, slice::Iter, slice::IterMut, str::FromStr,
};

use ewe_mem::accumulator::Accumulator;

use crate::requests::Request;
use crate::response::Response;
use lazy_regex::{lazy_regex, Lazy, Regex};
use thiserror::Error;

static REGEX_ONLY_CAPTURE_ROUTES: Lazy<Regex> = lazy_regex!(r#"^\(.+\)$"#);
static PARAM_REGEX_CAPTURE_ROUTES: Lazy<Regex> = lazy_regex!(r#"::\(.+\)"#);
static PARAM_ROUTE_STARTER: Lazy<Regex> = lazy_regex!(r#"^:(\w+|\d+)"#);

#[derive(Clone, Debug, Error)]
pub enum RouteOp {
    #[error("no matching route found for path: {0}")]
    NoMatchingRoute(String),

    #[error("segment was empty/None or invalid")]
    InvalidSegment,

    #[error("invalid route regex: {0}")]
    InvalidRouteRegex(regex::Error),

    #[error("invalid param fast validation matcher: {0}")]
    InvalidParamRestraintMatcher(String),

    #[error("attempting to route SegmentType::Index segments in this way is incorrect, use setIndex instead")]
    CantHandleIndexRoute,
}

impl From<regex::Error> for RouteOp {
    fn from(value: regex::Error) -> Self {
        RouteOp::InvalidRouteRegex(value)
    }
}

type RouteResult<T> = Result<T, RouteOp>;

/// ParamStaticValidation defines a set of faster restricted route segment validation types
/// a route can support that with improved performance compared to a regex.
#[derive(Clone, PartialEq, Debug)]
pub enum ParamStaticValidation {
    Numbers,
    Letters,
    Alphaneumeric,
}

impl FromStr for ParamStaticValidation {
    type Err = RouteOp;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "numbers" | "Numbers" | "NUMBERS" => Ok(ParamStaticValidation::Numbers),
            "letter" | "letters" | "LETTERS" => Ok(ParamStaticValidation::Letters),
            "alpha" | "Alpha" | "ALPHA" => Ok(ParamStaticValidation::Alphaneumeric),
            "alphaneumeric" | "Alphaneumeric" | "ALPHANEUMERIC" => {
                Ok(ParamStaticValidation::Alphaneumeric)
            }
            _ => Err(RouteOp::InvalidParamRestraintMatcher(String::from(s))),
        }
    }
}

#[derive(Clone)]
pub enum SegmentType<'a> {
    /// Static is a path which is a static string and the path
    /// must specifically match the text. It is the strongest
    /// segment type and has the highest Segment power.
    ///
    /// Segment Power:
    ///
    /// Example:
    ///
    /// ```
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::Static("users");
    /// ```
    ///
    Static(&'a str),

    /// Restricted is a path which provides a more performant
    /// restriction compared to the Regex that has a worst performance
    /// based on the regex being used.
    ///
    /// It supports only a basic 3 types: (see `RestrictedType`).
    ///
    /// - Numbers only
    /// - Letters Only
    /// - Alphaneumric (letters and numbers)
    ///
    /// Example:
    ///
    /// ```
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::Restricted("user_id", ParamStaticValidation::Letters);
    /// let route_type = SegmentType::Restricted("user_id", ParamStaticValidation::Numbers);
    /// let route_type = SegmentType::Restricted("user_id", ParamStaticValidation::Alphaneumeric);
    /// ```
    ///
    Restricted(&'a str, ParamStaticValidation),

    /// ParamRegx is a path which like a Regex path uses a regex for
    /// route segment validation but with the route segment having
    /// a name.
    ///
    /// Example:
    ///
    /// ```
    /// use regex;
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::ParamRegex("user_id", regex::Regex::new(r"([a-zA-Z]{0,4})").expect("compiles"));
    /// ```
    ///
    ParamRegex(&'a str, regex::Regex),

    /// Regex is a path which solely relies on a portion of the route to be
    /// validated with a regexp, this is like the Param but more restrictive.
    ///
    /// Example:
    ///
    /// ```
    /// use regex;
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::Regex(regex::Regex::new(r"([a-zA-Z]{0,4})").expect("compiles"));
    /// ```
    ///
    Regex(regex::Regex),

    /// Param is a named path without a validation restrictions like in the
    /// `SegmentType::Regex` or `SegmentType::Restricted` i.e /:user_id/
    ///
    /// Example:
    ///
    /// ```
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::Param(":user_id");
    /// ```
    ///
    Param(&'a str),

    /// AnyPath is when the route segment is represented with asterick(*)) but
    /// only as the end of the path. We do not support routes like for example:
    /// /users/pages/*/names (is not allowed)
    ///
    /// Only paths like: /users/pages/names/*  is possible with the final path
    /// being an asterisk.
    ///
    /// Example:
    ///
    /// ```
    /// use crate::routing::routes::*;
    ///
    /// let route_type = SegmentType::AnyPath;
    /// ```
    ///
    AnyPath,

    /// Index of any route, basically the singular slash (/).
    Index,
}

impl<'a> TryFrom<&'a str> for SegmentType<'a> {
    type Error = RouteOp;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        if REGEX_ONLY_CAPTURE_ROUTES.is_match(text) {
            return Ok(SegmentType::Regex(Regex::new(text)?));
        }

        // if we are not dealing with parametric routes treat as static.
        if !PARAM_ROUTE_STARTER.is_match(text) {
            if text == "/*" || text == "*" {
                return Ok(SegmentType::AnyPath);
            }
            if text == "/" {
                return Ok(SegmentType::Index);
            }

            return Ok(SegmentType::Static(text));
        }

        if !text.contains("::") {
            return Ok(SegmentType::Param(&text[1..]));
        }

        // we are dealing with parametric routes, so extract the first part.
        let parts: Vec<&str> = text.split("::").collect();

        let (first_part, second_part) = (parts[0], parts[2]);

        // its definitely a regex based parameteter route
        if PARAM_REGEX_CAPTURE_ROUTES.is_match(text) {
            return Ok(SegmentType::ParamRegex(
                &first_part[1..],
                Regex::new(second_part)?,
            ));
        }

        return Ok(SegmentType::Restricted(
            &first_part[1..],
            ParamStaticValidation::from_str(second_part)?,
        ));
    }
}

impl<'a> fmt::Debug for SegmentType<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(arg0) => f.debug_tuple("Static").field(arg0).finish(),
            Self::Restricted(arg0, arg1) => {
                f.debug_tuple("Restricted").field(arg0).field(arg1).finish()
            }
            Self::ParamRegex(arg0, arg1) => {
                f.debug_tuple("ParamRegex").field(arg0).field(arg1).finish()
            }
            Self::Regex(arg0) => f.debug_tuple("Regex").field(arg0).finish(),
            Self::Param(arg0) => f.debug_tuple("Param").field(arg0).finish(),
            Self::AnyPath => write!(f, "AnyPath"),
            Self::Index => write!(f, "Index"),
        }
    }
}

impl<'a> PartialEq for SegmentType<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Static(l0), Self::Static(r0)) => l0 == r0,
            (Self::Restricted(l0, l1), Self::Restricted(r0, r1)) => l0 == r0 && l1 == r1,
            (Self::ParamRegex(l0, l1), Self::ParamRegex(r0, r1)) => {
                l0 == r0 && l1.as_str() == r1.as_str()
            }
            (Self::Regex(l0), Self::Regex(r0)) => l0.as_str() == r0.as_str(),
            (Self::Param(l0), Self::Param(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl<'a> Eq for SegmentType<'a> {}

impl<'a> SegmentType<'a> {
    pub fn priority(&self) -> usize {
        match self {
            SegmentType::Index => 7,
            SegmentType::Static(_) => 6,
            SegmentType::Restricted(_, _) => 5,
            SegmentType::ParamRegex(_, _) => 4,
            SegmentType::Regex(_) => 3,
            SegmentType::Param(_) => 2,
            SegmentType::AnyPath => 1,
        }
    }
}

// // /// Servicer defines the expectation for the type of trait and functions
// // /// which will be used to handle requests.
// type Servicer<R: Send + Clone + 'static, S: Send + Clone + 'static> =
//     dyn FnMut(Request<R>) -> dyn Future<Output = Response<S>>;

// type BoxServicer<R: Send + Clone + 'static, S: Send + Clone + 'static> = Box<Servicer<R, S>>;

/// Servicer defines the expectation for the type of trait and functions
/// which will be used to handle requests.
pub trait Servicer<R: Send + Clone + 'static, S: Send + Clone + 'static>:
    Send + Clone + 'static
{
    fn serve<FT>(&self, req: Request<R>) -> FT
    where
        FT: Future<Output = Response<S>> + Send + 'static;
}

#[derive(Error, Debug, Clone)]
pub enum MethodError {
    #[error("unknown http method: {0}")]
    Unknown(String),

    #[error("unknown http::Method: {0}")]
    UnknownMethod(http::Method),
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

/// RouteMethod defines all possible route servers a route-segment might
/// will have allow you to define per HTTP method and a custom method
/// what `Servicer` should handle the incoming request.
pub struct RouteMethod<R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>>
{
    /// GET HTTP method servicer.
    get: Option<Server>,

    /// PATCH HTTP method servicer.
    patch: Option<Server>,

    /// HEAD HTTP method servicer.
    head: Option<Server>,

    /// TRACE HTTP method servicer.
    trace: Option<Server>,

    /// CONNECT HTTP method servicer.
    connect: Option<Server>,

    /// OPTIONS HTTP method servicer.
    options: Option<Server>,

    /// PUT HTTP method servicer.
    put: Option<Server>,

    /// POST HTTP method servicer.
    post: Option<Server>,

    /// DELETE HTTP method servicer.
    delete: Option<Server>,

    /// Custom HTTP method that we could come up with.
    custom: Option<(String, Option<Server>)>,

    _request: PhantomData<R>,
    _response: PhantomData<S>,
}

impl<R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>>
    RouteMethod<R, S, Server>
{
    pub fn empty() -> Self {
        Self {
            get: None,
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn connect(server: Server) -> Self {
        Self {
            connect: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            options: None,
            head: None,
            trace: None,
            get: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn head(server: Server) -> Self {
        Self {
            head: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            options: None,
            connect: None,
            trace: None,
            get: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn trace(server: Server) -> Self {
        Self {
            trace: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            options: None,
            connect: None,
            head: None,
            get: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn options(server: Server) -> Self {
        Self {
            options: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            get: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn patch(server: Server) -> Self {
        Self {
            patch: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            options: None,
            trace: None,
            connect: None,
            head: None,
            get: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn get(server: Server) -> Self {
        Self {
            get: Some(server),
            put: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn custom(method: String, server: Server) -> Self {
        Self {
            custom: Some((method, Some(server))),
            get: None,
            delete: None,
            put: None,
            post: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn post(server: Server) -> Self {
        Self {
            post: Some(server),
            get: None,
            delete: None,
            put: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn delete(server: Server) -> Self {
        Self {
            delete: Some(server),
            get: None,
            post: None,
            put: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn put(server: Server) -> Self {
        Self {
            put: Some(server),
            get: None,
            post: None,
            delete: None,
            custom: None,
            patch: None,
            trace: None,
            connect: None,
            head: None,
            options: None,
            _request: PhantomData::default(),
            _response: PhantomData::default(),
        }
    }

    pub fn get_method(&self, method: Method) -> Result<&Option<Server>, MethodError> {
        match method {
            Method::CONNECT => Ok(&self.connect),
            Method::PUT => Ok(&self.put),
            Method::GET => Ok(&self.get),
            Method::POST => Ok(&self.post),
            Method::HEAD => Ok(&self.head),
            Method::PATCH => Ok(&self.patch),
            Method::TRACE => Ok(&self.trace),
            Method::DELETE => Ok(&self.delete),
            Method::OPTIONS => Ok(&self.options),
            Method::CUSTOM(wanted) => match &self.custom {
                Some((actual, container)) => {
                    if *actual == wanted {
                        return Ok(&container);
                    }
                    return Err(MethodError::Unknown(wanted));
                }
                None => Err(MethodError::Unknown(wanted)),
            },
        }
    }

    /// take customizes existing methods by replacing all
    /// relevant methods that the `other` (`RouteMethod`)
    /// has filled into it's relevant fields.
    ///
    /// So even if the current `RouteMethod` has `Servicer`
    /// in all relevant http methods they will be replaced
    /// as far as the `other` has those methods filled.
    ///
    pub fn take(&mut self, other: RouteMethod<R, S, Server>) {
        if other.custom.is_some() {
            self.custom = other.custom;
        }
        if other.get.is_some() {
            self.get = other.get;
        }
        if other.put.is_some() {
            self.put = other.put;
        }
        if other.delete.is_some() {
            self.delete = other.delete;
        }
        if other.post.is_some() {
            self.post = other.post;
        }
        if other.patch.is_some() {
            self.patch = other.patch;
        }
        if other.trace.is_some() {
            self.trace = other.trace;
        }
        if other.connect.is_some() {
            self.connect = other.connect;
        }
        if other.head.is_some() {
            self.head = other.head;
        }
        if other.options.is_some() {
            self.options = other.options;
        }
    }
}

/// RouteSegments are unique and very special as they repesent a route segment
/// within a change of routes and sub-routes, where each can point to
/// a section with sub-sections.
///
/// This means a routes like:
/// -	/v1/apples/:id/seeds
/// -	/v1/oranges/:id/seeds
///
/// Becomes a a segment of:
///
/// RouteSegment(v1) ->
/// 	SubSegments -> [
/// 		RouteSegment(apples, [RouteSegment(:id, [RouteSegment(seeds)])]),
/// 		RouteSegment(oranges, [RouteSegment(:id, [RouteSegment(seeds)])]),
/// 	]
///
/// This means each route segment gets broken down and nested appropriately allowing
/// relevant drill down into the relevant subparts to match the route.
///
/// You will not be dierctly interacting with a `RouteSegment` directly and mostly
/// will only interact with the Router which encapsulates usage of the `RouteSegment`
/// internally.
pub struct RouteSegment<
    'a,
    R: Send + Clone + 'static,
    S: Send + Clone + 'static,
    Server: Servicer<R, S>,
> {
    segment: SegmentType<'a>,
    dynamic_routes: Vec<RouteSegment<'a, R, S, Server>>,
    static_routes: HashMap<&'a str, RouteSegment<'a, R, S, Server>>,
    method: RouteMethod<R, S, Server>,
    _req: PhantomData<R>,
    _res: PhantomData<S>,
}

impl<'a, R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>> fmt::Debug
    for RouteSegment<'a, R, S, Server>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteSegment")
            .field("segment", &self.segment)
            .field("dynamic_routes", &self.dynamic_routes)
            .field("static_fields", &self.static_routes)
            .finish()
    }
}

impl<'a, R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>> Eq
    for RouteSegment<'a, R, S, Server>
{
}

impl<'a, R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>> PartialEq
    for RouteSegment<'a, R, S, Server>
{
    fn eq(&self, other: &Self) -> bool {
        self.segment == other.segment && self.dynamic_routes == other.dynamic_routes
    }
}

/// sort_segment is a simplistic algorithmn that takes a giving two RouteSegment returning
///	`Ordering` indicators for how they should be ordered.
fn sort_segments<
    'a,
    R: Send + Clone + 'static,
    S: Send + Clone + 'static,
    Server: Servicer<R, S>,
>(
    left: &RouteSegment<'a, R, S, Server>,
    right: &RouteSegment<'a, R, S, Server>,
) -> Ordering {
    if left.segment.priority() > right.segment.priority() {
        return Ordering::Greater;
    }

    if right.segment.priority() > left.segment.priority() {
        return Ordering::Less;
    }

    match (&right.segment, &left.segment) {
        (SegmentType::Static(right), SegmentType::Static(left)) => return right.cmp(left),
        _ => Ordering::Equal,
    }
}

static SLASH: char = '/';

fn parse_route_into_segments<'a>(route: &'a str) -> RouteResult<Vec<&'a str>> {
    let mut segments = Vec::with_capacity(5);

    let target_route = if route.starts_with("/") {
        &route[1..]
    } else {
        route
    };

    let mut acc = Accumulator::new(target_route);

    while let Some(next) = acc.peek_next() {
        if next.chars().all(|t| t == SLASH) {
            acc.unpeek_next();

            match acc.take() {
                Some(content) => segments.push(content),
                None => {
                    return Err(RouteOp::InvalidSegment);
                }
            };

            acc.peek_next();
            acc.skip();
            continue;
        }
    }

    if acc.rem_len() > 0 {
        match acc.scan_remaining() {
            Some(left) => segments.push(left),
            None => return Err(RouteOp::InvalidSegment),
        };
    }

    if route.ends_with("/") {
        segments.push("/");
    }

    Ok(segments)
}

#[cfg(test)]
mod parse_route_segment_tests {
    use super::*;

    #[test]
    fn test_parsing_route_segments_with_ending_slash() {
        let result = parse_route_into_segments("/v1/users/:id/");
        assert!(matches!(result, RouteResult::Ok(_)));
        assert_eq!(result.unwrap(), vec!["v1", "users", ":id", "/"]);
    }

    #[test]
    fn test_parsing_route_segments_with_no_ending_slash() {
        let result = parse_route_into_segments("/v1/users/:id");
        assert!(matches!(result, RouteResult::Ok(_)));
        assert_eq!(result.unwrap(), vec!["v1", "users", ":id"]);
    }

    #[test]
    fn test_parsing_route_segments_with_special_segments() {
        let result = parse_route_into_segments("/v1/users/:id::numbers/:cam_id::(\\w+)/(\\d+)/*");
        assert!(matches!(result, RouteResult::Ok(_)));
        assert_eq!(
            result.unwrap(),
            vec![
                "v1",
                "users",
                ":id::numbers",
                ":cam_id::(\\w+)",
                "(\\d+)",
                "*"
            ]
        );
    }

    #[test]
    fn test_parsing_with_shorter_segments() {
        let result = parse_route_into_segments("/v1/users/:id/pages");
        assert!(matches!(result, RouteResult::Ok(_)));
        assert_eq!(result.unwrap(), vec!["v1", "users", ":id", "pages"]);
    }
}

impl<'a, R: Send + Clone + 'static, S: Send + Clone + 'static, Server: Servicer<R, S>>
    RouteSegment<'a, R, S, Server>
{
    /// Generates a route RouteSegment with all the relevant sub-routes/sub-segments added into the
    /// root segments in nested version where the last item in this segments get the RouteMethod.
    pub fn parse_route(route: &'a str, method: RouteMethod<R, S, Server>) -> RouteResult<Self> {
        let segments = parse_route_into_segments(route)?;
        ewe_logs::debug!(
            "parse_route: String Segments: {:?} from {}",
            segments,
            route
        );

        let route_segments_result: Result<Vec<RouteSegment<'a, R, S, Server>>, RouteOp> = segments
            .iter()
            .map(|t| Self::parse_first_as_segment(*t))
            .collect();

        let mut route_segments: Vec<RouteSegment<'a, R, S, Server>> = route_segments_result?;
        ewe_logs::debug!("parse_route: Route Segments: {:?}", route_segments);

        let mut method_container = Some(method);
        let mut last_leaf: Option<RouteSegment<'a, R, S, Server>> = None;

        while !route_segments.is_empty() {
            match route_segments.pop() {
                Some(mut leaf) => match last_leaf.take() {
                    Some(last_leave) => {
                        ewe_logs::debug!(
                            "parse_route: Add segment: {:?} into {:?}",
                            last_leave,
                            leaf,
                        );
                        leaf.add_route(last_leave);
                        last_leaf.replace(leaf);
                        ewe_logs::debug!("parse_route: With new last leaf {:?}", last_leaf);
                        continue;
                    }
                    None => {
                        if let Some(m) = method_container.take() {
                            leaf.method.take(m);
                        }
                        ewe_logs::debug!(
                            "parse_route: Set as last leave: {:?} into {:?}",
                            leaf,
                            last_leaf
                        );
                        last_leaf.replace(leaf);
                        continue;
                    }
                },
                None => return Err(RouteOp::InvalidSegment),
            }
        }

        match last_leaf {
            Some(root) => {
                ewe_logs::debug!("parse_route: returned root: {:?}", root);
                return Ok(root);
            }
            _ => Err(RouteOp::InvalidSegment),
        }
    }

    fn parse_first_as_segment(route_text: &'a str) -> RouteResult<Self> {
        let segment_type = SegmentType::try_from(route_text)?;
        Ok(Self::with_segment(segment_type))
    }

    pub fn with_segment(segment: SegmentType<'a>) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::empty(),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn custom(method: String, segment: SegmentType<'a>, server: Server) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::custom(method, server),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn delete(segment: SegmentType<'a>, server: Server) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::delete(server),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn put(segment: SegmentType<'a>, server: Server) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::put(server),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn post(segment: SegmentType<'a>, server: Server) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::post(server),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn empty(segment: SegmentType<'a>) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::empty(),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn get(segment: SegmentType<'a>, server: Server) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::get(server),
            _req: PhantomData::default(),
            _res: PhantomData::default(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<RouteSegment<'a, R, S, Server>> {
        self.dynamic_routes.iter_mut()
    }

    pub fn iter(&self) -> Iter<RouteSegment<'a, R, S, Server>> {
        self.dynamic_routes.iter()
    }

    /// take_routes consumes the RouteSegments routes therby moving
    /// it out of the route segments and returns all the routes
    /// as a Vec for whatever requirements you want.
    pub fn route_segments(self) -> Vec<SegmentType<'a>> {
        self.dynamic_routes
            .iter()
            .map(|item| item.segment.clone())
            .collect()
    }

    /// and_then will consume the request generating a new
    /// request instance with whatever changes the underlying function
    /// generates.
    pub fn add_then<E>(self, f: E) -> Self
    where
        E: FnOnce(Self) -> Self,
    {
        f(self)
    }

    /// add_route segement is special in that we are adding a segment to an existing
    /// route but these following specific rules.
    ///
    /// All routes are sorted based on the the following rules:
    ///
    /// 1. The highest priority segments are sorted in descending order.
    ///
    /// 2. Similiar routes segments with the same priority will be sorted in alphabetic order
    ///		in ascending order.
    ///
    /// One thing to note is that the `SegmentType::Index` is treated specially in that
    /// it won't appear in your route sub-segments but instead will have it's relevant
    /// `Servicer` used as the service for this segment itself. Which allows you to
    /// set/unset the index route handler of a given Segment.
    ///
    pub fn add_route(&mut self, mut segment: RouteSegment<'a, R, S, Server>) {
        match &segment.segment {
            SegmentType::Index => self.method.take(segment.method),
            SegmentType::Static(route) => {
                ewe_logs::debug!("Adding static route: {} with value: {:?}", route, segment);
                self.static_routes.entry(route).or_insert(segment);
                ewe_logs::debug!("Static routes: {:?}", self.static_routes);
            }
            _ => {
                self.dynamic_routes.push(segment);
                self.dynamic_routes.sort_by(sort_segments);
                self.dynamic_routes.reverse();
            }
        }
    }

    pub fn get_segment_route_mut(
        &mut self,
        segment: SegmentType<'a>,
    ) -> RouteResult<Option<&mut RouteSegment<'a, R, S, Server>>> {
        match segment {
            SegmentType::Index => Ok(Some(self)),
            SegmentType::Static(text) => {
                if !self.static_routes.contains_key(text) {
                    return Err(RouteOp::InvalidSegment);
                }
                return Ok(self.static_routes.get_mut(text));
            }
            _ => {
                for (index, subroute) in self.dynamic_routes.iter().enumerate() {
                    if subroute.segment == segment {
                        return Ok(self.dynamic_routes.get_mut(index));
                    }
                }
                return Err(RouteOp::InvalidSegment);
            }
        }
    }

    /// Looping through the routeSegments retrieves the segment that matches this given route either from
    /// the static list or the dynamic lists. It only checks at the level of this segment.
    pub fn get_segment_route(
        &self,
        segment: SegmentType<'a>,
    ) -> RouteResult<&RouteSegment<'a, R, S, Server>> {
        match segment {
            SegmentType::Index => Ok(self),
            SegmentType::Static(text) => {
                if !self.static_routes.contains_key(text) {
                    return Err(RouteOp::InvalidSegment);
                }
                match self.static_routes.get(text) {
                    Some(item) => Ok(item),
                    None => Err(RouteOp::InvalidSegment),
                }
            }
            _ => {
                for (index, subroute) in self.dynamic_routes.iter().enumerate() {
                    if subroute.segment == segment {
                        return match self.dynamic_routes.get(index) {
                            Some(item) => Ok(item),
                            None => Err(RouteOp::InvalidSegment),
                        };
                    }
                }
                return Err(RouteOp::InvalidSegment);
            }
        }
    }

    /// Looping through the routeSegments retrieves the segment that matches this given route either from
    /// the static list or the dynamic lists. It only checks at the level of this segment. If not found
    /// will add the segment as an empty route.
    pub fn add_or_get_segment_route(
        &mut self,
        mut segment: SegmentType<'a>,
    ) -> RouteResult<&mut RouteSegment<'a, R, S, Server>> {
        match segment {
            SegmentType::Index => Err(RouteOp::CantHandleIndexRoute),
            SegmentType::Static(text) => {
                if !self.static_routes.contains_key(text) {
                    self.add_route(RouteSegment::empty(segment));
                }
                match self.static_routes.get_mut(text) {
                    Some(item) => Ok(item),
                    None => Err(RouteOp::InvalidSegment),
                }
            }
            _ => {
                for (index, subroute) in self.dynamic_routes.iter().enumerate() {
                    if subroute.segment == segment {
                        return match self.dynamic_routes.get_mut(index) {
                            Some(item) => Ok(item),
                            None => Err(RouteOp::InvalidSegment),
                        };
                    }
                }

                // we know its going to be a dynamic route, so lets capture its position
                let next_index = self.dynamic_routes.len();
                self.add_route(RouteSegment::empty(segment));
                match self.dynamic_routes.get_mut(next_index) {
                    Some(item) => Ok(item),
                    None => Err(RouteOp::InvalidSegment),
                }
            }
        }
    }
}

#[cfg(test)]
mod route_segment_tests {

    use super::*;

    use regex::Regex;
    use tracing_test::traced_test;

    #[derive(Clone)]
    struct MyRequest {}

    #[derive(Clone)]
    struct MyResponse {}

    #[derive(Clone)]
    struct MyServer {}

    impl Servicer<MyRequest, MyResponse> for MyServer {
        fn serve<FT>(&self, req: Request<MyRequest>) -> FT
        where
            FT: Future<Output = Response<MyResponse>> + Send + 'static,
            Self: Sized,
        {
            todo!()
        }
    }

    #[test]
    fn test_route_segment_sourting_rules() {
        let mut root: RouteSegment<MyRequest, MyResponse, MyServer> =
            RouteSegment::with_segment(SegmentType::Static("v1"));

        // static routes
        let alpha = SegmentType::Static("alpha");
        root.add_route(RouteSegment::with_segment(alpha.clone()));

        let twitter = SegmentType::Static("twitter");
        root.add_route(RouteSegment::with_segment(twitter.clone()));

        let linkedln = SegmentType::Static("linkedln");
        root.add_route(RouteSegment::with_segment(linkedln.clone()));

        // Any and index routes
        let anyroute = SegmentType::AnyPath;
        root.add_route(RouteSegment::with_segment(anyroute.clone()));

        let index = SegmentType::Index;
        root.add_route(RouteSegment::with_segment(index.clone()));

        // dynamic routes
        let user_id = SegmentType::Param("user_id");
        root.add_route(RouteSegment::with_segment(user_id.clone()));

        let gen_id = SegmentType::ParamRegex("gen_id", Regex::new(r"serve_(\w+)").unwrap());
        root.add_route(RouteSegment::with_segment(gen_id.clone()));

        let gid = SegmentType::Restricted("gid", ParamStaticValidation::Numbers);
        root.add_route(RouteSegment::with_segment(gid.clone()));

        assert_eq!(root.route_segments(), vec![gid, gen_id, user_id, anyroute],);
    }

    #[traced_test]
    #[test]
    fn test_route_parsing_can_set_method_for_a_route() {
        let new_path: RouteResult<RouteSegment<MyRequest, MyResponse, MyServer>> =
            RouteSegment::parse_route("/v1/users/:id/pages", RouteMethod::get(MyServer {}));

        assert!(matches!(new_path, RouteResult::Ok(_)));

        let route = new_path.unwrap();
        assert_eq!(route.segment, SegmentType::Static("v1"));

        assert!(matches!(
            route.get_segment_route(SegmentType::Static("users")),
            RouteResult::Ok(_)
        ));

        assert!(matches!(
            route
                .get_segment_route(SegmentType::Static("users"))
                .unwrap()
                .get_segment_route(SegmentType::Param("id"))
                .unwrap()
                .get_segment_route(SegmentType::Static("pages")),
            RouteResult::Ok(_)
        ));
    }

    #[traced_test]
    #[test]
    fn test_route_parsing_can_set_multiple_method_for_a_route() {
        let new_path: RouteResult<RouteSegment<MyRequest, MyResponse, MyServer>> =
            RouteSegment::parse_route("/v1/users/:id/pages", RouteMethod::get(MyServer {}));

        assert!(matches!(new_path, RouteResult::Ok(_)));

        let route = new_path.unwrap();
        assert_eq!(route.segment, SegmentType::Static("v1"));
        assert!(matches!(
            route.get_segment_route(SegmentType::Static("users")),
            RouteResult::Ok(_)
        ));
    }
}
