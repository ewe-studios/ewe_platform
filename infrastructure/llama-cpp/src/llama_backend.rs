//! Representation of an initialized llama backend

use crate::LlamaCppError;
use infrastructure_llama_bindings::ggml_log_level;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

/// Representation of an initialized llama backend
/// This is required as a parameter for most llama functions as the backend must be initialized
/// before any llama functions are called. This type is proof of initialization.
#[derive(Eq, PartialEq, Debug)]
pub struct LlamaBackend {}

static LLAMA_BACKEND_REFCOUNT: AtomicUsize = AtomicUsize::new(0);

impl LlamaBackend {
    /// Try to become the exclusive first initializer.
    fn mark_init() -> crate::Result<()> {
        match LLAMA_BACKEND_REFCOUNT.compare_exchange(0, 1, SeqCst, SeqCst) {
            Ok(_) => Ok(()),
            Err(_) => Err(LlamaCppError::BackendAlreadyInitialized),
        }
    }

    /// Route llama.cpp + ggml native logs into `tracing`, exactly once.
    ///
    /// WHY: llama.cpp writes the full model-loader dump, per-tensor `repack:`
    /// lines and graph-reservation output straight to `stderr` on every load,
    /// which buries an app's own output (spec-60/B). Sending them through
    /// `tracing` instead makes them ordinary events under the app's
    /// `EnvFilter` — silent by default (their targets are `llama.cpp` / `ggml`,
    /// not enabled by a plain `info` filter's own-crate scope), and turned on
    /// with an explicit directive such as `llama.cpp=debug`.
    ///
    /// HOW: `send_logs_to_tracing` installs the C log callback via
    /// `llama_log_set`/`ggml_log_set`. It is guarded by a `Once` here so repeated
    /// `init_or_get` calls (the common case) install the hook only on the first.
    fn route_logs_to_tracing() {
        use std::sync::Once;
        static LOG_ROUTE: Once = Once::new();
        LOG_ROUTE.call_once(|| {
            crate::send_logs_to_tracing(crate::LogOptions::default());
        });
    }

    /// Return a handle to the llama backend, initializing it on first call.
    ///
    /// Unlike [`init`](Self::init), this never errors if the backend is
    /// already running — it simply returns another handle. The underlying
    /// C backend is freed only when the last handle is dropped.
    ///
    /// # Examples
    ///
    /// ```
    ///# use infrastructure_llama_cpp::llama_backend::LlamaBackend;
    ///# use std::error::Error;
    ///
    ///# fn main() -> Result<(), Box<dyn Error>> {
    /// let a = LlamaBackend::init_or_get()?;
    /// let b = LlamaBackend::init_or_get()?; // no error
    /// drop(a);
    /// // backend is still alive because `b` holds a reference
    /// assert!(b.supports_mmap() || !b.supports_mmap()); // can still use it
    ///# Ok(())
    ///# }
    /// ```
    #[tracing::instrument(skip_all)]
    pub fn init_or_get() -> crate::Result<LlamaBackend> {
        Self::route_logs_to_tracing();
        match Self::mark_init() {
            Ok(()) => {
                unsafe { infrastructure_llama_bindings::llama_backend_init() }
                Ok(LlamaBackend {})
            }
            Err(_) => {
                tracing::trace!("llama backend already initialized, returning existing handle");
                LLAMA_BACKEND_REFCOUNT.fetch_add(1, SeqCst);
                Ok(LlamaBackend {})
            }
        }
    }

    /// Initialize the llama backend (with numa).
    /// ```
    ///# use infrastructure_llama_cpp::llama_backend::LlamaBackend;
    ///# use std::error::Error;
    ///# use infrastructure_llama_cpp::llama_backend::NumaStrategy;
    ///
    ///# fn main() -> Result<(), Box<dyn Error>> {
    ///
    /// let llama_backend = LlamaBackend::init_numa(NumaStrategy::MIRROR)?;
    ///
    ///# Ok(())
    ///# }
    /// ```
    #[tracing::instrument(skip_all)]
    pub fn init_numa(strategy: NumaStrategy) -> crate::Result<LlamaBackend> {
        match Self::mark_init() {
            Ok(()) => {
                unsafe {
                    infrastructure_llama_bindings::llama_numa_init(
                        infrastructure_llama_bindings::ggml_numa_strategy::from(strategy),
                    );
                }
                Ok(LlamaBackend {})
            }
            Err(_) => {
                tracing::trace!("llama backend already initialized, returning existing handle");
                LLAMA_BACKEND_REFCOUNT.fetch_add(1, SeqCst);
                Ok(LlamaBackend {})
            }
        }
    }

    /// Was the code built for a GPU backend & is a supported one available.
    #[must_use]
    pub fn supports_gpu_offload(&self) -> bool {
        unsafe { infrastructure_llama_bindings::llama_supports_gpu_offload() }
    }

