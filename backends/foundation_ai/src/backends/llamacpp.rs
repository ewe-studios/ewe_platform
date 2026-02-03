//! `LlamaCPP` `ModelBackend` implementations.

use infrastructure_llama_cpp::context::params::LlamaContextParams;
use infrastructure_llama_cpp::llama_backend::LlamaBackend;
use infrastructure_llama_cpp::llama_batch::LlamaBatch;
use infrastructure_llama_cpp::model::params::kv_overrides::ParamOverrideValue;
use infrastructure_llama_cpp::model::params::{LlamaModelParams, LlamaSplitMode};
use infrastructure_llama_cpp::model::LlamaModel;
use infrastructure_llama_cpp::model::{AddBos, Special};
use infrastructure_llama_cpp::sampling::LlamaSampler;
use infrastructure_llama_cpp::{ggml_time_us, send_logs_to_tracing, LogOptions};

use std::ffi::CString;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::pin::pin;
use std::str::FromStr;
use std::time::Duration;

use crate::types::ModelBackend;

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
// Constructors
// ==================================

impl ModelBackend for LlamaBackends {
    fn get_model<T: crate::types::Model>(
        &self,
        _model_spec: crate::types::ModelSpec,
    ) -> crate::errors::ModelResult<T> {
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
