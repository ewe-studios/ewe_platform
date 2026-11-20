uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
pub struct PushInput {
    pub bytes: Vec<u8>,
}

#[derive(Debug, uniffi::Error)]
pub enum ProofError {
    EmptyInput,
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => formatter.write_str("Input has no bytes"),
        }
    }
}

impl std::error::Error for ProofError {}

#[uniffi::export]
pub async fn push_ops(input: PushInput) -> Result<Vec<u8>, ProofError> {
    if input.bytes.is_empty() {
        return Err(ProofError::EmptyInput);
    }
    Ok(input.bytes)
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait HostTransport: Send + Sync {
    async fn send(&self, input: Vec<u8>) -> Result<Vec<u8>, ProofError>;
}
