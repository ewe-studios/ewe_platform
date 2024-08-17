use crate::{
    requests::{
        self, BodyHttpRequest, FromBody, FromBytes, IntoBody, Method, MethodError, Request,
        RequestHead, TryFromBodyRequestError, TypedHttpRequest,
    },
    response::{self, Response, ResponseHead, StatusCode},
    routes::{
        create_servicer_func, ResponseFuture, RouteMethod, RouteOp, RouteSegment, Servicer,
        ServicerHandler, ServicerResult,
    },
};

use axum::body;
use std::{convert::Infallible, future::Future, task::Poll};
use thiserror::Error;

pub type ServerFuture<'a, S, E> =
    std::pin::Pin<Box<dyn Future<Output = ServicerResult<S, E>> + Send + 'a>>;

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

    #[error("Route failed read body: body bigger than {0}")]
    BodyBytesLengthLimitError(usize),

    #[error("Route failed to convert body: {0}")]
    BodyConversionError(TryFromBodyRequestError),

    #[error("Infallible error: {0}")]
    Infallible(Infallible),

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

/// RouterService wraps a Router instance and transforms into a tower::Service for the
/// attachment to an axum Router, it expects the usize defining the max body size the
/// service can take.
#[derive(Clone)]
pub struct RouterService<'a, R, S, Server>(usize, Router<'a, R, S, Server>)
where
    R: Send + Clone,
    S: Send + Clone,
    Server: Servicer<R, S>;

impl<'a: 'static, R, S, Server> RouterService<'a, R, S, Server>
where
    R: FromBytes<R> + Send + Clone + 'a,
    S: IntoBody<S, Infallible> + Send + Clone + 'a,
    Server: Servicer<R, S, Error = RouterErrors> + 'a,
{
    pub fn new(max_body_bytes: usize, router: Router<'a, R, S, Server>) -> Self {
        Self(max_body_bytes, router)
    }
}

impl<'a: 'static, R, S, Server> tower::Service<http::Request<axum::body::Body>>
    for RouterService<'a, R, S, Server>
where
    R: FromBytes<R> + Send + Clone + 'a,
    S: IntoBody<S, Infallible> + Default + Send + Clone + 'a,
    Server: Servicer<R, S, Error = RouterErrors> + Send + 'a,
{
    type Error = Infallible;
    type Response = http::Response<body::Body>;
    type Future =
        std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'a>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<axum::body::Body>) -> Self::Future {
        ewe_logs::debug!("RouterService received requests");

        let body_limit = self.0;
        let server = self.1.clone();

        Box::pin(async move {
            let (head, body) = req.into_parts();
            match axum::body::to_bytes(body, body_limit).await {
                Ok(body_bytes) => match R::from_body(body_bytes) {
                    Ok(transmuted_body) => {
                        let request_head: RequestHead = head.into();
                        let request = Request::from(Some(transmuted_body), request_head);

                        match server.serve(request).await {
                            Ok(response) => {
                                let (mut res_head, res_body) = response.into_parts();
                                match res_body {
                                    Some(content) => {
                                        let mut response_builder = http::response::Builder::new()
                                            .status(res_head.status)
                                            .version(res_head.version);

                                        response_builder
                                            .headers_mut()
                                            .replace(res_head.headers_mut());

                                        response_builder
                                            .extensions_mut()
                                            .replace(res_head.extensions_mut());

                                        let converted_body = S::into_body(content).unwrap();

                                        Ok(response_builder.body(converted_body).unwrap())
                                    }
                                    None => {
                                        let mut response_builder = http::response::Builder::new()
                                            .status(res_head.status)
                                            .version(res_head.version);

                                        response_builder
                                            .headers_mut()
                                            .replace(res_head.headers_mut());

                                        response_builder
                                            .extensions_mut()
                                            .replace(res_head.extensions_mut());

                                        let converted_body = S::into_body(S::default()).unwrap();

                                        Ok(response_builder.body(converted_body).unwrap())
                                    }
                                }
                            }
                            Err(bad_err) => {
                                ewe_logs::debug!("Bad request received: {:?}", bad_err);
                                response::ResponseResult(Ok(Response::from_head(
                                    ResponseHead::standard(StatusCode::BAD_REQUEST),
                                )))
                                .into()
                            }
                        }
                    }
                    Err(_err) => {
                        ewe_logs::debug!("TryFromBodyRequestError occured");
                        response::ResponseResult(Ok(Response::from_head(ResponseHead::standard(
                            StatusCode::BAD_REQUEST,
                        ))))
                        .into()
                    }
                },
                Err(err) => {
                    ewe_logs::error!("Failed to read body bytes: {:?}", err);
                    response::ResponseResult(Ok(Response::from_head(ResponseHead::standard(
                        StatusCode::BAD_REQUEST,
                    ))))
                    .into()
                }
            }
        })
    }
}

impl<'a: 'static, R, S, Server> tower::Service<BodyHttpRequest> for Router<'a, R, S, Server>
where
    R: FromBody<R> + Send + Clone + 'a,
    S: Send + Clone + 'a,
    Server: Servicer<R, S, Error = RouterErrors> + 'a,
{
    type Error = RouterErrors;
    type Response = http::Response<S>;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: BodyHttpRequest) -> Self::Future {
        let clone_self = self.clone();
        Box::pin(async move {
            let request: requests::Request<R> = req.try_into().expect("should unwrap");
            let result = clone_self.serve(request).await;
            response::ResponseResult(result).into()
        })
    }
}

