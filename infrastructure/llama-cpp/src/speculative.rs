//! Safe wrapper over the MTP speculative-decoding C shim (`ewe_mtp_*`).
//!
//! WHY: modern models (Gemma 4, Qwen 3.6, GLM, …) ship a Multi-Token Prediction
//! (MTP) head as a separate draft GGUF that llama.cpp can use to draft several
//! tokens per target step. The engine lives behind a C++ `common_speculative`
//! API; this module wraps the `extern "C"` shim ([`crate`]'s
//! `infrastructure_llama_bindings::ewe_mtp_*`) in a safe, RAII handle.
//!
//! WHAT: [`LlamaMtp`] — owns the shim generator (target + draft contexts, the
//! speculator, its sampler). Construct with [`LlamaMtp::new`] from an already
//! loaded [`LlamaModel`] plus the MTP head GGUF path; call
//! [`LlamaMtp::generate`] with a (chat-templated) prompt string to run
//! speculative generation.
//!
//! HOW: the shim runs the whole single-sequence draft→verify→accept loop in
//! C++; this wrapper just marshals params in and the generated string + stats
//! out, freeing the C allocation.

use std::ffi::{CStr, CString};
use std::path::Path;

use crate::model::LlamaModel;

/// Sampling parameters passed to the MTP generator.
#[derive(Debug, Clone, Copy)]
pub struct MtpSampling {
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub seed: u32,
}

impl Default for MtpSampling {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.0,
            seed: 0xFFFF_FFFF, // LLAMA_DEFAULT_SEED
        }
    }
}

/// Result of an MTP speculative generation.
#[derive(Debug, Clone)]
pub struct MtpGeneration {
    /// The generated text (chat-template prompt not included).
    pub text: String,
    /// Number of prompt tokens.
    pub n_prompt_tokens: i32,
    /// Number of generated tokens.
    pub n_generated: i32,
    /// Total draft tokens proposed across all steps.
    pub n_drafted: i32,
    /// Total draft tokens accepted across all steps.
    pub n_accepted: i32,
}

impl MtpGeneration {
    /// Fraction of drafted tokens that were accepted (0.0 if none drafted).
    #[must_use]
    pub fn acceptance_rate(&self) -> f32 {
        if self.n_drafted <= 0 {
            0.0
        } else {
            self.n_accepted as f32 / self.n_drafted as f32
        }
    }
}

/// Errors from constructing or running the MTP generator.
#[derive(Debug, thiserror::Error)]
pub enum MtpError {
    /// The draft path contained a NUL byte.
    #[error("{0}")]
    NulError(#[from] std::ffi::NulError),
    /// The shim could not initialize (draft load / context creation / MTP init
    /// failed, or the target context does not support the required operations).
    #[error("failed to initialize MTP speculative generator (draft load, context, or MTP init failed)")]
    InitFailed,
    /// Generation failed (bad prompt, decode error, or a caught C++ exception).
    #[error("MTP speculative generation failed")]
    GenerateFailed,
    /// The generated text was not valid UTF-8.
    #[error("{0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// RAII handle to an MTP speculative generator.
pub struct LlamaMtp {
    handle: *mut infrastructure_llama_bindings::ewe_mtp,
}

// SAFETY: the handle owns its own contexts, protected by exclusive `&mut`-free
// use — callers wrap it in a mutex (as `LlamaModels` does). The underlying
// llama.cpp pointers are not otherwise shared.
unsafe impl Send for LlamaMtp {}

impl LlamaMtp {
    /// Create an MTP generator over `target` using the MTP head GGUF at
    /// `draft_path`. `n_draft_max` is the max draft tokens proposed per step.
    ///
    /// # Errors
    /// Returns [`MtpError::InitFailed`] if the shim cannot initialize.
    pub fn new(
        target: &LlamaModel,
        draft_path: &Path,
        n_ctx: u32,
        n_batch: u32,
        n_threads: i32,
        n_draft_max: i32,
        sampling: MtpSampling,
    ) -> Result<Self, MtpError> {
        let draft = CString::new(draft_path.to_string_lossy().as_bytes())?;
        let c_sampling = infrastructure_llama_bindings::ewe_mtp_sampling {
            temperature: sampling.temperature,
            top_k: sampling.top_k,
            top_p: sampling.top_p,
            repeat_penalty: sampling.repeat_penalty,
            seed: sampling.seed,
        };
        let handle = unsafe {
            infrastructure_llama_bindings::ewe_mtp_init(
                target.model.as_ptr(),
                draft.as_ptr(),
                n_ctx,
                n_batch,
                n_threads,
                n_draft_max,
                c_sampling,
            )
        };
        if handle.is_null() {
            return Err(MtpError::InitFailed);
        }
        Ok(Self { handle })
    }

    /// Run speculative generation for a (chat-templated) `prompt`, producing up
    /// to `n_predict` tokens.
    ///
    /// # Errors
    /// Returns [`MtpError`] if the prompt is invalid, generation fails, or the
    /// output is not valid UTF-8.
    pub fn generate(&self, prompt: &str, n_predict: i32) -> Result<MtpGeneration, MtpError> {
        let c_prompt = CString::new(prompt)?;
        let mut n_prompt_tokens: i32 = 0;
        let mut n_generated: i32 = 0;
        let mut n_drafted: i32 = 0;
        let mut n_accepted: i32 = 0;

        let raw = unsafe {
            infrastructure_llama_bindings::ewe_mtp_generate(
                self.handle,
                c_prompt.as_ptr(),
                n_predict,
                &mut n_prompt_tokens,
                &mut n_generated,
                &mut n_drafted,
                &mut n_accepted,
            )
        };
        if raw.is_null() {
            return Err(MtpError::GenerateFailed);
        }

        let text = unsafe { CStr::from_ptr(raw) }
            .to_str()
            .map(std::borrow::ToOwned::to_owned);
        // The shim allocates the result with malloc; the chat shim's free is the
        // matching deallocator (both use the C runtime's free).
        unsafe { infrastructure_llama_bindings::ewe_chat_string_free(raw) };

        Ok(MtpGeneration {
            text: text?,
            n_prompt_tokens,
            n_generated,
            n_drafted,
            n_accepted,
        })
    }
}

impl Drop for LlamaMtp {
    fn drop(&mut self) {
        unsafe { infrastructure_llama_bindings::ewe_mtp_free(self.handle) };
    }
}
