//! Simple structured response for wasm — status, body, and headers as native types.

/// A structured wasm response — status code, optional body, and headers.
#[derive(Default)]
pub struct WasmResponse {
    pub status: u16,
    pub body: Option<Vec<u8>>,
    pub headers: Vec<(String, String)>,
}

impl WasmResponse {
    /// Create a new response with the given status code.
    pub fn new(status: u16) -> Self {
        Self {
            status,
            body: None,
            headers: Vec::new(),
        }
    }

    /// Set the response body.
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Add a header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// Build a `web_sys::Response` from a status code, body, and optional headers.
#[cfg(target_arch = "wasm32")]
pub use wasm_helpers::{build_response, from_bytes, from_wasm};

#[cfg(target_arch = "wasm32")]
mod wasm_helpers {
    use wasm_bindgen::prelude::*;
    use web_sys::{Response, ResponseInit};

    use super::WasmResponse;

    pub fn build_response(
        status: u16,
        body: impl Into<Vec<u8>>,
        headers: Vec<(String, String)>,
    ) -> Result<Response, JsError> {
        let body = body.into();
        let mut body_mut = body;

        let init = ResponseInit::new();
        init.set_status(status);

        let web_headers = web_sys::Headers::new().map_err(|e| {
            JsError::new(&format!("failed to create headers: {e:?}"))
        })?;
        for (k, v) in headers {
            let _ = web_headers.append(&k, &v);
        }
        init.set_headers(&web_headers);

        Response::new_with_opt_u8_array_and_init(Some(&mut body_mut), &init).map_err(|e| {
            JsError::new(&format!("failed to create response: {e:?}"))
        })
    }

    /// Build a `web_sys::Response` from a `WasmResponse`.
    pub fn from_wasm(response: WasmResponse) -> Result<Response, JsError> {
        let body = response.body.unwrap_or_default();
        build_response(response.status, body, response.headers)
    }

    /// Convenience: build a `web_sys::Response` from just status and body.
    pub fn from_bytes(status: u16, body: impl Into<Vec<u8>>) -> Result<Response, JsError> {
        build_response(status, body, Vec::new())
    }
}
