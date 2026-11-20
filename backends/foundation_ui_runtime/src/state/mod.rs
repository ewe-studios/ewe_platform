//! Host-selected state signing and verification.

mod config;
mod error;
mod keys;
mod signers;

use std::sync::Arc;

use foundation_context::Ctx;

pub use config::StateConfig;
pub use error::StateError;
pub use keys::{StateKey, StateKeyRing};
pub use signers::{ChaChaStateSigner, HmacStateSigner};

/// Signs state in a format chosen by the host.
pub trait StateSigner: Send + Sync {
    /// # Errors
    ///
    /// Returns an error when the state cannot be signed.
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError>;

    /// # Errors
    ///
    /// Returns an error when the supplied bytes fail verification for this binding.
    fn verify(
        &self,
        ctx: &Ctx,
        binding: Option<&[u8]>,
        signed: &[u8],
    ) -> Result<Vec<u8>, StateError>;
}

/// The signer and optional binding used by this host or request.
pub struct SignerEntry {
    signer: Arc<dyn StateSigner>,
    binding: Option<Arc<[u8]>>,
}

/// Installs a signer for public state in the host context.
pub fn install_state_signer(ctx: &Ctx, signer: Arc<dyn StateSigner>) {
    ctx.store(SignerEntry {
        signer,
        binding: None,
    });
}

/// Forks the supplied context and installs a request-specific binding.
///
/// # Errors
///
/// Returns a missing signer error when the context has no signer.
pub fn with_state_binding(ctx: &Ctx, value: &[u8]) -> Result<Ctx, StateError> {
    let child = ctx.fork();
    let entry = signer_entry(&child)?;
    child.store(SignerEntry {
        signer: Arc::clone(&entry.signer),
        binding: Some(Arc::from(value)),
    });
    Ok(child)
}

/// Signs a body with the signer and binding in the context.
///
/// # Errors
///
/// Returns a missing signer error or the selected signer's error.
pub fn sign_state(ctx: &Ctx, body: &[u8]) -> Result<Vec<u8>, StateError> {
    let entry = signer_entry(ctx)?;
    entry.signer.sign(ctx, entry.binding.as_deref(), body)
}

/// Returns the body after verifying it with the context's signer and binding.
///
/// # Errors
///
/// Returns a missing signer error or the selected signer's error.
pub fn verify_state(ctx: &Ctx, signed: &[u8]) -> Result<Vec<u8>, StateError> {
    let entry = signer_entry(ctx)?;
    entry.signer.verify(ctx, entry.binding.as_deref(), signed)
}

fn signer_entry(ctx: &Ctx) -> Result<Arc<SignerEntry>, StateError> {
    ctx.get::<SignerEntry>().ok_or(StateError::MissingSigner)
}
