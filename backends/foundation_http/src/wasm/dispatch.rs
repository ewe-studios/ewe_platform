//! wasm-specific dispatch logic for `HttpApp`.
//!
//! WHY: `dispatch_cf` and `dispatch_web` return `web_sys::Response` and
//! depend on wasm-specific types (`CfConn`, `WebConn`), so they live in the
//! wasm module rather than `shared/app` to avoid polluting shared code with
//! target-specific logic.
//!
//! WHAT: Extension trait `HttpAppDispatch` that adds `dispatch_cf` and
//! `dispatch_web` methods to `HttpApp<Arc<dyn CfServe>>` and
//! `HttpApp<Arc<dyn WebServe>>` respectively.

use std::sync::Arc;

use foundation_netio::simple_http::{SendSafeBody, SimpleIncomingRequest};

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::shared::middleware::MiddlewareResult;
use crate::shared::serve::ServeError;
use crate::wasm::cf_conn::{CfConn, CfConnectionResult};
use crate::wasm::serve_cf::CfServe;
use crate::wasm::serve_web::WebServe;
use crate::wasm::web_conn::{WebConn, WebConnectionResult};

/// Extension trait for dispatching `HttpApp<Arc<dyn CfServe>>`.
pub trait HttpAppCfDispatch {
    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    fn dispatch_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>>;
}

impl HttpAppCfDispatch for HttpApp<Arc<dyn CfServe>> {
    fn dispatch_cf(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
        let method = req.method.clone();
        let path = req.request_uri.path().to_string();

        // Run middleware
        let mut req = req;
        for mw in self.middleware_chain() {
            match mw.handle(&bag, &mut req) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Response(response) => {
                    let status_code: usize = response.status.clone().into();
                    let body_bytes = match &response.body {
                        Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                        Some(SendSafeBody::Bytes(b)) => b.clone(),
                        _ => Vec::new(),
                    };
                    let mut conn = CfConn::new();
                    conn.set_status(status_code as u16);
                    for (k, vals) in &response.headers {
                        conn.set_header(&k.to_string(), &vals.join(", "));
                    }
                    conn.set_body(body_bytes);
                    return conn.into_response().map_err(|e| {
                        ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router
        let handler = self.router().dispatch(&method, &path)
            .ok_or_else(|| ServeError::BadRequest {
                status: 404,
                reason: format!("no route for {method:?} {path}"),
            })?;

        // Execute handler
        let mut conn = CfConn::new();
        let result = handler.serve_cf(bag, req, &mut conn);

        match result {
            CfConnectionResult::Ok => {
                conn.into_response().map_err(|e| {
                    ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                })
            }
            CfConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }.into()
            })),
        }
    }
}

/// Extension trait for dispatching `HttpApp<Arc<dyn WebServe>>`.
pub trait HttpAppWebDispatch {
    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    fn dispatch_web(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>>;
}

impl HttpAppWebDispatch for HttpApp<Arc<dyn WebServe>> {
    fn dispatch_web(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
        let method = req.method.clone();
        let path = req.request_uri.path().to_string();

        // Run middleware
        let mut req = req;
        for mw in self.middleware_chain() {
            match mw.handle(&bag, &mut req) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Response(response) => {
                    let status_code: usize = response.status.clone().into();
                    let body_bytes = match &response.body {
                        Some(SendSafeBody::Text(s)) => s.as_bytes().to_vec(),
                        Some(SendSafeBody::Bytes(b)) => b.clone(),
                        _ => Vec::new(),
                    };
                    let mut conn = WebConn::new();
                    conn.set_status(status_code as u16);
                    for (k, vals) in &response.headers {
                        conn.set_header(&k.to_string(), &vals.join(", "));
                    }
                    conn.set_body(body_bytes);
                    return conn.into_response().map_err(|e| {
                        ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router
        let handler = self.router().dispatch(&method, &path)
            .ok_or_else(|| ServeError::BadRequest {
                status: 404,
                reason: format!("no route for {method:?} {path}"),
            })?;

        // Execute handler
        let mut conn = WebConn::new();
        let result = handler.serve_web(bag, req, &mut conn);

        match result {
            WebConnectionResult::Ok => {
                conn.into_response().map_err(|e| {
                    ServeError::InternalError { status: 500, reason: format!("{e:?}") }.into()
                })
            }
            WebConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }.into()
            })),
        }
    }
}
