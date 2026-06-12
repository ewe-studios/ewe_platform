//! WHY: `js = "single-file"` apps want ONE file to ship — the WASM bytes ride
//! inside the JS, no second network round trip (feature 10 §5).
//!
//! WHAT: [`bundle_single_file`] — replaces the wrapper's `__EWE_WASM_LOAD__`
//! fetch block with embedded bytes, either a `Uint8Array` literal (default,
//! zero decode cost) or base64 + `atob` (smaller source text).
//!
//! HOW: Pure string substitution against the marker `js_wrapper` emits — the
//! wrappers and the bundler agree on exactly one seam.

use super::js_wrapper::load_marker;
use super::Encoding;

/// Embed `wasm_bytes` into `wrapper_js`, removing the fetch path entirely.
#[must_use]
pub fn bundle_single_file(wrapper_js: &str, wasm_bytes: &[u8], encoding: Encoding) -> String {
    let load = match encoding {
        Encoding::Uint8Array => {
            let mut literal = String::with_capacity(wasm_bytes.len() * 4 + 64);
            literal.push_str("  const wasmBytes = new Uint8Array([");
            for (i, byte) in wasm_bytes.iter().enumerate() {
                if i > 0 {
                    literal.push(',');
                }
                literal.push_str(itoa(*byte));
            }
            literal.push_str("]).buffer;");
            literal
        }
        Encoding::B64 => {
            let encoded = base64_encode(wasm_bytes);
            format!(
                "  const wasmBytes = Uint8Array.from(atob('{encoded}'), (c) => c.charCodeAt(0)).buffer;"
            )
        }
    };
    wrapper_js.replace(load_marker(), &load)
}

/// Tiny u8 → decimal str (avoids one million format! calls for big binaries).
fn itoa(byte: u8) -> &'static str {
    // 256-entry lookup built once.
    static TABLE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| (0u16..256).map(|v| v.to_string()).collect());
    &table[byte as usize]
}

/// Standard base64 (no external dep — 20 lines beats a crate here).
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
