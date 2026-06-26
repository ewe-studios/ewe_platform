//! Web / browser connection type — captures response as structured fields.
//!
//! Handlers implementing `WebServe` receive a `&mut WebConn` and call
//! `set_status`, `set_header`, `set_body` to build the response.
//! No HTTP wire format parsing — the dispatcher converts fields directly
//! into a `web_sys::Response`.

use foundation_errstacks::ErrorTrace;

use crate::shared::serve::ServeError;

/// Outcome of a `WebServe` handler.
pub enum WebConnectionResult {
    /// Response was written successfully.
    Ok,
    /// Handler encountered an error — connection should be closed.
    Close(Option<ErrorTrace<ServeError>>),
}

/// A typed connection for web/browser wasm that captures response fields
/// without going through HTTP wire format.
pub struct WebConn {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl WebConn {
    /// Create a new connection with default 200 status.
    pub fn new() -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: None,
        }
    }

    /// Set the response status code.
    pub fn set_status(&mut self, status: u16) {
        self.status = status;
    }

    /// Set a response header (replaces any existing header with the same name).
    pub fn set_header(&mut self, name: &str, value: &str) {
        if let Some(entry) = self.headers.iter_mut().find(|(k, _)| k == name) {
            entry.1 = value.to_string();
        } else {
            self.headers.push((name.to_string(), value.to_string()));
        }
    }

    /// Append a response header (allows multiple headers with the same name).
    pub fn append_header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }

    /// Set the response body.
    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = Some(body);
    }

    /// Convert collected fields into a `web_sys::Response`.
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-http"))]
    pub fn into_response(self) -> Result<web_sys::Response, wasm_bindgen::JsError> {
        let init = web_sys::ResponseInit::new();
        init.set_status(self.status);

        let web_headers = web_sys::Headers::new().map_err(|e| {
            wasm_bindgen::JsError::new(&format!("failed to create headers: {e:?}"))
        })?;
        for (k, v) in &self.headers {
            let _ = web_headers.append(k, v);
        }
        init.set_headers(&web_headers);

        let body = self.body.unwrap_or_default();
        let mut body_mut = body;
        web_sys::Response::new_with_opt_u8_array_and_init(Some(&mut body_mut), &init).map_err(|e| {
            wasm_bindgen::JsError::new(&format!("failed to create response: {e:?}"))
        })
    }
}

impl Default for WebConn {
    fn default() -> Self {
        Self::new()
    }
}

