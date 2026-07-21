//! `HuggingFaceCandleProvider::download_model` — the cache short-circuit.
//!
//! WHY: `download_model` returns early when the destination already holds a
//! `config.json` plus at least one `.safetensors` file. That branch is what
//! keeps a second load from re-fetching gigabytes over the network, and it was
//! uncovered — the live suite always exercises the *download* path. A broken
//! cache check is invisible in tests and expensive in production: every model
//! load silently re-downloads.
//!
//! WHAT: the cache hit, and each way it must MISS (no config.json, no
//! safetensors, neither) — a check that is too eager is worse than one that is
//! too lazy, because it would hand back a half-populated directory that then
//! fails deep inside the loader.
//!
//! HOW: a temp cache dir seeded with empty marker files. The HIT tests are
//! deliberately plain `#[test]` with no valtron pool — they pass only because
//! `download_model` returns before touching the HF client at all, which is
//! itself the property under test. The MISS tests do reach the client, so they
//! need `#[valtron_test]` for the pool; the repo id is fictional, so the fetch
//! fails fast either way and the assertion is only that a half-populated
//! directory is never handed back as a hit.

use std::path::{Path, PathBuf};

use foundation_core::valtron::valtron_test;

use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const REPO: &str = "some-org/some-model";

/// `download_model` maps `org/model` -> `<cache>/org--model`.
fn cache_subdir(cache: &Path) -> PathBuf {
    cache.join(REPO.replace('/', "--"))
}

fn fresh_cache_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "hf-candle-{tag}-{}",
        foundation_compact::ids::new_scru128()
    ));
    std::fs::create_dir_all(&dir).expect("temp cache dir");
    dir
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent dir");
    }
    std::fs::write(path, b"").expect("marker file");
}

fn provider_with_cache(cache: &Path) -> HuggingFaceCandleProvider {
    let config = HuggingFaceCandleConfig::builder()
        .cache_dir(cache.to_path_buf())
        .build();
    HuggingFaceCandleProvider::new(config).expect("provider builds offline")
}

// ---------------------------------------------------------------------------
// Cache hit
// ---------------------------------------------------------------------------

#[test]
fn a_fully_populated_cache_dir_short_circuits_the_download() {
    let cache = fresh_cache_dir("hit");
    let dest = cache_subdir(&cache);
    touch(&dest.join("config.json"));
    touch(&dest.join("model.safetensors"));

    let provider = provider_with_cache(&cache);
    let result = provider.download_model(REPO);
    std::fs::remove_dir_all(&cache).ok();

    let path = result.expect("a populated cache must short-circuit, not attempt a download");
    assert_eq!(
        path, dest,
        "the cached directory itself must be returned"
    );
}

#[test]
fn a_sharded_cache_dir_also_counts_as_populated() {
    // Larger models ship as model-00001-of-00002.safetensors etc. The check is
    // "any .safetensors", so sharded caches must hit too — otherwise every load
    // of a big model re-downloads it.
    let cache = fresh_cache_dir("sharded");
    let dest = cache_subdir(&cache);
    touch(&dest.join("config.json"));
    touch(&dest.join("model-00001-of-00002.safetensors"));
    touch(&dest.join("model-00002-of-00002.safetensors"));

    let provider = provider_with_cache(&cache);
    let result = provider.download_model(REPO);
    std::fs::remove_dir_all(&cache).ok();

    assert!(
        result.is_ok(),
        "a sharded cache must short-circuit like a single-file one"
    );
}

// ---------------------------------------------------------------------------
// Cache misses — each required marker independently
// ---------------------------------------------------------------------------
//
// These assert the check is not too EAGER. They attempt a real download and so
// will fail without network access; what matters is that they do not return the
// half-populated directory as a hit. Both outcomes are accepted except the one
// that would be a bug: silently succeeding with an incomplete cache.

/// Did `download_model` wrongly treat `dest` as cached? A hit returns instantly
/// with the dir; a miss either downloads or errors out.
fn wrongly_reported_as_cached(cache: &Path, dest: &Path) -> bool {
    let provider = provider_with_cache(cache);
    match provider.download_model(REPO) {
        // Returning the dir is only correct if it is genuinely complete.
        Ok(p) => p == dest && !(dest.join("config.json").exists() && has_any_safetensors(dest)),
        Err(_) => false,
    }
}

fn has_any_safetensors(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(std::result::Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "safetensors"))
    })
}

#[valtron_test]
fn a_dir_with_config_but_no_weights_is_not_a_cache_hit() {
    // The dangerous case: config.json present, weights missing. Treating this as
    // cached hands the loader a directory with no tensors.
    let cache = fresh_cache_dir("noweights");
    let dest = cache_subdir(&cache);
    touch(&dest.join("config.json"));

    let wrong = wrongly_reported_as_cached(&cache, &dest);
    std::fs::remove_dir_all(&cache).ok();
    assert!(
        !wrong,
        "config.json alone must not count as cached — the weights are missing"
    );
}

#[valtron_test]
fn a_dir_with_weights_but_no_config_is_not_a_cache_hit() {
    let cache = fresh_cache_dir("noconfig");
    let dest = cache_subdir(&cache);
    touch(&dest.join("model.safetensors"));

    let wrong = wrongly_reported_as_cached(&cache, &dest);
    std::fs::remove_dir_all(&cache).ok();
    assert!(
        !wrong,
        "weights alone must not count as cached — config.json is required to load"
    );
}

#[valtron_test]
fn an_unrelated_file_does_not_make_a_dir_look_cached() {
    // A stray download artefact (e.g. a .bin or a partial file) must not be
    // mistaken for safetensors.
    let cache = fresh_cache_dir("stray");
    let dest = cache_subdir(&cache);
    touch(&dest.join("config.json"));
    touch(&dest.join("pytorch_model.bin"));

    let wrong = wrongly_reported_as_cached(&cache, &dest);
    std::fs::remove_dir_all(&cache).ok();
    assert!(
        !wrong,
        "a .bin is not a .safetensors and must not satisfy the cache check"
    );
}

// ---------------------------------------------------------------------------
// Path mapping
// ---------------------------------------------------------------------------

#[test]
fn the_cache_path_flattens_the_org_separator() {
    // `org/model` becomes `org--model` so the cache stays one level deep. If the
    // mapping changed, every previously-cached model would miss and re-download.
    let cache = fresh_cache_dir("naming");
    let dest = cache_subdir(&cache);
    touch(&dest.join("config.json"));
    touch(&dest.join("model.safetensors"));

    let provider = provider_with_cache(&cache);
    let result = provider.download_model(REPO);
    std::fs::remove_dir_all(&cache).ok();

    let path = result.expect("cached");
    assert!(
        path.file_name().is_some_and(|n| n == "some-org--some-model"),
        "expected the flattened name, got {path:?}"
    );
}
