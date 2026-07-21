//! `LlamaBackendConfig` / `LlamaBackendConfigBuilder` — pure config plumbing.
//!
//! WHY: I had written these off as "GPU paths needing hardware". They are not.
//! `split_mode()`, `main_gpu()`, `n_gpu_layers()` and friends are plain setters
//! that assign a field — calling `main_gpu(2)` touches no GPU. Every one of them
//! was uncovered, and a setter that writes the wrong field (or is dropped by a
//! refactor) silently loads a model with the wrong offload, cache precision or
//! device, which surfaces much later as an OOM or a performance cliff.
//!
//! WHAT: each builder setter individually, the full chain, the defaults, and the
//! conversions into llama.cpp model/context params.
//!
//! HOW: entirely offline — no model file, no backend init, no GPU.

use std::path::PathBuf;

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, SpeculativeConfig};
use foundation_ai::types::{KVCacheType, SplitMode};

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

#[test]
fn new_matches_default() {
    // `new()` and `Default` are used interchangeably; divergence would mean a
    // model loads differently depending on which constructor a caller reached
    // for.
    let a = LlamaBackendConfig::new();
    let b = LlamaBackendConfig::default();
    assert_eq!(a.n_gpu_layers, b.n_gpu_layers);
    assert_eq!(a.context_length, b.context_length);
    assert_eq!(a.batch_size, b.batch_size);
    assert_eq!(a.use_mmap, b.use_mmap);
    assert_eq!(a.use_mlock, b.use_mlock);
}

#[test]
fn defaults_are_cpu_safe() {
    // The default must run anywhere: no GPU offload, mmap on (avoids reading the
    // whole file into RAM), mlock off (mlock can fail without privileges).
    let c = LlamaBackendConfig::default();
    assert_eq!(c.n_gpu_layers, 0, "default must not assume a GPU");
    assert!(c.use_mmap, "mmap keeps default memory use low");
    assert!(!c.use_mlock, "mlock requires privileges and must be opt-in");
    assert!(c.context_length > 0, "a zero context window is unusable");
}

#[test]
fn builder_default_matches_builder_new() {
    let a = LlamaBackendConfig::builder().build();
    let b = LlamaBackendConfig::default();
    assert_eq!(a.n_gpu_layers, b.n_gpu_layers);
    assert_eq!(a.context_length, b.context_length);
}

// ---------------------------------------------------------------------------
// Individual setters — each must write its OWN field and nothing else
// ---------------------------------------------------------------------------

#[test]
fn n_gpu_layers_setter() {
    let c = LlamaBackendConfig::builder().n_gpu_layers(32).build();
    assert_eq!(c.n_gpu_layers, 32);
}

#[test]
fn context_length_setter() {
    let c = LlamaBackendConfig::builder().context_length(8192).build();
    assert_eq!(c.context_length, 8192);
}

#[test]
fn batch_size_setter() {
    let c = LlamaBackendConfig::builder().batch_size(1024).build();
    assert_eq!(c.batch_size, 1024);
}

#[test]
fn n_threads_setter() {
    let c = LlamaBackendConfig::builder().n_threads(3).build();
    assert_eq!(c.n_threads, 3);
}

#[test]
fn use_mmap_setter_toggles_both_ways() {
    assert!(LlamaBackendConfig::builder().use_mmap(true).build().use_mmap);
    assert!(!LlamaBackendConfig::builder().use_mmap(false).build().use_mmap);
}

#[test]
fn use_mlock_setter_toggles_both_ways() {
    assert!(LlamaBackendConfig::builder().use_mlock(true).build().use_mlock);
    assert!(!LlamaBackendConfig::builder().use_mlock(false).build().use_mlock);
}

#[test]
fn kv_cache_type_setter() {
    let c = LlamaBackendConfig::builder()
        .kv_cache_type(KVCacheType::Q8_0)
        .build();
    assert_eq!(c.kv_cache_type, KVCacheType::Q8_0);
}

