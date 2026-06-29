# RCA: SmolLM2 GGUF download fails on redirect to the Xet CDN

- **Date:** 2026-06-29
- **Symptom:** `foundation_ai` integration test
  `providers::integrations::llamacpp_provider::test_download_smollm_model`
  failed. `TestHarness::get_smollm_model()` produced a **0-byte**
  `SmolLM2-360M-Instruct-Q2_K.gguf`, so the test panicked at
  `assert!(file_stats.size() > 0)`.
- **Scope:** Any `repository::repo_download_file` call against a HuggingFace
  repo whose file is served via the Xet CDN (`cas-bridge.xethub.hf.co`), i.e.
  any download that follows a `302` redirect. The bug lived in
  `foundation_netio`'s redirect handling, so it affected every redirected GET,
  not just HuggingFace.

## What actually happened

1. `GET https://huggingface.co/.../resolve/main/SmolLM2-360M-Instruct-Q2_K.gguf`
   returns `302 Found` with `Location: https://cas-bridge.xethub.hf.co/...`.
2. The redirect follow-up request was built by
   `simple_http::client::shared::redirects::build_followup_request_from_request_descriptor`.
   That function had two defects:
   - **It unconditionally added `Expect: 100-continue`** to the follow-up
     request (`headers.insert(SimpleHeader::EXPECT, vec!["100-continue".into()])`),
     even though the follow-up is a **GET with no body**.
   - **It never rewrote the `Host` header.** It cloned the original request's
     headers, so the follow-up to `cas-bridge.xethub.hf.co` still carried
     `Host: huggingface.co`.
3. Because the GET to the CDN now carried `Expect: 100-continue`, the CDN
   replied with an interim `HTTP/1.1 100 Continue`.
4. In `tasks/request_redirect.rs`, the **no-body** code path read the response
   intro, got `100 Continue`, treated it as the *final* response (it is not a
   3xx redirect), and handed it back to the caller. The real `200/206` response
   carrying the GGUF bytes was never read.
5. `repo_download_file` had already done `File::create(&destination)` before
   reading the body, so the empty response left a **truncated 0-byte file** on
   disk. On the next run, `TestHarness` saw that file, assumed the model was
   cached, and returned it immediately — making the failure sticky.

The combination of (a) a spurious `Expect` header on a bodyless GET and (b) the
no-body reader treating the interim `100 Continue` as the final response is the
root cause. The stale `Host` header and the non-atomic file write were
contributing/aggravating defects.

## Fix

`foundation_netio` (the real bug):

- `client/shared/redirects.rs`
  - `build_followup_request_from_request_descriptor` and
    `build_followup_request_from` no longer add `Expect: 100-continue`; they now
    **remove** any inherited `Expect` header (follow-ups are GET/no-body).
  - Both now rewrite the `Host` header to the redirect target via a new
    `set_host_header` helper (includes the port only when non-default, matching
    how the Host header is first built in `request.rs`).
- `client/native/tasks/request_redirect.rs`
  - The `Expect`/100-continue handshake is now gated on a new config flag and
    only applied to requests that actually have a body; any stale `Expect`
    header is stripped otherwise.
  - Added a new branch for "has body but handshake disabled": write the body
    immediately instead of waiting for an interim response.
  - The no-body path now defensively **skips** any unsolicited interim
    `100 Continue` before reading the final response.
- `client/shared/config.rs` + `client/native/client.rs`
  - New `ClientConfig::expect_continue_enabled` (default `true`) with
    `ClientConfig::with_expect_continue(bool)` and
    `SimpleHttpClient::expect_continue(bool)` builder methods, so callers can
    disable the `Expect: 100-continue` flow per-client for endpoints that handle
    it poorly — without affecting other services.

`foundation_deployment_huggingface`:

- `repository::repo_download_file` now streams to a temporary `<file>.part` and
  `rename`s it into place only after a successful download, removing the temp
  file on error. This stops a partial/failed download from poisoning the cache
  with a 0-byte file. Also removed `println!` token-length leaks (now `tracing`).
- `tests/huggingface_integration.rs` referenced the old crate path
  `foundation_deployment::providers::huggingface` and used `tracing_test`
  without declaring it. Fixed the import to `foundation_deployment_huggingface`,
  added the `tracing-test` dev-dependency, and replaced the `eprintln!` that
  printed the first 10 characters of the token with a `tracing` call that logs
  only the token length.

## Verification

- `cargo test -p foundation_ai --features testing --features integration_tests -- providers::integrations::llamacpp_provider::test_download_smollm_model`
  now downloads the ~150 MB GGUF and passes (≈43s).
- `cargo test -p foundation_netio --lib redirects` passes, including two new
  tests asserting the follow-up strips `Expect` and rewrites `Host`.
- `cargo test -p foundation_deployment_huggingface --no-run` compiles
  (previously failed to compile).

## Note on the `Expect: 100-continue` toggle

`Expect: 100-continue` is only ever useful before sending a **large request
body** (it lets the server reject early, e.g. on auth/size, before the body is
streamed). It must never be sent on a bodyless GET. The new
`expect_continue` toggle lets a client turn the handshake off entirely; the
default remains on so legitimate large-upload negotiation still works.
