use crate::simple_http::shared::{SimpleHeader, SimpleHeaders};
use std::collections::BTreeMap;
use wasm_bindgen::JsValue;

/// Convert `SimpleHeaders` → `web_sys::Headers`.
///
/// # Errors
///
/// Returns `JsValue` if `Headers::new()` or `append()` fails.
pub fn simple_headers_to_web_sys(headers: &SimpleHeaders) -> Result<web_sys::Headers, JsValue> {
    let ws = web_sys::Headers::new()?;
    for (key, values) in headers {
        let name = key.to_string().to_lowercase();
        for value in values {
            ws.append(&name, value)?;
        }
    }
    Ok(ws)
}

/// Convert `web_sys::Headers` → `SimpleHeaders`.
pub fn web_sys_headers_to_simple(headers: &web_sys::Headers) -> SimpleHeaders {
    let mut simple: SimpleHeaders = BTreeMap::new();
    let iter = headers.entries();
    let entries = js_sys::try_iter(&iter).ok().and_then(|i| i);

    if let Some(iter) = entries {
        for entry in iter.flatten() {
            let pair = js_sys::Array::from(&entry);
            if let (Some(key), Some(val)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                let header = SimpleHeader::from(key);
                simple.entry(header).or_default().push(val);
            }
        }
    }
    simple
}