#[test]
fn split_mode_setter() {
    let c = LlamaBackendConfig::builder()
        .split_mode(SplitMode::Row)
        .build();
    assert_eq!(c.split_mode, SplitMode::Row);
}

#[test]
fn main_gpu_setter() {
    // Selecting a GPU index is pure config — it does not probe for the device.
    let c = LlamaBackendConfig::builder().main_gpu(2).build();
    assert_eq!(c.main_gpu, 2);
}

#[test]
fn speculative_setter_stores_the_config() {
    let spec = SpeculativeConfig::mtp(Some(PathBuf::from("/tmp/draft.gguf")), 4);
    let c = LlamaBackendConfig::builder().speculative(spec).build();
    assert!(
        c.speculative.is_some(),
        "an explicit SpeculativeConfig must be retained"
    );
}

#[test]
fn mtp_setter_enables_speculative_decoding() {
    let c = LlamaBackendConfig::builder()
        .mtp(Some(PathBuf::from("/tmp/draft.gguf")), 5)
        .build();
    assert!(
        c.speculative.is_some(),
        "mtp() is the convenience wrapper around speculative() and must set it"
    );
}

#[test]
fn speculative_is_absent_by_default() {
    // Speculative decoding must be opt-in — silently enabling it would change
    // generation behaviour for every caller.
    assert!(LlamaBackendConfig::default().speculative.is_none());
}

// ---------------------------------------------------------------------------
// The full chain — setters must not clobber one another
// ---------------------------------------------------------------------------

#[test]
fn the_full_builder_chain_preserves_every_field() {
    // The failure this guards: a setter assigning the wrong field. Chaining all
    // of them and checking each value catches a cross-wire that per-setter tests
    // would miss when both fields share a type.
    let c = LlamaBackendConfig::builder()
        .n_gpu_layers(24)
        .context_length(4096)
        .batch_size(512)
        .n_threads(6)
        .use_mmap(false)
        .use_mlock(true)
        .kv_cache_type(KVCacheType::F32)
        .split_mode(SplitMode::Layer)
        .main_gpu(1)
        .build();

    assert_eq!(c.n_gpu_layers, 24);
    assert_eq!(c.context_length, 4096);
    assert_eq!(c.batch_size, 512);
    assert_eq!(c.n_threads, 6);
    assert!(!c.use_mmap);
    assert!(c.use_mlock);
    assert_eq!(c.kv_cache_type, KVCacheType::F32);
    assert_eq!(c.split_mode, SplitMode::Layer);
    assert_eq!(c.main_gpu, 1);
}

#[test]
fn a_later_setter_wins_over_an_earlier_one() {
    let c = LlamaBackendConfig::builder()
        .context_length(1024)
        .context_length(2048)
        .build();
    assert_eq!(c.context_length, 2048, "the last write must win");
}

// ---------------------------------------------------------------------------
// Conversions into llama.cpp params
// ---------------------------------------------------------------------------

#[test]
fn to_model_params_does_not_panic_for_a_default_config() {
    // These build the FFI param structs. They take no GPU and no model file, so
    // they are reachable offline — and a panic here would abort every load.
    let _ = LlamaBackendConfig::default().to_model_params();
}

#[test]
fn to_model_params_does_not_panic_for_a_gpu_config() {
    let cfg = LlamaBackendConfig::builder()
        .n_gpu_layers(99)
        .split_mode(SplitMode::Row)
        .main_gpu(0)
        .build();
    let _ = cfg.to_model_params();
}

#[test]
fn to_context_params_does_not_panic_across_cache_types() {
    // Each KV cache type maps to a different ggml type in the context params;
    // an unmapped variant would panic at load time.
    for t in [
        KVCacheType::F32,
        KVCacheType::F16,
        KVCacheType::Q8_0,
        KVCacheType::Q5_0,
    ] {
        let cfg = LlamaBackendConfig::builder().kv_cache_type(t).build();
        let _ = cfg.to_context_params();
    }
}
