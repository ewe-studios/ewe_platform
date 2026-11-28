//! The bearer-token interceptor of a native listener.
//!
//! The control-plane server (`native::serve`) and the model slave
//! (`native::model`) share one check. It runs in an interceptor before the
//! handler, so a call with no token or a wrong token never reaches the service.
//! The failure is `Unauthenticated` (HTTP 401).

use std::sync::Arc;

use foundation_connectrpc::shared::interceptor::{
    Interceptor, StreamCall, StreamingClientFunc, StreamingHandlerFunc, UnaryCall, UnaryFunc,
};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::SimpleHeader;

use crate::shared::error::SandboxError;
use crate::shared::Secret;

/// Refuses every call that does not carry the served token.
#[derive(Clone)]
pub struct BearerAuth {
    token: Secret,
    max_error_message: usize,
}

impl BearerAuth {
    /// An interceptor that accepts only `token`.
    ///
    /// `max_error_message` bounds the message of the refusal, as the rest of
    /// the sandbox errors are bounded.
    #[must_use]
    pub fn new(token: Secret, max_error_message: usize) -> Self {
        Self {
            token,
            max_error_message,
        }
    }

    /// The `Authorization` header of a call.
    fn check(&self, ctx: &foundation_connectrpc::RpcCtx) -> Result<(), ()> {
        let header = ctx
            .request
            .headers
            .get(&SimpleHeader::AUTHORIZATION)
            .and_then(|values| values.first())
            .map(String::as_str);
        match Secret::from_bearer(header) {
            Some(provided) if provided.ct_eq(&self.token) => Ok(()),
            _ => Err(()),
        }
    }

    /// The error that a refused call returns.
    fn error(&self) -> ErrorTrace<foundation_connectrpc::ConnectError> {
        ErrorTrace::new(SandboxError::Unauthorized.to_connect(self.max_error_message))
    }
}

impl Interceptor for BearerAuth {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let this = self.clone();
        Arc::new(move |ctx: foundation_connectrpc::RpcCtx, call: UnaryCall| {
            let this = this.clone();
            let next = next.clone();
            Box::pin(async move {
                this.check(&ctx).map_err(|()| this.error())?;
                next(ctx, call).await
            })
        })
    }

    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        next
    }

    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        let this = self.clone();
        Arc::new(
            move |ctx: foundation_connectrpc::RpcCtx, call: StreamCall| {
                let this = this.clone();
                let next = next.clone();
                Box::pin(async move {
                    this.check(&ctx).map_err(|()| this.error())?;
                    next(ctx, call).await
                })
            },
        )
    }
}
