/// Size, depth and table limits for a wire read.
///
/// The default frame size is four mebibytes, the gRPC Go receive limit.
/// The default action request size is one mebibyte, the nginx client body limit.
/// The depth limit is 64 and the table limit is 1,000,000, the `FlatBuffers` verifier defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireLimits {
    frame_bytes: usize,
    action_request_bytes: usize,
    depth: usize,
    tables: usize,
}

impl Default for WireLimits {
    fn default() -> Self {
        let verifier = flatbuffers::VerifierOptions::default();
        Self {
            frame_bytes: 4 * 1024 * 1024,
            action_request_bytes: 1024 * 1024,
            depth: verifier.max_depth,
            tables: verifier.max_tables,
        }
    }
}

impl WireLimits {
    /// Sets the maximum size of a frame in bytes. A zero limit becomes one.
    #[must_use]
    pub fn max_frame_bytes(mut self, limit: usize) -> Self {
        self.frame_bytes = limit.max(1);
        self
    }

    /// Sets the maximum size of an action request or signed state in bytes. A zero limit becomes one.
    #[must_use]
    pub fn max_action_request_bytes(mut self, limit: usize) -> Self {
        self.action_request_bytes = limit.max(1);
        self
    }

    /// Sets the maximum depth of nested tables. A zero limit becomes one.
    #[must_use]
    pub fn max_depth(mut self, limit: usize) -> Self {
        self.depth = limit.max(1);
        self
    }

    /// Sets the maximum number of tables. A zero limit becomes one.
    #[must_use]
    pub fn max_tables(mut self, limit: usize) -> Self {
        self.tables = limit.max(1);
        self
    }

    #[must_use]
    pub fn frame_bytes(&self) -> usize {
        self.frame_bytes
    }

    #[must_use]
    pub fn action_request_bytes(&self) -> usize {
        self.action_request_bytes
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub fn tables(&self) -> usize {
        self.tables
    }
}
