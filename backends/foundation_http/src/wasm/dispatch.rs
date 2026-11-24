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

use foundation_netio::shared::http::{SendSafeBody, SimpleIncomingRequest};

use crate::shared::app::HttpApp;
use crate::shared::context::Ctx;
use crate::shared::middleware::MiddlewareResult;
use crate::shared::router::RouteOp;
use crate::shared::serve::ServeError;
use crate::shared::serve_web::WebServe;
use crate::shared::web_conn::{WebConn, WebConnectionResult};
use crate::wasm::cf_conn::{CfConn, CfConnectionResult};
use crate::wasm::serve_cf::CfServe;

/// Build a `web_sys::Response` that carries only a status and a short text
/// body.
///
/// Used for a route miss and for the 500 of a caught handler panic.
pub(crate) fn web_status_response(
    status: u16,
    text: &str,
) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
    let mut conn = WebConn::new();
    conn.set_status(status);
    conn.set_header("content-type", "text/plain");
    conn.set_body(text.as_bytes().to_vec());
    conn.into_response().map_err(|e| {
        ServeError::InternalError {
            status: 500,
            reason: format!("{e:?}"),
        }
        .into()
    })
}

/// Build a 405 `web_sys::Response` that names the allowed methods.
///
/// WHY: RFC 9110 section 15.5.6 requires an `Allow` header on a 405.
/// HOW: a short text body and the `Allow` header, through a fresh `WebConn`.
fn web_method_not_allowed(
    allowed: &[String],
) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
    let mut conn = WebConn::new();
    conn.set_status(405);
    conn.set_header("content-type", "text/plain");
    conn.set_header("allow", &allowed.join(", "));
    conn.set_body(b"Method Not Allowed".to_vec());
    conn.into_response().map_err(|e| {
        ServeError::InternalError {
            status: 500,
            reason: format!("{e:?}"),
        }
        .into()
    })
}

/// Build a 405 `web_sys::Response` for the Cloudflare entry.
///
/// See [`web_method_not_allowed`].
fn cf_method_not_allowed(
    allowed: &[String],
) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
    let mut conn = CfConn::new();
    conn.set_status(405);
    conn.set_header("content-type", "text/plain");
    conn.set_header("allow", &allowed.join(", "));
    conn.set_body(b"Method Not Allowed".to_vec());
    conn.into_response().map_err(|e| {
        ServeError::InternalError {
            status: 500,
            reason: format!("{e:?}"),
        }
        .into()
    })
}

/// Build a `web_sys::Response` that carries only a status and a short text
/// body for the Cloudflare entry.
///
/// Used for a route miss and for the 500 of a caught handler panic.
pub(crate) fn cf_status_response(
    status: u16,
    text: &str,
) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
    let mut conn = CfConn::new();
    conn.set_status(status);
    conn.set_header("content-type", "text/plain");
    conn.set_body(text.as_bytes().to_vec());
    conn.into_response().map_err(|e| {
        ServeError::InternalError {
            status: 500,
            reason: format!("{e:?}"),
        }
        .into()
    })
}

/// Extension trait for dispatching `HttpApp<Arc<dyn CfServe>>`.
pub trait HttpAppCfDispatch {
    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    fn dispatch_cf(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
    ) -> impl std::future::Future<
        Output = Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>>,
    > + '_;
}

impl HttpAppCfDispatch for HttpApp<Arc<dyn CfServe>> {
    async fn dispatch_cf(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
        let method = req.method.clone();
        let path = req.request_uri.path().to_string();

        // Run middleware
        let mut req = req;
        for mw in self.middleware_chain() {
            match mw.handle(&ctx, &mut req) {
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
                        ServeError::InternalError {
                            status: 500,
                            reason: format!("{e:?}"),
                        }
                        .into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router. A matched path with no handler for the method is
        // a 405; a path that matches nothing is a 404 (spec 88 feature 02).
        let handler = match self.router().dispatch_result(&method, &path) {
            Ok(handler) => handler,
            Err(RouteOp::MethodNotAllowed) => {
                return cf_method_not_allowed(&self.router().allowed_methods(&path));
            }
            Err(_) => return cf_status_response(404, "Not Found"),
        };

        // Execute handler (async)
        let mut conn = CfConn::new();
        let result = handler.serve_cf(ctx, req, &mut conn).await;

        match result {
            CfConnectionResult::Ok => conn.into_response().map_err(|e| {
                ServeError::InternalError {
                    status: 500,
                    reason: format!("{e:?}"),
                }
                .into()
            }),
            CfConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }
                .into()
            })),
        }
    }
}

/// Extension trait for dispatching `HttpApp<Arc<dyn WebServe>>`.
pub trait HttpAppWebDispatch {
    /// Dispatch a request through middleware and router, returning a `web_sys::Response`.
    fn dispatch_web(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
    ) -> impl std::future::Future<
        Output = Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>>,
    > + '_;
}

impl HttpAppWebDispatch for HttpApp<Arc<dyn WebServe>> {
    async fn dispatch_web(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
    ) -> Result<web_sys::Response, foundation_errstacks::ErrorTrace<ServeError>> {
        let method = req.method.clone();
        let path = req.request_uri.path().to_string();

        // Run middleware
        let mut req = req;
        for mw in self.middleware_chain() {
            match mw.handle(&ctx, &mut req) {
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
                        ServeError::InternalError {
                            status: 500,
                            reason: format!("{e:?}"),
                        }
                        .into()
                    });
                }
                MiddlewareResult::InterimResponse(_) => {}
            }
        }

        // Dispatch to router. A matched path with no handler for the method is
        // a 405; a path that matches nothing is a 404 (spec 88 feature 02).
        let handler = match self.router().dispatch_result(&method, &path) {
            Ok(handler) => handler,
            Err(RouteOp::MethodNotAllowed) => {
                return web_method_not_allowed(&self.router().allowed_methods(&path));
            }
            Err(_) => return web_status_response(404, "Not Found"),
        };

        // Execute handler (async)
        let mut conn = WebConn::new();
        let result = handler.serve_web(ctx, req, &mut conn).await;

        match result {
            WebConnectionResult::Ok => conn.into_response().map_err(|e| {
                ServeError::InternalError {
                    status: 500,
                    reason: format!("{e:?}"),
                }
                .into()
            }),
            WebConnectionResult::Close(err) => Err(err.unwrap_or_else(|| {
                ServeError::InternalError {
                    status: 500,
                    reason: "handler closed connection".into(),
                }
                .into()
            })),
        }
    }
}
