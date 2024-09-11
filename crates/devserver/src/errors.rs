use derive_more::From;

use crate::types::BoxedError;

// -- Errors

#[derive(Debug, From)]
pub enum StreamError {
    Failed,
    FailedStreaming(BoxedError),
}

impl std::error::Error for StreamError {}

impl core::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
