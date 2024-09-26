// Types for the packages

use std::collections::HashMap;

use std::future::Future;
use std::net::SocketAddr;
use std::{pin, result};

use axum::body;

use derive_more::{Debug, From};

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type Result<T> = std::result::Result<T, BoxedError>;

pub type JoinHandle<T> = tokio::task::JoinHandle<Result<T>>;

#[derive(Debug, Default, Clone, From)]
pub struct ProxyRemoteConfig {
    pub addr: String,
    pub port: usize,
}

// -- Constructors

impl ProxyRemoteConfig {
    #[must_use]
    pub fn new(addr: String, port: usize) -> Self {
        Self { addr, port }
    }
}

// -- Debug Display

impl core::fmt::Display for ProxyRemoteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

// -- Proxy Type Structures

#[derive(Debug, Clone, From)]
pub struct Tunnel {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

impl core::fmt::Display for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Tunnel {
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self {
            source,
            destination,
        }
    }
}

pub type HyperRequest = hyper::Request<hyper::body::Incoming>;
pub type HyperResponse = hyper::Response<body::Body>;
pub type HyperResponseResult = result::Result<HyperResponse, hyper::Error>;
pub type HyperFuture = dyn Future<Output = HyperResponseResult> + Sync + Send + 'static;

pub type HyperFunc =
    dyn Fn(SocketAddr, HyperRequest) -> pin::Pin<Box<HyperFuture>> + Send + Sync + 'static;

pub type HyperFuncMap = HashMap<String, std::sync::Arc<HyperFunc>>;

#[derive(Debug, Clone, From)]
pub struct Http1 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
    #[debug(skip)]
    pub routes: Option<HyperFuncMap>,
}

impl Http1 {
    pub fn new(
        source: ProxyRemoteConfig,
        destination: ProxyRemoteConfig,
        routes: Option<HyperFuncMap>,
    ) -> Self {
        Self {
            source,
            destination,
            routes,
        }
    }

    pub fn and_routes(&mut self, mutator: impl Fn(&mut HyperFuncMap)) {
        self.routes = match self.routes.clone() {
            Some(mut route_map) => {
                mutator(&mut route_map);
                Some(route_map)
            }
            None => {
                let mut new_routes = HashMap::new();
                mutator(&mut new_routes);
                Some(new_routes)
            }
        };
    }
}

impl core::fmt::Display for Http1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Http1(source: {:?}, destination: {:?})",
            self.source, self.destination
        )
    }
}

#[derive(Debug, Clone, From)]
pub struct Http2 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
    #[debug(skip)]
    pub routes: Option<HyperFuncMap>,
}

impl Http2 {
    pub fn new(
        source: ProxyRemoteConfig,
        destination: ProxyRemoteConfig,
        routes: Option<HyperFuncMap>,
    ) -> Self {
        Self {
            source,
            destination,
            routes,
        }
    }

    pub fn and_routes(&mut self, mutator: impl Fn(&mut HyperFuncMap)) {
        self.routes = match self.routes.clone() {
            Some(mut route_map) => {
                mutator(&mut route_map);
                Some(route_map)
            }
            None => {
                let mut new_routes = HashMap::new();
                mutator(&mut new_routes);
                Some(new_routes)
            }
        };
    }
}

impl core::fmt::Display for Http2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Http2(source: {:?}, destination: {:?})",
            self.source, self.destination
        )
    }
}

#[derive(Debug, Clone, From)]
pub struct Http3 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,

    #[debug(skip)]
    pub routes: Option<HyperFuncMap>,
}

impl Http3 {
    pub fn new(
        source: ProxyRemoteConfig,
        destination: ProxyRemoteConfig,
        routes: Option<HyperFuncMap>,
    ) -> Self {
        Self {
            source,
            destination,
            routes,
        }
    }

    pub fn and_routes(&mut self, mutator: impl Fn(&mut HyperFuncMap)) {
        self.routes = match self.routes.clone() {
            Some(mut route_map) => {
                mutator(&mut route_map);
                Some(route_map)
            }
            None => {
                let mut new_routes = HashMap::new();
                mutator(&mut new_routes);
                Some(new_routes)
            }
        };
    }
}

impl core::fmt::Display for Http3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Http3(source: {:?}, destination: {:?})",
            self.source, self.destination
        )
    }
}
