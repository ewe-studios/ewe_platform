//! Safe wrapper over the step-driven MTP speculative-decoding C shim
//! (`ewe_mtp_*`).
//!
//! WHY: modern models (Gemma 4, Qwen 3.6, GLM, …) ship a Multi-Token Prediction
//! (MTP) head as a separate draft GGUF that llama.cpp can use to draft several
//! tokens per target step. The engine lives behind a C++ `common_speculative`
//! API; this module wraps the `extern "C"` shim
//! (`infrastructure_llama_bindings::ewe_mtp_*`) in a safe, RAII handle.
//!
//! WHAT: [`LlamaMtp`] — owns the *reusable* engine (target + draft contexts, the
//! speculator). Build it once with [`LlamaMtp::new`], then run any number of
//! generations. Two ways to drive it:
//! * [`LlamaMtp::generate`] — one-shot, returns the full text + stats.
//! * [`LlamaMtp::begin`] + [`LlamaMtp::step`] — streaming: begin a generation,
//!   then pull one committed piece at a time.
//!
//! HOW: the shim keeps all the intricate C++ loop state; this wrapper marshals
//! params in and pieces/stats out, freeing C allocations.

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

/// Cumulative stats for a generation.
#[derive(Debug, Clone, Copy, Default)]
pub struct MtpStats {
    pub n_prompt_tokens: i32,
    pub n_generated: i32,
    pub n_drafted: i32,
    pub n_accepted: i32,
}

impl MtpStats {
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

/// Result of a one-shot MTP speculative generation.
#[derive(Debug, Clone)]
pub struct MtpGeneration {
    /// The generated text (chat-template prompt not included).
    pub text: String,
    /// Cumulative stats.
    pub stats: MtpStats,
}

impl MtpGeneration {
    /// Fraction of drafted tokens that were accepted.
    #[must_use]
    pub fn acceptance_rate(&self) -> f32 {
        self.stats.acceptance_rate()
    }

    /// Number of prompt tokens (convenience).
    #[must_use]
    pub fn n_prompt_tokens(&self) -> i32 {
        self.stats.n_prompt_tokens
    }

