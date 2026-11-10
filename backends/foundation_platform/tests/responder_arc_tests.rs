//! The responder registry keeps a responder alive after the entry is replaced.
//!
//! WHY: `get_responder` used to return a reference built from a `Box` inside an
//! `RwLock` read guard. The guard dropped before the caller used the reference,
//! so a concurrent replacement freed the `Box` and left a dangling reference.
//!
//! WHAT: a replaced responder must stay valid through an old handle.
//!
//! HOW: take a handle, replace the registry entry, then call the old handle.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_platform::*;

/// A responder that returns a fixed body.
struct FixedResponder(&'static str);

impl RouteResponder for FixedResponder {
    fn respond(
        &self,
        _intent: &NavigationIntent,
        _decision: &RouteDecision,
        _session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>> {
        tauri::http::Response::builder()
            .status(200)
            .body(self.0.as_bytes().to_vec())
            .unwrap()
    }
}

fn session() -> Arc<PlatformSession> {
    PlatformSession::new_test(
        PathBuf::from("."),
        foundation_context::Ctx::fixed(std::sync::Arc::new(
            foundation_context::MonotonicClock::new(),
        )),
    )
}

fn intent(url: &str) -> NavigationIntent {
    NavigationIntent {
        url: url.to_string(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    }
}

#[test]
fn a_replaced_responder_stays_alive_through_an_old_handle() {
    let session = session();

    session.register_responder("route", Box::new(FixedResponder("old responder")));
    let old = session
        .get_responder("route")
        .expect("the original responder");

    session.register_responder("route", Box::new(FixedResponder("new responder")));

    let response = old.respond(&intent("/app/items"), &webview_app(), &session);
    assert_eq!(
        response.body().as_slice(),
        b"old responder",
        "the old handle must keep the original responder alive after replacement"
    );
}
