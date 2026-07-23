//! Fault injection against the `HuggingFace` download paths of both HF providers.
//!
//! WHY: `download_model` on the GGUF and Candle providers is the code that
//! decides *which* file to fetch, what to do when the repository does not hold
//! it, and how to assemble a sharded checkpoint. Until now the only tests that
//! ran it were the live ones against the real Hub, so every failure branch
//! (repository listing errors, missing quantizations, a corrupt shard index,
//! a 404 on `config.json`) was only ever exercised by a bad day in production.
//!
//! WHAT: stands up a `TestHttpServer` that speaks the two Hub routes these
//! providers use — `GET /api/models/{repo}/tree/{rev}` for listings and
//! `GET /{repo}/resolve/{rev}/{file}` for downloads — and points the provider at
//! it, then asserts the provider picks the right file, writes it to the cache
//! dir, and reports the right error when the Hub does not cooperate.
//!
//! HOW: `HuggingFaceGGUFConfig::endpoint` / `HuggingFaceCandleConfig::endpoint`
//! aim the provider's `HFClient` at the test server, so no `HF_ENDPOINT`
//! environment variable (which would be process-global and race the rest of the
//! suite) and no Hub credentials are involved. Each test gets its own temp cache
//! directory so a cache hit from a neighbouring test cannot mask a download.

use std::path::{Path, PathBuf};
use std::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};

use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_core::valtron::valtron_test;
use foundation_testing::http::{HttpResponse, TestHttpServer};

// ---------------------------------------------------------------------------
// Fake Hub
// ---------------------------------------------------------------------------

/// A unique temp cache directory per test, so no test can be served by another
/// test's download.
fn temp_cache_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ewe-hf-fault-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp cache dir");
    dir
}

fn json_ok(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
    }
}

fn bytes_ok(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            (
                "Content-Type".to_string(),
                "application/octet-stream".to_string(),
            ),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.to_vec(),
    }
}

fn status_only(status: u16, text: &str, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        status_text: text.to_string(),
        headers: vec![
            ("Content-Type".to_string(), "text/plain".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
        ],
        body: body.as_bytes().to_vec(),
    }
}

/// Start a fake Hub that honours keep-alive.
///
/// `TestHttpServer` defaults to a non-blocking read, so after answering one
/// request it finds no second request already buffered, gets `WouldBlock` and
/// drops the connection. The HF client pools connections between downloads —
/// and a Candle model needs three or more (config, tokenizer, weights) — so
/// against the default server every download after the first fails with "No
/// response intro received" on a connection the server had already closed.
/// A blocking read with a short timeout makes the connection genuinely
/// reusable, which is what the client expects of a real Hub.
fn hub<F>(handler: F) -> TestHttpServer
where
    F: Fn(&foundation_testing::http::HttpRequest) -> HttpResponse + Send + 'static,
{
    TestHttpServer::with_response(handler).blocking_read(Some(Duration::from_millis(500)))
}

