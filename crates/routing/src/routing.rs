use crate::{
    requests, response,
    routes::{RouteSegment, Servicer, ServicerResult},
};

use std::{future::Future, marker::PhantomData, pin::Pin, task::Poll};
use tower::Service;

#[derive(Clone, Debug)]
pub struct Routing<'a, E: Send + Clone, R: Send + Clone, S: Send + Clone, Server: Servicer<R, S>> {
    root: RouteSegment<'a, R, S, Server>,
    _err: PhantomData<E>,
}

impl<'a, E: Send + Clone, R: Send + Clone, S: Send + Clone, Server: Servicer<R, S>>
    Routing<'a, E, R, S, Server>
{
    fn new() -> Self {
        Routing {
            root: RouteSegment::root(),
            _err: PhantomData::default(),
        }
    }
}

pub type ServerFuture<S, E> = std::pin::Pin<Box<dyn Future<Output = ServicerResult<S, E>>>>;

impl<'a, E: Send + Clone, R: Send + Clone, S: Send + Clone, Server: Servicer<R, S>> Servicer<R, S>
    for Routing<'a, E, R, S, Server>
{
    type Error = E;

    type Future = ServerFuture<S, Self::Error>;

    fn serve<F>(&self, req: requests::Request<R>) -> Self::Future {
        todo!()
    }
}

impl<
        'a: 'static,
        E: Send + Clone + 'a,
        R: Send + Clone + 'a,
        S: Send + Clone + 'a,
        Server: Servicer<R, S> + 'a,
    > tower::Service<http::Request<R>> for Routing<'a, E, R, S, Server>
{
    type Error = E;
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
            let result = clone_self
                .serve::<ServerFuture<S, Self::Error>>(request)
                .await;
            response::ResponseResult(result).into()
        })
    }
}
