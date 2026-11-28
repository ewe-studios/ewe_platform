//! Convert between the SQL engine values and JavaScript values.
//!
//! Cloudflare D1 and Durable Object (DO) SQL receive bindings as JavaScript
//! values. A JavaScript Number holds a 64-bit integer only up to 2^53. This
//! module binds a larger integer as decimal text, so a typed SQLite column
//! keeps its exact value.

use foundation_core::durable::sql::SqlValue;
use js_sys::{Array, ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

/// The largest integer a JavaScript Number holds exactly, as a float.
pub(crate) const MAX_SAFE_FLOAT: f64 = 9_007_199_254_740_992.0;

/// The largest integer a JavaScript Number holds exactly.
pub(crate) const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_992;

/// Turn one bound value into the JavaScript value that SQL binds.
pub(crate) fn value_to_js(value: &SqlValue) -> JsValue {
    match value {
        SqlValue::Null => JsValue::NULL,
        SqlValue::Integer(value) if (-MAX_SAFE_INTEGER..MAX_SAFE_INTEGER).contains(value) =>
        {
            #[allow(clippy::cast_precision_loss)]
            JsValue::from_f64(*value as f64)
        }
        SqlValue::Integer(value) => JsValue::from_str(&value.to_string()),
        SqlValue::Real(value) => JsValue::from_f64(*value),
        SqlValue::Text(value) => JsValue::from_str(value),
        SqlValue::Blob(value) => {
            let length = u32::try_from(value.len()).unwrap_or(u32::MAX);
            let buffer = Uint8Array::new_with_length(length);
            buffer.copy_from(value);
            buffer.into()
        }
    }
}

/// Turn one column value from a query row into a bound value.
pub(crate) fn js_to_value(value: &JsValue) -> SqlValue {
    if value.is_null() || value.is_undefined() {
        return SqlValue::Null;
    }
    if let Some(flag) = value.as_bool() {
        return SqlValue::Integer(i64::from(flag));
    }
    if let Some(number) = value.as_f64() {
        return number_to_value(number);
    }
    if let Some(text) = value.as_string() {
        return SqlValue::Text(text);
    }
    if let Some(buffer) = value.dyn_ref::<ArrayBuffer>() {
        return SqlValue::Blob(Uint8Array::new(buffer).to_vec());
    }
    if let Some(bytes) = value.dyn_ref::<Uint8Array>() {
        return SqlValue::Blob(bytes.to_vec());
    }
    if let Some(numbers) = value.dyn_ref::<Array>() {
        return SqlValue::Blob(array_to_blob(numbers));
    }
    SqlValue::Text(
        js_sys::JSON::stringify(value)
            .ok()
            .and_then(|text| text.as_string())
            .unwrap_or_default(),
    )
}

/// Keep an integral Number as an integer, and a fractional Number as a real.
fn number_to_value(number: f64) -> SqlValue {
    let integral = number.fract() == 0.0;
    let safe = (-MAX_SAFE_FLOAT..=MAX_SAFE_FLOAT).contains(&number);
    if integral && safe {
        #[allow(clippy::cast_possible_truncation)]
        SqlValue::Integer(number as i64)
    } else {
        SqlValue::Real(number)
    }
}

/// Read a JavaScript array of byte values as a blob.
fn array_to_blob(numbers: &Array) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::try_from(numbers.length()).unwrap_or(0));
    for index in 0..numbers.length() {
        let byte = numbers.get(index).as_f64().unwrap_or(0.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        out.push(byte as u8);
    }
    out
}
