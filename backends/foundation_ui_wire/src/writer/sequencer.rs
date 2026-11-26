/// Stream, sequence and acknowledgement identifiers for one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub stream_id: u64,
    pub sequence_id: u64,
    pub ack: u64,
}

/// Advances the frame sequence for one stream.
#[derive(Debug)]
pub struct StreamSequencer {
    stream_id: u64,
    last: u64,
}

impl StreamSequencer {
    #[must_use]
    pub fn new(stream_id: u64) -> Self {
        Self::resume(stream_id, 0)
    }

    #[must_use]
    pub fn resume(stream_id: u64, last: u64) -> Self {
        Self { stream_id, last }
    }

    #[must_use]
    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }

    #[must_use]
    pub fn last(&self) -> u64 {
        self.last
    }

    /// Advances the counter and saturates at the largest sequence number.
    #[expect(
        clippy::should_implement_trait,
        reason = "The writer API returns a sequence number even after the counter saturates."
    )]
    pub fn next(&mut self) -> u64 {
        self.last = self.last.saturating_add(1);
        self.last
    }

    /// Advances the counter and builds the frame header. Pass zero for a push.
    pub fn header(&mut self, ack: u64) -> FrameHeader {
        FrameHeader {
            stream_id: self.stream_id,
            sequence_id: self.next(),
            ack,
        }
    }
}
