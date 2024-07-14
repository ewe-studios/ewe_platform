use crate::{
    requests::{self, Method, MethodError, Params, Request, Uri},
    response::{self, Response, ResponseHead, StatusCode},
    routes::{
        create_servicer_func, ResponseFuture, RouteMethod, RouteOp, RouteSegment, Servicer,
        ServicerHandler, ServicerResult,
    },
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
        &mut self,
        route: &'a str,
        method: RouteMethod<R, S, Server>,
    ) -> RouterResult<RouterErrors> {
        match RouteSegment::parse_route(route, method) {
            Ok(segment) => {
                self.root.add_route(segment);
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }
}

pub fn bad_request_handler<R, S>(req: Request<R>) -> ResponseFuture<S, RouterErrors> {
    Box::pin(async {
        Ok(Response::from(
            None,
            ResponseHead::standard(StatusCode::NOT_IMPLEMENTED),
        ))
    })
}

pub fn default_fallback_method<R: Clone + Send, S: Clone + Send>(
) -> RouteMethod<R, S, ServicerHandler<R, S, RouterErrors>> {
    let method: RouteMethod<R, S, ServicerHandler<R, S, RouterErrors>> = RouteMethod::empty()
        .add_then(|mut method| {
            method.set_get(Some(create_servicer_func(bad_request_handler)));
            method.set_post(Some(create_servicer_func(bad_request_handler)));
            method.set_put(Some(create_servicer_func(bad_request_handler)));
            method.set_patch(Some(create_servicer_func(bad_request_handler)));
            method.set_delete(Some(create_servicer_func(bad_request_handler)));
            method.set_connect(Some(create_servicer_func(bad_request_handler)));
            method.set_trace(Some(create_servicer_func(bad_request_handler)));
            method.set_options(Some(create_servicer_func(bad_request_handler)));
            method
        });
    method
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use requests::RequestHead;

    use super::*;

    #[derive(Clone, PartialEq, Eq)]
    enum MyRequests {
        Hello(String),
    }

    impl fmt::Debug for MyRequests {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Hello(arg0) => f.debug_tuple("Hello").field(arg0).finish(),
            }
        }
    }

    #[derive(Clone, PartialEq, Eq)]
    enum MyResponse {
        World(String),
    }

    impl fmt::Debug for MyResponse {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::World(arg0) => f.debug_tuple("World").field(arg0).finish(),
            }
        }
    }

    fn hello_request(req: Request<MyRequests>) -> ResponseFuture<MyResponse, RouterErrors> {
        Box::pin(async move {
            let (_head, mut body) = req.into_parts();
            if let Some(message) = body.take() {
                let MyRequests::Hello(content) = message;
                return Ok(Response::from(
                    Some(MyResponse::World(String::from(format!(
                        "{} World!",
                        content,
                    )))),
                    ResponseHead::standard(StatusCode::NOT_IMPLEMENTED),
                ));
            }
            return Err(RouterErrors::IntervalError);
        })
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_get_hello_route() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();

        let hello_server = create_servicer_func(hello_request);

        let mut router = Router::new(fallback);
        router.route("/hello", RouteMethod::get(hello_server));

        let hello_endpoint = Uri::from_static("/hello");
        let req = Request::from(
            Some(MyRequests::Hello(String::from("Alex"))),
            RequestHead::get(hello_endpoint),
        );

        let response_result = router.serve(req).await;

        assert!(matches!(response_result, Result::Ok(_)));

        let mut response = response_result.unwrap();

        let response_body = response.body_mut().take().unwrap();
        assert_eq!(
            response_body,
            MyResponse::World(String::from("Alex World!"))
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fallback_handles_unknown_route() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();

        let mut router = Router::new(fallback);

        let hello_server = create_servicer_func(hello_request);
        router.route("/hello", RouteMethod::get(hello_server));

        let hello_endpoint = Uri::from_static("/world");
        let req = Request::from(
            Some(MyRequests::Hello(String::from("Alex"))),
            RequestHead::get(hello_endpoint),
        );

        let response_result = router.serve(req).await;

        assert!(matches!(response_result, Result::Ok(_)));

        let (head, body) = response_result.unwrap().into_parts();
        assert!(matches!(body, Option::None));
        assert_eq!(head.status, StatusCode::NOT_IMPLEMENTED);
    }
}