impl<'a: 'static, R, S, Server> tower::Service<TypedHttpRequest<R>> for Router<'a, R, S, Server>
where
    R: Send + Clone + 'a,
    S: Send + Clone + 'a,
    Server: Servicer<R, S, Error = RouterErrors> + 'a,
{
    type Error = RouterErrors;
    type Response = http::Response<S>;
    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TypedHttpRequest<R>) -> Self::Future {
        let clone_self = self.clone();
        Box::pin(async move {
            let request: requests::Request<R> = req.try_into().expect("should unwrap");
            let result = clone_self.serve(request).await;
            response::ResponseResult(result).into()
        })
    }
}

impl<'b: 'static, R, S, Server> Servicer<R, S> for Router<'b, R, S, Server>
where
    R: Send + Clone + 'b,
    S: Send + Clone + 'b,
    Server: Servicer<R, S, Error = RouterErrors> + 'b,
{
    type Error = RouterErrors;

    type Future = ServerFuture<'b, S, Self::Error>;

    fn serve(&self, req: requests::Request<R>) -> Self::Future
    where
        <Server as Servicer<R, S>>::Future: Send,
    {
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

pub fn bad_request_handler<R, S>(_req: Request<R>) -> ResponseFuture<S, RouterErrors> {
    ewe_logs::debug!("Fallback handler reeceived requests");
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
mod router_tests {
    use super::*;
    use core::fmt;
    use http::Uri;
    use requests::{FromBytes, IntoBody, RequestHead, TryFromBodyRequestError};
    use serde::{Deserialize, Serialize};

    use tower::Service;
    use tracing_test::traced_test;

    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum MyRequests {
        Hello(String),
    }

    impl FromBytes<MyRequests> for MyRequests {
        fn from_body(
            body: bytes::Bytes,
        ) -> std::result::Result<MyRequests, requests::TryFromBodyRequestError> {
            let content = String::from_utf8(body.to_vec())
                .map_err(|_| TryFromBodyRequestError::FailedConversion)?;
            ewe_logs::debug!("Request from bytes: {}", content);
            let data: MyRequests = serde_json::from_slice(content.as_bytes()).unwrap();

            Ok(data)
        }
    }

    impl fmt::Debug for MyRequests {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Hello(arg0) => f.debug_tuple("Hello").field(arg0).finish(),
            }
        }
    }

    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
    enum MyResponse {
        World(String),
    }

    impl Default for MyResponse {
        fn default() -> Self {
            MyResponse::World(String::from(""))
        }
    }

    impl IntoBody<MyResponse, Infallible> for MyResponse {
        fn into_body(body: MyResponse) -> std::result::Result<body::Body, Infallible> {
            Ok(axum::body::Body::from(
                serde_json::to_string(&body).unwrap(),
            ))
        }
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
                    ResponseHead::standard(StatusCode::OK),
                ));
            }
            return Err(RouterErrors::IntervalError);
        })
    }

    #[traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_get_hello_route() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();

        let hello_server = create_servicer_func(hello_request);

        let mut router = Router::new(fallback);
        assert!(matches!(
            router.route("/hello", RouteMethod::get(hello_server)),
            RouterResult::Ok(_)
        ));

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

    #[traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fallback_handles_unknown_route() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();
        let mut router = Router::new(fallback);

        let hello_server = create_servicer_func(hello_request);
        assert!(matches!(
            router.route("/hello", RouteMethod::get(hello_server)),
            RouterResult::Ok(_)
        ));

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

    #[traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_get_index_route_using_blanket_routing() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();

        let hello_server = create_servicer_func(hello_request);

        let mut router = Router::new(fallback);
        assert!(matches!(
            router.route("/*", RouteMethod::get(hello_server)),
            RouterResult::Ok(_)
        ));

        let hello_endpoint = Uri::from_static("/");
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

    #[traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_get_hello_route_using_blanket_routing() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();

        let hello_server = create_servicer_func(hello_request);

        let mut router = Router::new(fallback);
        assert!(matches!(
            router.route("/*", RouteMethod::get(hello_server)),
            RouterResult::Ok(_)
        ));

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

    #[traced_test]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_use_router_with_axum_router() {
        let fallback = default_fallback_method::<MyRequests, MyResponse>();
        let mut our_router = Router::new(fallback);

        let hello_server = create_servicer_func(hello_request);
        assert!(matches!(
            our_router.route("/*", RouteMethod::get(hello_server)),
            RouterResult::Ok(_)
        ));

        let our_router_service = RouterService::new(1024, our_router);
        let mut axum_router: axum::Router =
            axum::Router::new().route_service("/", our_router_service);

        let serialized = serde_json::to_string(&MyRequests::Hello(String::from("Alex"))).unwrap();

        let req = http::Request::builder()
            .uri("/")
            .body(axum::body::Body::from(serialized))
            .unwrap();

        // let response = axum_router.ready().await.unwrap().call(req).await;
        let response = axum_router.call(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let response_body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();

        let my_response: MyResponse = serde_json::from_slice(&response_body).unwrap();

        assert_eq!(my_response, MyResponse::World(String::from("Alex World!")));
    }
}
