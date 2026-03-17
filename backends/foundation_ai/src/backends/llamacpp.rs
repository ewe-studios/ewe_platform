//! `LlamaCPP` `ModelBackend` implementations.

use crate::types::{Model, ModelProvider};

/// [`LlamaBackends`] implements a model backend for different
/// underlying local backends for interacting with Large Language Models.
pub enum LlamaBackends {
    /// [`LLamaCPU`] defines the `LLamaCPP` CPU variant which
    /// focused on executing through the
    LLamaCPU,

    /// [`LLamaGPU`] defines the `LLamaCPP` GPU variant (which will use Vulkan or CUDA) which
    /// focused on executing through the
    LLamaGPU,

    /// [`LLamaMetal`] defines the `LLamaCPP` Apple Metal variant which
    /// focused on executing through the metal implementation.
    LLamaMetal,
}

// ==================================
// LlamaModels
// ==================================

pub enum LlamaModels {}

impl Model for LlamaModels {
    fn spec(&self) -> crate::types::ModelSpec {
        todo!()
    }

    fn costing(&self) -> crate::errors::GenerationResult<crate::types::UsageReport> {
        todo!()
    }

    fn generate(
        &self,
        interaction: crate::types::ModelInteraction,
        specs: Option<crate::types::ModelParams>,
    ) -> crate::errors::GenerationResult<Vec<crate::types::Messages>> {
        todo!()
    }

    fn stream<T>(
        &self,
        interaction: crate::types::ModelInteraction,
        specs: Option<crate::types::ModelParams>,
    ) -> crate::errors::GenerationResult<T>
    where
        T: foundation_core::valtron::StreamIterator<
            crate::types::Messages,
            crate::types::ModelState,
        >,
    {
        todo!()
    }
}

// ==================================
// Constructors
// ==================================

impl ModelProvider for LlamaBackends {
    type Model = LlamaModels;

    fn authenticate(
        self,
        credential: Option<foundation_auth::AuthCredential>,
    ) -> crate::errors::ModelProviderResult<Self>
    where
        Self: Sized,
    {
        Ok(self)
    }

    fn get_model<T: crate::types::Model>(
        &self,
        model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<T> {
        todo!()
    }

    fn get_model_by_spec<T: crate::types::Model>(
        &self,
        model_spec: crate::types::ModelSpec,
    ) -> crate::errors::ModelProviderResult<T> {
        todo!()
    }

    fn get_one(
        &self,
        model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<crate::types::ModelSpec> {
        todo!()
    }

    fn get_all(
        &self,
        model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<crate::types::ModelSpec> {
        todo!()
    }
}

// ==================================
// LLamaCPU implementation handler
// ==================================

impl LlamaBackends {
    fn get_llama_cpu_model<T: crate::types::Model>(
        &self,
        _model_spec: crate::types::ModelSpec,
    ) -> crate::errors::ModelResult<T> {
        // let llama_backend = LlamaBackend::init()?;
        // let model_params = LlamaDefault::default();
        todo!()
    }
}

// ==================================
// LLamaGPU implementation handler
// ==================================

// ==================================
// LLamaMetal implementation handler
// ==================================
