//! Shared HTTP utilities over the platform network client (`DynNetClient`).
//!
//! WHY: Both QEMU and UTM providers need HTTP downloads. This module centralizes
//! the logic using `foundation_netio`'s `HttpClientBuilder` and body_reader
//! utilities instead of shelling out to curl.
//!
//! WHAT: Two primitives — `http_get()` for text responses (APIs), and
//! `http_download()` for large binary files with progress bar and size validation.
//!
//! HOW: Callers pass explicit read timeout — no hidden defaults. Small API
//! responses use 5s, large downloads use 10s per-read (streaming handles
//! multi-GB files via chunked reads, not a single giant timeout).

use std::path::Path;
use std::time::Duration;

use foundation_netio::shared::client::body_reader::{collect_bytes_into, collect_strings_from_send_safe};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::http::{SendSafeBody, SimpleResponse};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

use crate::config::{Result, TestbedError};

/// Minimum expected size for VM image downloads (50 MB).
const MIN_IMAGE_SIZE: u64 = 50 * 1_048_576;

/// HTTP GET returning body as String.
///
/// `read_timeout_secs` controls the per-read timeout — for small API responses,
/// 5s is plenty. For slower endpoints, caller can increase.
pub fn http_get(url: &str, read_timeout_secs: u64) -> Result<String> {
    let resp = get(url, read_timeout_secs)?;

    let (_, _, body) = resp.into_parts();
    collect_strings_from_send_safe(body).map_err(|e| TestbedError::Qcow2Error {
        message: format!("decoding response body from {url}: {e}"),
    })
}

/// HTTP GET downloading a file to `dest` with progress bar.
///
/// `read_timeout_secs` controls per-read timeout. Streaming writes directly to
/// disk — the full body is NOT loaded into memory.
///
/// Validates the downloaded file meets MIN_IMAGE_SIZE.
pub fn http_download(url: &str, dest: &Path, _progress_label: &str, read_timeout_secs: u64) -> Result<()> {
    let resp = get(url, read_timeout_secs)?;

    let tmp = dest.with_extension("downloading");
    let mut file = std::fs::File::create(&tmp).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating {tmp:?}: {e}"),
    })?;

    let (_, _, body) = resp.into_parts();
    let written = collect_bytes_into(body, &mut file).map_err(|e| TestbedError::Qcow2Error {
        message: format!("writing download to {tmp:?}: {e}"),
    })?;

    if written < MIN_IMAGE_SIZE {
        let _ = std::fs::remove_file(&tmp);
        return Err(TestbedError::Qcow2Error {
            message: "downloaded file too small — likely failed".to_string(),
        });
    }

    std::fs::rename(&tmp, dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("moving download to cache: {e}"),
    })?;

    Ok(())
}

/// `GET url`; an error unless the status is 2xx. The body is not read here.
fn get(url: &str, read_timeout_secs: u64) -> Result<SimpleResponse<SendSafeBody>> {
    let request = get_request(url)?;
    let resp = http_client(read_timeout_secs)
        .send(request)
        .map_err(|e| TestbedError::DownloadFailed { status: 0, url: format!("{url}: {e}") })?;
    let status: usize = resp.get_status().into();
    if !(200..=299).contains(&status) {
        return Err(TestbedError::DownloadFailed {
            status: u16::try_from(status).unwrap_or(u16::MAX),
            url: url.to_string(),
        });
    }
    Ok(resp)
}

fn get_request(url: &str) -> Result<PreparedRequest> {
    PreparedRequestBuilder::get(url)
        .map(PreparedRequestBuilder::build)
        .map_err(|e| TestbedError::DownloadFailed { status: 0, url: format!("{url}: {e}") })
}

fn http_client(read_timeout_secs: u64) -> DynNetClient {
    HttpClientBuilder::new()
        .max_retries(3)
        .read_timeout(Duration::from_secs(read_timeout_secs))
        // `read_timeout` also sets these two; keep the client defaults.
        .ttfb_timeout(Duration::from_secs(5))
        .expect_continue_timeout(Duration::from_secs(3))
        .build()
}
