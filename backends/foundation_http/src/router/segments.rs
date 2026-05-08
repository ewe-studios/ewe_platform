//! `RouteSegment` tree — migrated from `ewe_routing` with generics removed.
//!
//! The matching logic is unchanged. The only difference is that `RouteMethod`
//! now stores `ArcServe` (from method.rs) instead of a generic `Servicer`.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use foundation_core::wire::simple_http::SimpleMethod;
use regex::Regex;

use crate::serve::Serve;

use super::method::RouteMethod;

// ---------------------------------------------------------------------------
// Type aliases

/// Route parameters extracted during matching.
pub type Params = HashMap<String, String>;

/// Result type for route operations.
pub type RouteResult<T> = Result<T, RouteOp>;

/// Handler type alias.
pub type ArcServe = Arc<dyn Serve>;

// ---------------------------------------------------------------------------
// RouteOp — error type

/// Errors during route matching and registration.
#[derive(Clone, Debug)]
pub enum RouteOp {
    NoMatchingRoute(String),
    InvalidSegment,
    InvalidRouteRegex(regex::Error),
    InvalidParamRestraintMatcher(String),
    CantHandleIndexRoute,
    InvalidRootRoute,
    DidNotMatchExpected(String, String),
    PanicSegmentNotFound,
}

impl From<regex::Error> for RouteOp {
    fn from(value: regex::Error) -> Self {
        RouteOp::InvalidRouteRegex(value)
    }
}

impl std::fmt::Display for RouteOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteOp::NoMatchingRoute(p) => write!(f, "no matching route for path: {p}"),
            RouteOp::InvalidSegment => write!(f, "invalid route segment"),
            RouteOp::InvalidRouteRegex(e) => write!(f, "invalid route regex: {e}"),
            RouteOp::InvalidParamRestraintMatcher(s) => write!(f, "invalid param restraint: {s}"),
            RouteOp::CantHandleIndexRoute => write!(f, "index route must be set via set_index"),
            RouteOp::InvalidRootRoute => write!(f, "root segment does not match"),
            RouteOp::DidNotMatchExpected(a, b) => write!(f, "did not match {a} with {b}"),
            RouteOp::PanicSegmentNotFound => write!(f, "unexpected: no segment found"),
        }
    }
}

// ---------------------------------------------------------------------------
// ParamStaticValidation — restricted param types

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParamStaticValidation {
    Numbers,
    Letters,
    Alphaneumeric,
    ASCII,
}

impl std::str::FromStr for ParamStaticValidation {
    type Err = RouteOp;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ascii" | "ASCII" => Ok(ParamStaticValidation::ASCII),
            "numbers" | "Numbers" | "NUMBERS" => Ok(ParamStaticValidation::Numbers),
            "letter" | "letters" | "LETTERS" => Ok(ParamStaticValidation::Letters),
            "alpha" | "Alpha" | "ALPHA"
            | "alphaneumeric" | "Alphaneumeric" | "ALPHANEUMERIC" => {
                Ok(ParamStaticValidation::Alphaneumeric)
            }
            _ => Err(RouteOp::InvalidParamRestraintMatcher(s.into())),
        }
    }
}

// ---------------------------------------------------------------------------
// SegmentType — path segment kinds

#[derive(Clone, Debug)]
pub enum SegmentType {
    Root,
    Static(String),
    Restricted(String, ParamStaticValidation),
    ParamRegex(String, Regex),
    Regex(Regex),
    Param(String),
    AnyPath,
    Index,
}

impl PartialEq for SegmentType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SegmentType::Root, SegmentType::Root) => true,
            (SegmentType::Static(a), SegmentType::Static(b)) => a == b,
            (SegmentType::Restricted(a_name, a_val), SegmentType::Restricted(b_name, b_val)) => {
                a_name == b_name && a_val == b_val
            }
            (SegmentType::ParamRegex(a_name, _), SegmentType::ParamRegex(b_name, _)) => {
                a_name == b_name
            }
            (SegmentType::Regex(_), SegmentType::Regex(_)) => true,
            (SegmentType::Param(a), SegmentType::Param(b)) => a == b,
            (SegmentType::AnyPath, SegmentType::AnyPath) => true,
            (SegmentType::Index, SegmentType::Index) => true,
            _ => false,
        }
    }
}