/// One `RepoTreeEntry::File` as the Hub serialises it.
fn tree_file(path: &str, size: u64) -> String {
    format!(r#"{{"type":"file","oid":"deadbeef","size":{size},"path":"{path}","lfs":null}}"#)
}

fn tree_json(files: &[(&str, u64)]) -> String {
    let entries: Vec<String> = files.iter().map(|(p, s)| tree_file(p, *s)).collect();
    format!("[{}]", entries.join(","))
}

/// True for the repository-listing route.
fn is_tree(path: &str) -> bool {
    path.contains("/api/models/") && path.contains("/tree/")
}

/// The filename a `resolve` download is asking for, if this is a download route.
fn resolve_filename(path: &str) -> Option<&str> {
    path.split("/resolve/")
        .nth(1)
        .and_then(|rest| rest.split_once('/'))
        .map(|(_revision, filename)| filename)
}

fn gguf_provider(server: &TestHttpServer, cache_dir: &Path) -> HuggingFaceGGUFProvider {
    let config = HuggingFaceGGUFConfig::builder()
        .endpoint(server.base_url())
        .cache_dir(cache_dir)
        .build();
    HuggingFaceGGUFProvider::new(config).expect("provider builds")
}

fn candle_provider(server: &TestHttpServer, cache_dir: &Path) -> HuggingFaceCandleProvider {
    let config = HuggingFaceCandleConfig::builder()
        .endpoint(server.base_url())
        .cache_dir(cache_dir)
        .build();
    HuggingFaceCandleProvider::new(config).expect("provider builds")
}

// ---------------------------------------------------------------------------
// GGUF — file selection
// ---------------------------------------------------------------------------

#[valtron_test]
fn gguf_download_picks_the_requested_quantization() {
    // Three quantizations on offer; the caller asked for Q4_K_M and must get
    // exactly that one — picking a neighbour silently changes model quality.
    let cache_dir = temp_cache_dir("gguf-exact");
    let server = hub(move |req| {
        let path = req.path.url.clone();
        if is_tree(&path) {
            return json_ok(&tree_json(&[
                ("model-Q2_K.gguf", 10),
                ("model-Q4_K_M.gguf", 20),
                ("model-Q8_0.gguf", 30),
            ]));
        }
        match resolve_filename(&path) {
            Some(name) => bytes_ok(format!("weights of {name}").as_bytes()),
            None => status_only(404, "Not Found", "unexpected route"),
        }
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let path = provider.download_model(&parsed).expect("download succeeds");

    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("model-Q4_K_M.gguf"),
        "the requested quantization must be the one fetched, got {path:?}"
    );
    let written = std::fs::read_to_string(&path).expect("file landed on disk");
    assert_eq!(
        written, "weights of model-Q4_K_M.gguf",
        "the downloaded bytes must be written through to the cache file"
    );
}

#[valtron_test]
fn gguf_download_falls_back_when_the_quantization_is_absent() {
    // The repo has no Q4_K_M. Falling back to the one GGUF present is better
    // than failing, but it is a substitution the caller did not ask for — the
    // provider logs a warning and this test pins the behaviour.
    let cache_dir = temp_cache_dir("gguf-fallback");
    let server = hub(move |req| {
        let path = req.path.url.clone();
        if is_tree(&path) {
            return json_ok(&tree_json(&[("README.md", 1), ("model-Q8_0.gguf", 30)]));
        }
        match resolve_filename(&path) {
            Some(name) => bytes_ok(format!("weights of {name}").as_bytes()),
            None => status_only(404, "Not Found", "unexpected route"),
        }
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let path = provider.download_model(&parsed).expect("download succeeds");
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("model-Q8_0.gguf"),
        "with no exact match the only GGUF present must be used: {path:?}"
    );
}

#[valtron_test]
fn gguf_download_errors_when_the_repo_holds_no_gguf() {
    // A repo with no GGUF at all is a caller mistake (wrong repo id, or a
    // safetensors-only repo). It must be an error, not an empty success.
    let cache_dir = temp_cache_dir("gguf-none");
    let server = hub(move |req| {
        if is_tree(&req.path.url) {
            return json_ok(&tree_json(&[
                ("README.md", 1),
                ("model.safetensors", 100),
            ]));
        }
        status_only(404, "Not Found", "no such file")
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let err = provider
        .download_model(&parsed)
        .expect_err("a repo with no GGUF must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("q4_k_m") || msg.to_lowercase().contains("gguf"),
        "the error must name what was looked for: {msg}"
    );
}

#[valtron_test]
fn gguf_download_surfaces_a_listing_failure() {
    // A 500 from the listing route means we do not know what the repo holds.
    // Guessing a filename from that state would produce a confusing 404 later.
    let cache_dir = temp_cache_dir("gguf-tree-500");
    let server = hub(move |_req| {
        status_only(500, "Internal Server Error", "hub is unwell")
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let err = provider
        .download_model(&parsed)
        .expect_err("a failed listing must not be swallowed");
    let msg = err.to_string();
    assert!(
        msg.contains("list repository files"),
        "a Hub failure must be reported as a listing failure, not as a missing \
         quantization — the latter sends the caller after the wrong problem: {msg}"
    );
}

#[valtron_test]
fn gguf_download_surfaces_a_download_failure() {
    // The listing succeeds and names a file, but fetching it 404s (revision
    // moved, file gated). The cache must not be left holding a partial file.
    let cache_dir = temp_cache_dir("gguf-get-404");
    let server = hub(move |req| {
        if is_tree(&req.path.url) {
            return json_ok(&tree_json(&[("model-Q4_K_M.gguf", 20)]));
        }
        status_only(404, "Not Found", "gone")
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let err = provider
        .download_model(&parsed)
        .expect_err("a 404 on the file itself must fail the download");
    assert!(
        err.to_string().to_lowercase().contains("download"),
        "the error must say the download failed: {err}"
    );

    let leftover = cache_dir.join("acme--models").join("model-Q4_K_M.gguf");
    assert!(
        !leftover.exists(),
        "a failed download must not leave a file that later looks cached: {leftover:?}"
    );
}

#[valtron_test]
fn gguf_download_reuses_the_cached_file_without_calling_the_hub() {
    // The cache hit must short-circuit before any request: a provider that
    // re-downloads a cached 4 GB checkpoint is unusable offline.
    let cache_dir = temp_cache_dir("gguf-cached");
    let repo_dir = cache_dir.join("acme--models");
    std::fs::create_dir_all(&repo_dir).expect("repo dir");
    std::fs::write(repo_dir.join("model-Q4_K_M.gguf"), b"already here").expect("seed cache");

    let server = hub(move |_req| {
        status_only(500, "Internal Server Error", "the hub must not be reached")
    });

    let provider = gguf_provider(&server, &cache_dir);
    let parsed = provider
        .parse_model_id(&ModelId::Name("acme/models:q4_k_m".to_string(), None))
        .expect("model id parses");

    let path = provider
        .download_model(&parsed)
        .expect("a cached model resolves without the hub");
    assert_eq!(
        std::fs::read_to_string(&path).expect("cached file readable"),
        "already here"
    );
}

// ---------------------------------------------------------------------------
// Candle — safetensors assembly
// ---------------------------------------------------------------------------

#[valtron_test]
fn candle_download_fetches_a_single_file_checkpoint() {
    // The common case: config.json + tokenizer.json + one model.safetensors.
    let cache_dir = temp_cache_dir("candle-single");
    let server = hub(move |req| {
        match resolve_filename(&req.path.url) {
            Some("config.json") => json_ok(r#"{"architectures":["LlamaForCausalLM"]}"#),
            Some("tokenizer.json") => json_ok(r#"{"version":"1.0"}"#),
            Some("model.safetensors") => bytes_ok(b"single-file weights"),
            _ => status_only(404, "Not Found", "no such file"),
        }
    });

    let provider = candle_provider(&server, &cache_dir);
    let dir = provider
        .download_model("acme/tiny")
        .expect("download succeeds");

    assert!(dir.join("config.json").exists(), "config.json must land");
    assert!(
        dir.join("tokenizer.json").exists(),
        "tokenizer.json must land"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("model.safetensors")).expect("weights readable"),
        "single-file weights"
    );
}

#[valtron_test]
fn candle_download_assembles_a_sharded_checkpoint() {
    // No single model.safetensors: the provider must read the shard index and
    // pull every distinct shard it names. Missing one shard means a model that
    // fails to load much later with an opaque tensor error.
    let cache_dir = temp_cache_dir("candle-sharded");
    let index = r#"{"metadata":{"total_size":4},"weight_map":{
        "model.layers.0.weight":"model-00001-of-00002.safetensors",
        "model.layers.1.weight":"model-00001-of-00002.safetensors",
        "model.layers.2.weight":"model-00002-of-00002.safetensors"}}"#;
    let server = hub(move |req| {
        match resolve_filename(&req.path.url) {
            Some("config.json") => json_ok(r#"{"architectures":["LlamaForCausalLM"]}"#),
            Some("tokenizer.json") => json_ok(r#"{"version":"1.0"}"#),
            Some("model.safetensors") => status_only(404, "Not Found", "sharded repo"),
            Some("model.safetensors.index.json") => json_ok(index),
            Some(name) if name.ends_with(".safetensors") => {
                bytes_ok(format!("shard {name}").as_bytes())
            }
            _ => status_only(404, "Not Found", "no such file"),
        }
    });

    let provider = candle_provider(&server, &cache_dir);
    let dir = provider
        .download_model("acme/sharded")
        .expect("sharded download succeeds");

    for shard in [
        "model-00001-of-00002.safetensors",
        "model-00002-of-00002.safetensors",
    ] {
        assert_eq!(
            std::fs::read_to_string(dir.join(shard)).unwrap_or_else(|e| panic!(
                "shard {shard} must be downloaded: {e}"
            )),
            format!("shard {shard}"),
            "each shard named by the index must be fetched"
        );
    }
}

#[valtron_test]
fn candle_download_rejects_a_corrupt_shard_index() {
    // A truncated index would otherwise parse as "no weight_map" and produce a
    // model directory with no weights in it at all.
    let cache_dir = temp_cache_dir("candle-bad-index");
    let server = hub(move |req| {
        match resolve_filename(&req.path.url) {
            Some("config.json") => json_ok(r#"{"architectures":["LlamaForCausalLM"]}"#),
            Some("tokenizer.json") => json_ok(r#"{"version":"1.0"}"#),
            Some("model.safetensors") => status_only(404, "Not Found", "sharded repo"),
            Some("model.safetensors.index.json") => json_ok(r#"{"weight_map": {"a":"#),
            _ => status_only(404, "Not Found", "no such file"),
        }
    });

    let provider = candle_provider(&server, &cache_dir);
    let err = provider
        .download_model("acme/broken")
        .expect_err("a corrupt index must fail the download");
    assert!(
        err.to_string().contains("parse index"),
        "the error must point at the index, not at the weights: {err}"
    );
}

#[valtron_test]
fn candle_download_errors_when_no_weights_exist() {
    // Neither a single file nor an index: there is nothing to load.
    let cache_dir = temp_cache_dir("candle-no-weights");
    let server = hub(move |req| {
        match resolve_filename(&req.path.url) {
            Some("config.json") => json_ok(r#"{"architectures":["LlamaForCausalLM"]}"#),
            Some("tokenizer.json") => json_ok(r#"{"version":"1.0"}"#),
            _ => status_only(404, "Not Found", "no such file"),
        }
    });

    let provider = candle_provider(&server, &cache_dir);
    let err = provider
        .download_model("acme/empty")
        .expect_err("a repo with no safetensors must fail");
    assert!(
        err.to_string().contains("No model.safetensors or index"),
        "the error must say which files were looked for: {err}"
    );
}

#[valtron_test]
fn candle_download_errors_when_config_is_missing() {
    // Without config.json the architecture is unknown; failing here is far
    // clearer than failing inside the model loader after a multi-GB download.
    let cache_dir = temp_cache_dir("candle-no-config");
    let server =
        hub(move |_req| status_only(404, "Not Found", "no such file"));

    let provider = candle_provider(&server, &cache_dir);
    let err = provider
        .download_model("acme/no-config")
        .expect_err("a missing config.json must fail the download");
    assert!(
        err.to_string().contains("config.json"),
        "the error must name the missing file: {err}"
    );
}

#[valtron_test]
fn candle_download_reuses_a_cached_directory() {
    // Same contract as GGUF: a complete cache directory short-circuits before
    // any request reaches the hub.
    let cache_dir = temp_cache_dir("candle-cached");
    let repo_dir = cache_dir.join("acme--cached");
    std::fs::create_dir_all(&repo_dir).expect("repo dir");
    std::fs::write(repo_dir.join("config.json"), b"{}").expect("seed config");
    std::fs::write(repo_dir.join("model.safetensors"), b"cached").expect("seed weights");

    let server = hub(move |_req| {
        status_only(500, "Internal Server Error", "the hub must not be reached")
    });

    let provider = candle_provider(&server, &cache_dir);
    let dir = provider
        .download_model("acme/cached")
        .expect("a cached model resolves without the hub");
    assert_eq!(dir, repo_dir);
}

#[valtron_test]
fn candle_list_model_files_returns_only_sorted_safetensors() {
    // The listing exists so a caller can see what a repo holds before pulling
    // it; returning READMEs or unsorted output makes that answer unusable.
    let cache_dir = temp_cache_dir("candle-list");
    let server = hub(move |req| {
        if is_tree(&req.path.url) {
            return json_ok(&tree_json(&[
                ("model-00002-of-00002.safetensors", 2),
                ("README.md", 1),
                ("model-00001-of-00002.safetensors", 1),
                ("model.safetensors.index.json", 1),
                ("tokenizer.json", 1),
            ]));
        }
        status_only(404, "Not Found", "no such file")
    });

    let provider = candle_provider(&server, &cache_dir);
    let files = provider
        .list_model_files("acme/sharded")
        .expect("listing succeeds");

    assert_eq!(
        files,
        vec![
            "model-00001-of-00002.safetensors".to_string(),
            "model-00002-of-00002.safetensors".to_string(),
            "model.safetensors.index.json".to_string(),
        ],
        "only safetensors artefacts, in sorted order"
    );
}

#[valtron_test]
fn candle_list_model_files_surfaces_a_listing_failure() {
    // An empty `Ok` here would read as "this repo holds no weights", which is a
    // different — and actionable in a different way — answer than "the Hub is
    // down".
    let cache_dir = temp_cache_dir("candle-list-500");
    let server = hub(move |_req| {
        status_only(500, "Internal Server Error", "hub is unwell")
    });

    let provider = candle_provider(&server, &cache_dir);
    let err = provider
        .list_model_files("acme/sharded")
        .expect_err("a 500 must not be reported as an empty repository");
    assert!(
        err.to_string().contains("list repository files"),
        "the failure must name the listing: {err}"
    );
}

// ---------------------------------------------------------------------------
// get_model routing
// ---------------------------------------------------------------------------

#[valtron_test]
fn candle_get_model_rejects_a_non_repo_model_id() {
    // `ModelId::Name("llama3")` has no owner: there is no repo to fetch. The
    // rejection happens before any download, so no server is needed.
    let cache_dir = temp_cache_dir("candle-bad-id");
    let server = hub(move |_req| status_only(404, "Not Found", "unused"));

    let provider = candle_provider(&server, &cache_dir);
    let err = provider
        .get_model(ModelId::Name("llama3".to_string(), None))
        .expect_err("a bare name is not a HuggingFace repo id");
    assert!(
        err.to_string().contains("owner/repo"),
        "the error must show the expected shape: {err}"
    );
}