    /// Number of generated tokens (convenience).
    #[must_use]
    pub fn n_generated(&self) -> i32 {
        self.stats.n_generated
    }
}

/// One committed piece from a streaming step.
#[derive(Debug, Clone)]
pub struct MtpStep {
    /// Text committed this step (may be empty).
    pub piece: String,
    /// Whether the generation is complete after this step.
    pub done: bool,
}

/// Errors from constructing or running the MTP generator.
#[derive(Debug, thiserror::Error)]
pub enum MtpError {
    /// A string argument contained a NUL byte.
    #[error("{0}")]
    NulError(#[from] std::ffi::NulError),
    /// The shim could not initialize (draft load / context creation / MTP init
    /// failed, or the target context does not support the required operations).
    #[error("failed to initialize MTP speculative engine (draft load, context, or MTP init failed)")]
    InitFailed,
    /// `begin` failed (bad prompt, decode error, or a caught C++ exception).
    #[error("failed to begin MTP generation (bad prompt or decode error)")]
    BeginFailed,
    /// A `step` failed (decode/process error or a caught C++ exception).
    #[error("MTP speculative step failed")]
    StepFailed,
    /// A generated piece was not valid UTF-8.
    #[error("{0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// RAII handle to a reusable MTP speculative engine.
pub struct LlamaMtp {
    handle: *mut infrastructure_llama_bindings::ewe_mtp,
}

// SAFETY: the handle owns its own contexts; callers wrap it in a mutex (as the
// llama.cpp backend does) so it is never used from two threads at once. The
// underlying llama.cpp pointers are not otherwise shared.
unsafe impl Send for LlamaMtp {}

impl LlamaMtp {
    /// Build a reusable MTP engine over `target` using the MTP head GGUF at
    /// `draft_path`. `n_draft_max` is the max draft tokens proposed per step.
    ///
    /// This is the expensive step (loads the draft model, creates two contexts);
    /// keep the returned engine and reuse it across generations.
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
    ) -> Result<Self, MtpError> {
        let draft = CString::new(draft_path.to_string_lossy().as_bytes())?;
        let handle = unsafe {
            infrastructure_llama_bindings::ewe_mtp_init(
                target.model.as_ptr(),
                draft.as_ptr(),
                n_ctx,
                n_batch,
                n_threads,
                n_draft_max,
            )
        };
        if handle.is_null() {
            return Err(MtpError::InitFailed);
        }
        Ok(Self { handle })
    }

    /// Begin a new generation for a (chat-templated) `prompt`, up to `n_predict`
    /// tokens. Resets the KV cache and rebuilds the sampler. Returns the prompt
    /// token count.
    ///
    /// # Errors
    /// Returns [`MtpError::BeginFailed`] if the prompt is invalid or the prompt
    /// decode fails.
    pub fn begin(
        &self,
        prompt: &str,
        n_predict: i32,
        sampling: MtpSampling,
    ) -> Result<i32, MtpError> {
        let c_prompt = CString::new(prompt)?;
        let c_sampling = infrastructure_llama_bindings::ewe_mtp_sampling {
            temperature: sampling.temperature,
            top_k: sampling.top_k,
            top_p: sampling.top_p,
            repeat_penalty: sampling.repeat_penalty,
            seed: sampling.seed,
        };
        let n = unsafe {
            infrastructure_llama_bindings::ewe_mtp_begin(
                self.handle,
                c_prompt.as_ptr(),
                n_predict,
                c_sampling,
            )
        };
        if n < 0 {
            return Err(MtpError::BeginFailed);
        }
        Ok(n)
    }

    /// Advance one speculative step, returning the piece committed this step and
    /// whether generation is complete. Call after [`LlamaMtp::begin`].
    ///
    /// # Errors
    /// Returns [`MtpError::StepFailed`] on a decode/process error, or
    /// [`MtpError::Utf8`] if the piece is not valid UTF-8.
    pub fn step(&self) -> Result<MtpStep, MtpError> {
        let mut out_piece: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = unsafe {
            infrastructure_llama_bindings::ewe_mtp_step(self.handle, &mut out_piece)
        };
        if rc < 0 {
            if !out_piece.is_null() {
                unsafe { infrastructure_llama_bindings::ewe_chat_string_free(out_piece) };
            }
            return Err(MtpError::StepFailed);
        }
        let piece = if out_piece.is_null() {
            String::new()
        } else {
            let s = unsafe { CStr::from_ptr(out_piece) }
                .to_str()
                .map(std::borrow::ToOwned::to_owned);
            unsafe { infrastructure_llama_bindings::ewe_chat_string_free(out_piece) };
            s?
        };
        Ok(MtpStep {
            piece,
            done: rc == 0,
        })
    }

    /// Current cumulative stats.
    #[must_use]
    pub fn stats(&self) -> MtpStats {
        let mut s = MtpStats::default();
        unsafe {
            infrastructure_llama_bindings::ewe_mtp_stats(
                self.handle,
                &mut s.n_prompt_tokens,
                &mut s.n_generated,
                &mut s.n_drafted,
                &mut s.n_accepted,
            );
        }
        s
    }

    /// One-shot generation: begin + drive steps to completion, returning the
    /// full text and stats.
    ///
    /// # Errors
    /// Returns [`MtpError`] if begin or any step fails.
    pub fn generate(
        &self,
        prompt: &str,
        n_predict: i32,
        sampling: MtpSampling,
    ) -> Result<MtpGeneration, MtpError> {
        self.begin(prompt, n_predict, sampling)?;
        let mut text = String::new();
        loop {
            let step = self.step()?;
            text.push_str(&step.piece);
            if step.done {
                break;
            }
        }
        Ok(MtpGeneration {
            text,
            stats: self.stats(),
        })
    }
}

impl Drop for LlamaMtp {
    fn drop(&mut self) {
        unsafe { infrastructure_llama_bindings::ewe_mtp_free(self.handle) };
    }
}
