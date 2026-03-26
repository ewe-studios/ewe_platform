#![allow(dead_code)]

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
        _interaction: crate::types::ModelInteraction,
        _specs: Option<crate::types::ModelParams>,
    ) -> crate::errors::GenerationResult<Vec<crate::types::Messages>> {
        todo!()
    }

    fn stream<T>(
        &self,
        _interaction: crate::types::ModelInteraction,
        _specs: Option<crate::types::ModelParams>,
    ) -> crate::errors::GenerationResult<T>
    where
        T: foundation_core::valtron::StreamIterator<
            D = crate::types::Messages,
            P = crate::types::ModelState,
        >,
    {
        todo!()
    }
}

// ==================================
// Constructors
// ==================================

impl ModelProvider for LlamaBackends {
    // we will update config to match the sensible things LLamaCpp requires
    type Config = ();
    type Model = LlamaModels;

    fn create(
        self,
        _config: Option<Self::Config>,
        _credential: Option<foundation_auth::AuthCredential>,
    ) -> crate::errors::ModelProviderResult<Self>
    where
        Self: Sized,
    {
        Ok(self)
    }

    fn get_model(
        &self,
        _model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<Self::Model> {
        todo!()
    }

    fn get_model_by_spec(
        &self,
        _model_spec: crate::types::ModelSpec,
    ) -> crate::errors::ModelProviderResult<Self::Model> {
        todo!()
    }

    fn get_one(
        &self,
        _model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<crate::types::ModelSpec> {
        todo!()
    }

    fn get_all(
        &self,
        _model_id: crate::types::ModelId,
    ) -> crate::errors::ModelProviderResult<crate::types::ModelSpec> {
        todo!()
    }

    fn describe(
        &self,
    ) -> crate::errors::ModelProviderResult<crate::types::ModelProviderDescriptor> {
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