impl Eq for SegmentType {}

static REGEX_ONLY_CAPTURE_ROUTES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\(.+\)$"#).unwrap());
static PARAM_REGEX_CAPTURE_ROUTES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"::\(.+\)"#).unwrap());
static PARAM_ROUTE_STARTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^:(\w+|\d+)"#).unwrap());

impl<'a> TryFrom<&'a str> for SegmentType {
    type Error = RouteOp;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        if REGEX_ONLY_CAPTURE_ROUTES.is_match(text) {
            return Ok(SegmentType::Regex(Regex::new(text)?));
        }

        if !PARAM_ROUTE_STARTER.is_match(text) {
            if text == "/*" || text == "*" {
                return Ok(SegmentType::AnyPath);
            }
            if text == "/" {
                return Ok(SegmentType::Index);
            }
            return Ok(SegmentType::Static(text.into()));
        }

        if !text.contains("::") {
            return Ok(SegmentType::Param(text[1..].into()));
        }

        let parts: Vec<&str> = text.split("::").collect();
        let (first_part, second_part) = (parts[0], parts[2]);

        if PARAM_REGEX_CAPTURE_ROUTES.is_match(text) {
            return Ok(SegmentType::ParamRegex(first_part[1..].into(), Regex::new(second_part)?));
        }

        Ok(SegmentType::Restricted(
            first_part[1..].into(),
            second_part.parse()?,
        ))
    }
}

impl SegmentType {
    pub fn priority(&self) -> usize {
        match self {
            SegmentType::Index => 7,
            SegmentType::Static(_) => 6,
            SegmentType::Restricted(_, _) => 5,
            SegmentType::ParamRegex(_, _) => 4,
            SegmentType::Regex(_) => 3,
            SegmentType::Param(_) => 2,
            SegmentType::AnyPath => 1,
            SegmentType::Root => 0,
        }
    }

    pub fn as_string(&self) -> String {
        format!("{self:?}")
    }

