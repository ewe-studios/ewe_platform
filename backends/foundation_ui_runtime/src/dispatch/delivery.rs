use foundation_ui_wire::writer::{
    write_messages, ClientCaps, FrameHeader, WriteError, WriterConfig,
};
use foundation_ui_wire::MessageT;

/// Encodes signed action output for one client and its negotiated capabilities.
pub trait DeliveryHandler: Send + Sync {
    /// Writes one frame with the supplied header. Apply the client's capability gate during encoding.
    ///
    /// # Errors
    ///
    /// Return a writer error if the messages cannot be delivered to this client.
    fn messages(
        &self,
        messages: Vec<MessageT>,
        caps: &ClientCaps,
        header: FrameHeader,
    ) -> Result<Vec<u8>, WriteError>;
}

/// Delivers a frame with the shared writer and its capability gate.
#[derive(Clone, Debug, Default)]
pub struct DefaultDelivery {
    config: WriterConfig,
}

impl DefaultDelivery {
    #[must_use]
    pub fn new(config: WriterConfig) -> Self {
        Self { config }
    }
}

impl DeliveryHandler for DefaultDelivery {
    fn messages(
        &self,
        messages: Vec<MessageT>,
        caps: &ClientCaps,
        header: FrameHeader,
    ) -> Result<Vec<u8>, WriteError> {
        write_messages(messages, caps, header, &self.config).map(|(bytes, _report)| bytes)
    }
}