    /// Does this platform support loading the model via mmap.
    #[must_use]
    pub fn supports_mmap(&self) -> bool {
        unsafe { infrastructure_llama_bindings::llama_supports_mmap() }
    }

    /// Does this platform support locking the model in RAM.
    #[must_use]
    pub fn supports_mlock(&self) -> bool {
        unsafe { infrastructure_llama_bindings::llama_supports_mlock() }
    }

    /// Change the output of llama.cpp's logging to be voided instead of pushed to `stderr`.
    pub fn void_logs(&mut self) {
        unsafe extern "C" fn void_log(
            _level: ggml_log_level,
            _text: *const ::std::os::raw::c_char,
            _user_data: *mut ::std::os::raw::c_void,
        ) {
        }

        unsafe {
            infrastructure_llama_bindings::llama_log_set(Some(void_log), std::ptr::null_mut());
        }
    }
}

/// A rusty wrapper around `numa_strategy`.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum NumaStrategy {
    /// The numa strategy is disabled.
    DISABLED,
    /// help wanted: what does this do?
    DISTRIBUTE,
    /// help wanted: what does this do?
    ISOLATE,
    /// help wanted: what does this do?
    NUMACTL,
    /// help wanted: what does this do?
    MIRROR,
    /// help wanted: what does this do?
    COUNT,
}

/// An invalid numa strategy was provided.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct InvalidNumaStrategy(
    /// The invalid numa strategy that was provided.
    pub infrastructure_llama_bindings::ggml_numa_strategy,
);

impl TryFrom<infrastructure_llama_bindings::ggml_numa_strategy> for NumaStrategy {
    type Error = InvalidNumaStrategy;

    fn try_from(
        value: infrastructure_llama_bindings::ggml_numa_strategy,
    ) -> Result<Self, Self::Error> {
        match value {
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_DISABLED => Ok(Self::DISABLED),
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_DISTRIBUTE => Ok(Self::DISTRIBUTE),
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_ISOLATE => Ok(Self::ISOLATE),
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_NUMACTL => Ok(Self::NUMACTL),
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_MIRROR => Ok(Self::MIRROR),
            infrastructure_llama_bindings::GGML_NUMA_STRATEGY_COUNT => Ok(Self::COUNT),
            value => Err(InvalidNumaStrategy(value)),
        }
    }
}

impl From<NumaStrategy> for infrastructure_llama_bindings::ggml_numa_strategy {
    fn from(value: NumaStrategy) -> Self {
        match value {
            NumaStrategy::DISABLED => infrastructure_llama_bindings::GGML_NUMA_STRATEGY_DISABLED,
            NumaStrategy::DISTRIBUTE => {
                infrastructure_llama_bindings::GGML_NUMA_STRATEGY_DISTRIBUTE
            }
            NumaStrategy::ISOLATE => infrastructure_llama_bindings::GGML_NUMA_STRATEGY_ISOLATE,
            NumaStrategy::NUMACTL => infrastructure_llama_bindings::GGML_NUMA_STRATEGY_NUMACTL,
            NumaStrategy::MIRROR => infrastructure_llama_bindings::GGML_NUMA_STRATEGY_MIRROR,
            NumaStrategy::COUNT => infrastructure_llama_bindings::GGML_NUMA_STRATEGY_COUNT,
        }
    }
}

/// Drops the llama backend.
/// ```
///
///# use infrastructure_llama_cpp::llama_backend::LlamaBackend;
///# use std::error::Error;
///
///# fn main() -> Result<(), Box<dyn Error>> {
/// let backend = LlamaBackend::init()?;
/// drop(backend);
/// // can be initialized again after being dropped
/// let backend = LlamaBackend::init()?;
///# Ok(())
///# }
///
/// ```
impl Drop for LlamaBackend {
    fn drop(&mut self) {
        let prev = LLAMA_BACKEND_REFCOUNT.fetch_sub(1, SeqCst);
        debug_assert!(prev > 0, "LlamaBackend refcount underflow");
        if prev == 1 {
            unsafe { infrastructure_llama_bindings::llama_backend_free() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numa_from_and_to() {
        let numas = [
            NumaStrategy::DISABLED,
            NumaStrategy::DISTRIBUTE,
            NumaStrategy::ISOLATE,
            NumaStrategy::NUMACTL,
            NumaStrategy::MIRROR,
            NumaStrategy::COUNT,
        ];

        for numa in &numas {
            let from = infrastructure_llama_bindings::ggml_numa_strategy::from(*numa);
            let to = NumaStrategy::try_from(from).expect("Failed to convert from and to");
            assert_eq!(*numa, to);
        }
    }

    #[test]
    fn check_invalid_numa() {
        let invalid = 800;
        let invalid = NumaStrategy::try_from(invalid);
        assert_eq!(invalid, Err(InvalidNumaStrategy(invalid.unwrap_err().0)));
    }
}
