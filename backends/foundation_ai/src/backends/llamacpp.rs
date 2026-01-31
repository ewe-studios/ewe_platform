//! LlamaCPP ModelBackend implementations.

use crate::types::ModelBackend;

/// [`LlamaBackends`] implements a model backend for different
/// underlying local backends for interacting with Large Language Models.
pub enum LlamaBackends {
    /// [`LLamaCPU`] defines the LLamaCPP CPU variant which
    /// focused on executing through the
    LLamaCPU,

    /// [`LLamaGPU`] defines the LLamaCPP GPU variant (which will use Vulkan or CUDA) which
    /// focused on executing through the
    LLamaGPU,

    /// [`LLamaMetal`] defines the LLamaCPP Apple Metal variant which
    /// focused on executing through the metal implementation.
    LLamaMetal,
}

impl ModelBackend for LlamaBackends {
    fn get_model<T: crate::types::Model>(
        &self,
        model_spec: crate::types::ModelSpec,
    ) -> crate::errors::ModelResult<T> {
        todo!()
    }
}
