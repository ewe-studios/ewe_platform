//! Memory-backed stream for wasm request/response dispatch.
//!
//! `WasmStream` collects raw HTTP/1.1 wire format from `ServeWriter` handlers
//! and parses it into structured `WasmResponse` (status, headers, body).

/// A write-only stream that collects HTTP wire format and parses it
/// into structured response components.
pub struct WasmStream {
    buffer: Vec<u8>,
    parsed: Option<ParsedResponse>,
}

/// Structured components of a parsed HTTP response.
pub struct ParsedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl WasmStream {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            parsed: None,
        }
    }

    /// Parse the collected HTTP wire format into a structured `WasmResponse`.
    pub fn into_wasm_response(mut self) -> crate::wasm::response::WasmResponse {
        if self.parsed.is_none() {
            self.parsed = Some(parse_raw(&self.buffer));
        }
        let parsed = self.parsed.unwrap();

        crate::wasm::response::WasmResponse {
            status: parsed.status,
            body: Some(parsed.body),
            headers: parsed.headers,
        }
    }

    /// Consume the stream and return the collected response bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }
}

impl Default for WasmStream {
    fn default() -> Self {
        Self::new()
    }
}

impl std::io::Read for WasmStream {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

impl std::io::Write for WasmStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.parsed.is_none() {
            self.parsed = Some(parse_raw(&self.buffer));
        }
        Ok(())
    }
}

/// Parse raw HTTP/1.1 wire format into structured components.
pub fn parse_raw(bytes: &[u8]) -> ParsedResponse {
    let mut status: u16 = 200;
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body_offset: usize = bytes.len();

    if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        body_offset = pos + 4;
        let header_section = &bytes[..pos];

        // Split header section on \r\n using windows
        let mut start = 0usize;
        let mut lines = Vec::new();
        for i in 0..header_section.len().saturating_sub(1) {
            if header_section[i..i + 2] == b"\r\n"[..] {
                lines.push(&header_section[start..i]);
                start = i + 2;
            }
        }
        if start < header_section.len() {
            lines.push(&header_section[start..]);
        }

        let mut line_iter = lines.into_iter();

        // Status line: HTTP/1.1 {code} {reason}
        if let Some(status_line) = line_iter.next() {
            let mut parts = status_line.split(|b| *b == b' ');
            if let Some(_proto) = parts.next() {
                if let Some(code_bytes) = parts.next() {
                    if let Ok(code_str) = std::str::from_utf8(code_bytes) {
                        if let Ok(code) = code_str.parse::<u16>() {
                            status = code;
                        }
                    }
                }
            }
        }

        // Headers
        for line in line_iter {
            if line.is_empty() {
                continue;
            }
            if let Some(colon_pos) = line.iter().position(|b| *b == b':') {
                if colon_pos + 1 < line.len() && line[colon_pos + 1] == b' ' {
                    let name = std::str::from_utf8(&line[..colon_pos]).unwrap_or("").to_string();
                    let value = std::str::from_utf8(&line[colon_pos + 2..]).unwrap_or("").to_string();
                    headers.push((name, value));
                }
            }
        }
    }

    let body = bytes[body_offset..].to_vec();

    ParsedResponse {
        status,
        headers,
        body,
    }
}