    fn match_value_from(&self, other: &SegmentType) -> RouteResult<Option<(String, String)>> {
        match (self, other) {
            (SegmentType::Root, _) => Ok(None),
            (SegmentType::Index, SegmentType::Index) => Ok(None),
            (SegmentType::AnyPath, SegmentType::Index) => Ok(None),
            (SegmentType::AnyPath, SegmentType::Static(_)) => Ok(None),
            (SegmentType::AnyPath, SegmentType::Param(_)) => Ok(None),
            (SegmentType::AnyPath, SegmentType::ParamRegex(_, _)) => Ok(None),
            (SegmentType::AnyPath, SegmentType::Regex(_)) => Ok(None),
            (SegmentType::AnyPath, SegmentType::Restricted(_, _)) => Ok(None),
            (SegmentType::AnyPath, SegmentType::AnyPath) => Ok(None),
            (SegmentType::Static(left), SegmentType::Static(right)) => {
                if left != right {
                    return Err(RouteOp::NoMatchingRoute(other.as_string()));
                }
                Ok(None)
            }
            (SegmentType::Regex(matcher), SegmentType::Static(right)) => {
                if !matcher.is_match(right) {
                    return Err(RouteOp::DidNotMatchExpected(self.as_string(), other.as_string()));
                }
                Ok(None)
            }
            (SegmentType::ParamRegex(param, matcher), SegmentType::Static(right)) => {
                if !matcher.is_match(right) {
                    return Err(RouteOp::DidNotMatchExpected(self.as_string(), other.as_string()));
                }
                Ok(Some((param.clone(), right.clone())))
            }
            (SegmentType::Param(key), SegmentType::Static(value)) => {
                Ok(Some((key.clone(), value.clone())))
            }
            (SegmentType::Restricted(key, restriction), SegmentType::Static(right)) => {
                let ok = match restriction {
                    ParamStaticValidation::ASCII => right.is_ascii(),
                    ParamStaticValidation::Letters => right.chars().all(char::is_alphabetic),
                    ParamStaticValidation::Numbers => right.chars().all(char::is_numeric),
                    ParamStaticValidation::Alphaneumeric => right.chars().all(char::is_alphanumeric),
                };
                if ok {
                    Ok(Some((key.clone(), right.clone())))
                } else {
                    Err(RouteOp::DidNotMatchExpected(self.as_string(), other.as_string()))
                }
            }
            _ => Err(RouteOp::NoMatchingRoute(other.as_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// RouteSegment — the tree node

#[derive(Clone)]
pub struct RouteSegment {
    segment: SegmentType,
    dynamic_routes: Vec<RouteSegment>,
    static_routes: HashMap<String, RouteSegment>,
    method: RouteMethod,
}

impl std::fmt::Debug for RouteSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSegment")
            .field("segment", &self.segment)
            .field("dynamic_routes", &self.dynamic_routes)
            .field("static_routes", &self.static_routes)
            .finish()
    }
}

fn sort_segments(left: &RouteSegment, right: &RouteSegment) -> Ordering {
    if left.segment.priority() > right.segment.priority() {
        return Ordering::Greater;
    }
    if right.segment.priority() > left.segment.priority() {
        return Ordering::Less;
    }
    match (&left.segment, &right.segment) {
        (SegmentType::Static(l), SegmentType::Static(r)) => r.cmp(l),
        _ => Ordering::Equal,
    }
}

fn parse_route_into_segments(route: &str) -> RouteResult<Vec<&str>> {
    let mut segments = Vec::with_capacity(5);
    let target_route = route.strip_prefix('/').unwrap_or(route);

    // Simple split on '/' — the original used StringPointer but split works equivalently
    for part in target_route.split('/') {
        if part.is_empty() {
            continue;
        }
        segments.push(part);
    }

    if route.ends_with('/') {
        segments.push("/");
    }

    if segments.is_empty() && (route == "/" || route.is_empty()) {
        segments.push("/");
    }

    Ok(segments)
}

impl RouteSegment {
    pub fn root() -> Self {
        Self::with_segment(SegmentType::Root)
    }

    pub fn with_segment(segment: SegmentType) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::empty(),
        }
    }

    pub fn empty(segment: SegmentType) -> Self {
        Self {
            segment,
            dynamic_routes: Vec::new(),
            static_routes: HashMap::new(),
            method: RouteMethod::empty(),
        }
    }

    // -----------------------------------------------------------------------
    // Route parsing — builds a tree from a path string

    pub fn parse_route(route: &str) -> RouteResult<Self> {
        let segments = parse_route_into_segments(route)?;

        let route_segments: Result<Vec<RouteSegment>, RouteOp> = segments
            .iter()
            .map(|t| SegmentType::try_from(*t).map(Self::with_segment))
            .collect();

        let mut route_segments = route_segments?;
        let mut last_leaf: Option<RouteSegment> = None;

        while !route_segments.is_empty() {
            match route_segments.pop() {
                Some(mut leaf) => {
                    if let Some(last) = last_leaf.take() {
                        leaf.add_route(last);
                        last_leaf.replace(leaf);
                    } else {
                        last_leaf.replace(leaf);
                    }
                }
                None => return Err(RouteOp::InvalidSegment),
            }
        }

        last_leaf.ok_or(RouteOp::InvalidSegment)
    }

    // -----------------------------------------------------------------------
    // Adding a sub-route

    pub fn add_route(&mut self, segment: RouteSegment) {
        match &segment.segment {
            SegmentType::Root => panic!("should never add root segment as a subroute"),
            SegmentType::Index => self.method.take(segment.method),
            SegmentType::Static(route) => {
                self.static_routes.entry(route.clone()).or_insert(segment);
            }
            _ => {
                self.dynamic_routes.push(segment);
                self.dynamic_routes.sort_by(sort_segments);
                self.dynamic_routes.reverse();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Merge a parsed route tree into the existing tree (used by Router::add_route)

    pub fn merge_route(&mut self, other: RouteSegment, method: SimpleMethod, handler: ArcServe) {
        // If segments match AND self is not Root, merge directly on this node
        // Root is special — its children are the first-level route segments
        if self.segment == other.segment && !matches!(&self.segment, SegmentType::Root) {
            self.method.set_method(method.clone(), handler.clone());
            // Merge children from `other` into `self`
            for (key, sub) in other.static_routes {
                self.merge_or_create_static(key, sub, &method, &handler);
            }
            for sub in other.dynamic_routes {
                self.merge_or_create_dynamic(sub, &method, &handler);
            }
            return;
        }

        match &other.segment {
            SegmentType::Root => {
                // Merge root's children into self
                for (key, sub) in other.static_routes {
                    self.merge_or_create_static(key, sub, &method, &handler);
                }
                for sub in other.dynamic_routes {
                    self.merge_or_create_dynamic(sub, &method, &handler);
                }
                // If other has an Index method set, merge it
                if other.method.has_any() {
                    self.method.set_method(method, handler);
                }
            }
            SegmentType::Index => {
                self.method.set_method(method, handler);
            }
            SegmentType::Static(key) => {
                self.merge_or_create_static(key.clone(), other, &method, &handler);
            }
            _ => {
                // Root always stores dynamic segments as children
                if matches!(&self.segment, SegmentType::Root) {
                    self.merge_or_create_dynamic(other, &method, &handler);
                } else if other.static_routes.is_empty() && other.dynamic_routes.is_empty() {
                    // Leaf dynamic segment on a non-root node — set method here
                    self.method.set_method(method, handler);
                } else {
                    self.merge_or_create_dynamic(other, &method, &handler);
                }
            }
        }
    }

    /// Set method on the innermost leaf of a parsed route tree.
    fn set_method_on_leaf(&mut self, method: SimpleMethod, handler: ArcServe) {
        // Descend into the first child (parsed routes are linear chains)
        if let Some((_, child)) = self.static_routes.iter_mut().next() {
            child.set_method_on_leaf(method, handler);
        } else if let Some(child) = self.dynamic_routes.first_mut() {
            child.set_method_on_leaf(method, handler);
        } else {
            self.method.set_method(method, handler);
        }
    }

    fn merge_or_create_static(
        &mut self,
        key: String,
        other: RouteSegment,
        method: &SimpleMethod,
        handler: &ArcServe,
    ) {
        if let Some(existing) = self.static_routes.get_mut(&key) {
            existing.merge_route(other, method.clone(), handler.clone());
        } else {
            let mut new_node = other;
            new_node.set_method_on_leaf(method.clone(), handler.clone());
            self.static_routes.insert(key, new_node);
        }
    }

    fn merge_or_create_dynamic(
        &mut self,
        other: RouteSegment,
        method: &SimpleMethod,
        handler: &ArcServe,
    ) {
        // Check if an equivalent dynamic route already exists
        let existing_idx = self.dynamic_routes.iter().position(|d| d.segment == other.segment);

        if let Some(idx) = existing_idx {
            let existing = &mut self.dynamic_routes[idx];
            existing.merge_route(other, method.clone(), handler.clone());
        } else {
            // No existing equivalent — set method on leaf and insert
            let mut new_node = other;
            new_node.set_method_on_leaf(method.clone(), handler.clone());
            self.dynamic_routes.push(new_node);
            self.dynamic_routes.sort_by(sort_segments);
            self.dynamic_routes.reverse();
        }
    }

    pub fn merge_route_all_methods(&mut self, other: RouteSegment, handler: ArcServe) {
        // Register for all standard HTTP methods
        for method in [
            SimpleMethod::GET,
            SimpleMethod::POST,
            SimpleMethod::PUT,
            SimpleMethod::DELETE,
            SimpleMethod::PATCH,
            SimpleMethod::HEAD,
            SimpleMethod::OPTIONS,
            SimpleMethod::CONNECT,
            SimpleMethod::TRACE,
        ] {
            self.merge_route(other.clone(), method, handler.clone());
        }
    }

    // -----------------------------------------------------------------------
    // Route matching

    pub fn match_route(&self, method: &SimpleMethod, route: &str) -> RouteResult<ArcServe> {
        let segments = parse_route_into_segments(route)?;
        let route_segments: Vec<SegmentType> = segments
            .iter()
            .map(|t| SegmentType::try_from(*t))
            .collect::<Result<_, _>>()?;

        let (matched, _params) = self.match_routes_from(route_segments, HashMap::new())?;
        matched.method.get_method(method)
    }

    fn match_routes_from(
        &self,
        mut route_patterns: Vec<SegmentType>,
        mut params: Params,
    ) -> RouteResult<(Self, Params)> {
        let next_segment_type = match route_patterns.first() {
            Some(next) => next.clone(),
            None => return Err(RouteOp::PanicSegmentNotFound),
        };

        // AnyPath catches everything — match regardless of remaining segments
        if matches!(&self.segment, SegmentType::AnyPath) {
            return Ok((self.clone(), params));
        }

        let remaining = match &self.segment {
            SegmentType::Root => route_patterns.split_off(0),
            _ => route_patterns.split_off(1),
        };

        // Root + Index with handler registered — return directly, don't recurse
        if self.segment == SegmentType::Root
            && remaining.len() == 1
            && matches!(remaining[0], SegmentType::Index)
            && self.method.has_any()
        {
            return Ok((self.clone(), params));
        }

        if !remaining.is_empty() {
            match self.validate_against_self(next_segment_type.clone(), &mut params) {
                Ok(_) => {
                    let next = self.get_matching_segment_route(remaining[0].clone(), &mut params)?;
                    Self::match_routes_from(next, remaining, params)
                }
                Err(err) => Err(err),
            }
        } else {
            match self.validate_against_self(next_segment_type, &mut params) {
                Ok(target) => Ok((target.clone(), params)),
                Err(err) => Err(err),
            }
        }
    }

    fn validate_against_self(
        &self,
        segment: SegmentType,
        params: &mut Params,
    ) -> RouteResult<&RouteSegment> {
        match self.segment.match_value_from(&segment)? {
            Some((key, value)) => {
                params.entry(key).or_insert(value);
                Ok(self)
            }
            None => Ok(self),
        }
    }

    fn get_matching_segment_route(
        &self,
        segment: SegmentType,
        params: &mut Params,
    ) -> RouteResult<&RouteSegment> {
        match &segment {
            SegmentType::Index => {
                if self.segment != SegmentType::Root {
                    return Ok(self);
                }
                // Root route with Index handler stored directly on root
                if self.method.has_any() {
                    return Ok(self);
                }
                self.match_against_dynamic_routes(segment, params)
            }
            SegmentType::Static(text) => {
                if self.static_routes.contains_key(text) {
                    return Ok(&self.static_routes[text]);
                }
                self.match_against_dynamic_routes(segment, params)
            }
            _ => self.match_against_dynamic_routes(segment, params),
        }
    }

    fn match_against_dynamic_routes(
        &self,
        segment: SegmentType,
        params: &mut Params,
    ) -> RouteResult<&RouteSegment> {
        for subroute in &self.dynamic_routes {
            if subroute.segment == SegmentType::AnyPath {
                return Ok(subroute);
            }
            match subroute.segment.match_value_from(&segment) {
                Ok(Some((key, value))) => {
                    params.entry(key).or_insert(value);
                    return Ok(subroute);
                }
                Ok(None) => return Ok(subroute),
                Err(_) => continue,
            }
        }
        Err(RouteOp::NoMatchingRoute(segment.as_string()))
    }
}
