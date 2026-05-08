//! Panic recovery — wraps handler execution in `catch_unwind`.
//!
//! Applied in the worker loop, not as middleware, because `Serve::serve`
//! is where the handler runs.

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::SimpleIncomingRequest;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use crate::context::ContextBag;
use crate::serve::{ConnectionResult, Serve, ServeError};

/// Wrap a handler's `serve` call with panic recovery.
///
/// If the handler panics, logs the panic and returns a Close result.
pub fn with_panic_recovery<H: Serve>(
    handler: &H,
    bag: Arc<ContextBag>,
    req: SimpleIncomingRequest,
    conn: SharedByteBufferStream<RawStream>,
) -> ConnectionResult {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        handler.serve(bag, req, conn)
    }));

    match result {
        Ok(connection_result) => connection_result,
        Err(panic_info) => {
            let reason = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            tracing::error!("Handler panicked: {reason}");
            // Note: conn is already moved into the handler, so we can't write a response here.
            // The panic recovery must be applied at a level where the conn is still available.
            ConnectionResult::Close(Some(
                foundation_errstacks::ErrorTrace::new(ServeError::InternalError {
                    status: 500,
                    reason: format!("handler panicked: {reason}"),
                }),
            ))
        }
    }
}
