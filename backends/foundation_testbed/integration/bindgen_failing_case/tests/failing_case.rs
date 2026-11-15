//! The two cases of the wasm-bindgen regression fixture (spec 88 T5).
//!
//! `a_passes` is green and `b_fails` is red. Every wasm-bindgen runner must
//! exit non-zero when it runs both, and exit 0 when `--filter a_passes` runs
//! only the first. `echo_endpoint_answers` proves the host offers the local
//! endpoint of spec 88 invariant I5; `--filter echo` runs it alone.
//! Keep this file the only test target (rule R10).

#![cfg(target_arch = "wasm32")]

use foundation_testbed_bindgen_failing_case::ANSWER;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn a_passes() {
    assert_eq!(ANSWER, 42, "the passing case of the fixture");
}

#[wasm_bindgen_test]
fn b_fails() {
    assert_eq!(1, 2, "b_fails is the deliberate failure of this fixture");
}

/// The base URL the host sets before the first case (spec 88 invariant I5).
fn base_url() -> String {
    js_sys::Reflect::get(
        &js_sys::global(),
        &JsValue::from_str("__FWT_BASE_URL__"),
    )
    .expect("read __FWT_BASE_URL__")
    .as_string()
    .expect("the host sets __FWT_BASE_URL__ to a string")
}

/// Send one POST to `url` and return its status and text.
async fn post(url: &str, body: &str) -> (u16, String) {
    let headers = web_sys::Headers::new().expect("headers");
    headers
        .set("x-fwt-probe", "bindgen")
        .expect("set the probe header");
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(body));
    init.set_headers(&headers);
    let request = web_sys::Request::new_with_str_and_init(url, &init).expect("build the request");
    // A page has a Window; workerd has only a WorkerGlobalScope.
    let promise = match web_sys::window() {
        Some(window) => window.fetch_with_request(&request),
        None => {
            let scope: web_sys::WorkerGlobalScope = js_sys::global().unchecked_into();
            scope.fetch_with_request(&request)
        }
    };
    let resolved = JsFuture::from(promise).await.unwrap_or_else(|error| {
        let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
            .ok()
            .and_then(|value| value.as_string())
            .or_else(|| error.as_string())
            .unwrap_or_else(|| "the fetch rejected with no message".to_string());
        panic!("the fetch to {url} failed: {message}");
    });
    let response: web_sys::Response = resolved.unchecked_into();
    let status = response.status();
    let text = JsFuture::from(response.text().expect("the body promise"))
        .await
        .expect("the body resolves")
        .as_string()
        .unwrap_or_default();
    (status, text)
}

#[wasm_bindgen_test]
async fn echo_endpoint_answers() {
    let url = format!("{}/echo", base_url());
    let (status, text) = post(&url, "hello").await;
    assert_eq!(status, 200, "the echo route answered: {text}");
    assert!(text.contains("\"method\":\"POST\""), "{text}");
    assert!(text.contains("\"path\":\"/echo\""), "{text}");
    assert!(text.contains("\"x-fwt-probe\":\"bindgen\""), "{text}");
    assert!(text.contains("\"body\":\"aGVsbG8=\""), "{text}");
    assert!(text.contains("\"bytes\":5"), "{text}");
}
