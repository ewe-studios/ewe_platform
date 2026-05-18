//! Memory-backed stream for wasm request/response dispatch.

/// A write-only stream that collects response bytes in memory.
///
/// The `Read` impl returns `Ok(0)` immediately since wasm requests
/// have their full body available in `SimpleIncomingRequest`.
pub struct WasmStream {
    response: Vec<u8>,
}

impl WasmStream {
    #[must_use]
    pub fn new() -> Self {
        Self { response: Vec::new() }
    }

    /// Consume the stream and return the collected response bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.response
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
        self.response.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
