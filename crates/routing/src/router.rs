use crate::{
    requests::{self, Method, MethodError, Params},
    response,
    routes::{RouteMethod, RouteOp, RouteSegment, Servicer, ServicerResult},
};

use std::{error, future::Future, marker::PhantomData, pin::Pin, task::Poll};
use thiserror::Error;
use tower::Service;

pub type ServerFuture<S, E> = std::pin::Pin<Box<dyn Future<Output = ServicerResult<S, E>>>>;

pub type RouterResult<E> = std::result::Result<(), E>;

#[derive(Debug, Error, Clone)]
pub enum RouterErrors {
    #[error("failed to add route to router: {0}")]
    FailedAdding(String),

    #[error("failed to find related route for: {0}")]
    NoRouteMatching(String),

    #[error("Internal routing error, please investigate")]
    RouteError(RouteOp),

    #[error("Route has no server for http Method: {0} and error: {1}")]
    MethodServerError(Method, MethodError),

    #[error("Route has no server for http Method: {0}")]
    MethodHasNoServer(Method),

    #[error("Internal error, please investigate")]
    IntervalError,
}

impl From<RouteOp> for RouterErrors {
    fn from(value: RouteOp) -> Self {
        RouterErrors::RouteError(value)
    }
}

#[derive(Clone)]
pub struct Router<'a, R: Send + Clone, S: Send + Clone, Server: Servicer<R, S>> {
    fallback: RouteMethod<R, S, Server>,
    root: RouteSegment<'a, R, S, Server>,
}

impl<
        'a: 'static,
        R: Send + Clone + 'a,
        S: Send + Clone + 'a,
        Server: Servicer<R, S, Error = RouterErrors> + 'a,
    > tower::Service<http::Request<R>> for Router<'a, R, S, Server>
{
    type Error = RouterErrors;
    type Response = http::Response<S>;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<R>) -> Self::Future {
        let request: requests::Request<R> = req.try_into().unwrap();
        let clone_self = self.clone();
        Box::pin(async move {
            let result = clone_self.serve(request).await;
            response::ResponseResult(result).into()
        })
    }
}

impl<
        'b: 'static,
        R: Send + Clone + 'b,
        S: Send + Clone + 'b,
        Server: Servicer<R, S, Error = RouterErrors> + 'b,
    > Servicer<R, S> for Router<'b, R, S, Server>
{
    type Error = RouterErrors;

    type Future = ServerFuture<S, Self::Error>;

    fn serve(&self, req: requests::Request<R>) -> Self::Future {
        let route = req.path();
        let (head, body) = req.into_parts();
        let method = head.clone_method();

        let get_route_result = self
            .root
            .match_route(method.clone(), &route)
            .map_err(|e| RouterErrors::RouteError(e));

        let fallback_route_result = self
            .fallback
            .get_method(method.clone())
            .map_err(|e| RouterErrors::MethodServerError(method.clone(), e));

        Box::pin(async move {
            if let Ok((server, params)) = get_route_result {
                let request_with_params = requests::Request::with(body, head, params);
                return server.serve(request_with_params).await;
            }

            if let Ok(fallback_server) = fallback_route_result {
                let request_with_params = requests::Request::from(body, head);
                return fallback_server.serve(request_with_params).await;
            }

            Err(RouterErrors::MethodHasNoServer(method.clone()))
        })
    }
}

impl<'a, R: Send + Clone, S: Send + Clone, Server: Servicer<R, S>> Router<'a, R, S, Server> {
    fn new(fallback: RouteMethod<R, S, Server>) -> Self {
        Router {
            fallback,
            root: RouteSegment::root(),
        }
    }

    fn route(
        &self,
        route: &'a str,
        method: RouteMethod<R, S, Server>,
    ) -> RouterResult<RouterErrors> {
        // self.root.
        todo!()
    }
}
