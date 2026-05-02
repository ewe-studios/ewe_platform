//! HTTP download of pre-built qcow2 images with progress bar and resume support.
//!
//! Downloads are stored in the image cache directory. If a partial download
//! exists, it resumes from where it left off using HTTP Range requests.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;

use crate::config::{Result, TestbedError};

/// Download a file from `url` to `dest`, showing a progress bar.
///
/// If `dest` already exists and is non-empty, the download resumes from
/// the current file size (HTTP Range request). If the server doesn't
/// support Range, the download restarts from scratch.
pub fn download(url: &str, dest: &Path) -> Result<()> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("client build error: {e}"),
        })?;

    // Check existing file size for resume
    let start_byte = if dest.exists() {
        let metadata = std::fs::metadata(dest).map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("stat {dest:?}: {e}"),
        })?;
        metadata.len()
    } else {
        0
    };

    // Build request (with Range header if resuming)
    let mut request = client.get(url);
    if start_byte > 0 {
        request = request.header("Range", format!("bytes={start_byte}-"));
    }

    let response = request.send().map_err(|_e| TestbedError::DownloadFailed {
        status: 0,
        url: url.to_string(),
    })?;

    let status = response.status().as_u16();

    // Handle 206 Partial Content (resume) vs 200 OK (fresh/restart)
    let (actual_start, total_size) = if status == 206 {
        // Server supports Range; parse Content-Range header
        let content_range = response
            .headers()
            .get("Content-Range")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let total = parse_content_range_total(content_range).unwrap_or(0);
        (start_byte, total)
    } else if status == 200 {
        // Fresh download (or server doesn't support Range)
        // If we had a partial file, truncate it
        let total = response.content_length().unwrap_or(0);
        if start_byte > 0 {
            // Restart: truncate existing partial
            File::create(dest).map_err(|e| TestbedError::DownloadFailed {
                status: 0,
                url: format!("truncate {dest:?}: {e}"),
            })?;
        }
        (0, total)
    } else {
        return Err(TestbedError::DownloadFailed { status, url: url.to_string() });
    };

    // Setup progress bar
    let pb = if total_size > 0 {
        let bar = ProgressBar::new(total_size);
        bar.set_position(actual_start);
        bar.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));
        bar
    } else {
        let bar = ProgressBar::new(0);
        bar.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {bytes} ({bytes_per_sec})")
            .unwrap());
        bar
    };

    // Open file for appending (creates if not exists)
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(dest)
        .map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("open {dest:?}: {e}"),
        })?;

    // Stream response body to file
    let mut reader = response;
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("read response: {e}"),
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("write {dest:?}: {e}"),
        })?;
        pb.inc(n as u64);
    }

    pb.finish_with_message("Download complete");
    Ok(())
}

/// Check if a cached image exists and is valid (non-empty).
pub fn is_cached(dest: &Path) -> bool {
    dest.exists()
        && std::fs::metadata(dest)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

/// Parse the total size from a Content-Range header like "bytes 100-999/10000".
fn parse_content_range_total(header: &str) -> Option<u64> {
    // Format: "bytes start-end/total" or "bytes */total"
    let parts: Vec<&str> = header.split('/').collect();
    if parts.len() == 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_range_total() {
        assert_eq!(parse_content_range_total("bytes 100-999/10000"), Some(10000));
        assert_eq!(parse_content_range_total("bytes 0-499/500"), Some(500));
        assert_eq!(parse_content_range_total("bytes */5000"), Some(5000));
        assert_eq!(parse_content_range_total("invalid"), None);
        assert_eq!(parse_content_range_total(""), None);
    }

    #[test]
    fn test_is_cached_false_for_missing_or_empty() {
        // Non-existent path
        assert!(!is_cached(Path::new("/tmp/nonexistent_file_12345.qcow2")));

        // Empty file
        let path = Path::new("/tmp/test_empty_cache.qcow2");
        let _ = std::fs::remove_file(path);
        File::create(path).unwrap();
        assert!(!is_cached(path));
        let _ = std::fs::remove_file(path);
    }
}
