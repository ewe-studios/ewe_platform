//! Panic recovery — wraps handler execution in `catch_unwind`.

use foundation_netio::shared::http::SimpleIncomingRequest;
use std::panic::AssertUnwindSafe;

use crate::shared::context::ContextBag;
use crate::shared::serve::{ConnectionResult, ServeError};

/// Wrap a `ServeWriter` handler's execution with panic recovery.
pub fn with_panic_recovery(
    handler: &dyn crate::shared::serve::ServeWriter,
    bag: &ContextBag,
    req: SimpleIncomingRequest,
    conn: &mut dyn std::io::Write,
) -> ConnectionResult {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        handler.serve_writer(bag, req, conn)
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
            ConnectionResult::Close(Some(
                foundation_errstacks::ErrorTrace::new(ServeError::InternalError {
                    status: 500,
                    reason: format!("handler panicked: {reason}"),
                }),
            ))
        }
    }
}
