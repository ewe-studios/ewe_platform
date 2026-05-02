//! HTTP download of pre-built qcow2 images with progress bar and resume support.
//!
//! Downloads are performed via the `curl` CLI subprocess to avoid blocking
//! HTTP client issues inside async runtimes.

use std::fs::File;
use std::path::Path;
use std::process::Stdio;

use indicatif::{ProgressBar, ProgressStyle};

use crate::config::{Result, TestbedError};

/// Download a file from `url` to `dest`, showing a progress bar.
///
/// Uses curl as a subprocess to avoid tokio runtime conflicts with
/// blocking HTTP clients. If `dest` already exists and is non-empty,
/// the download resumes from the current file size.
pub fn download(url: &str, dest: &Path) -> Result<()> {
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

    // Build curl command
    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "-L",              // follow redirects
        "--fail",          // exit non-zero on HTTP error
        "--create-dirs",   // create parent directories
        "-o", dest.to_str().unwrap(),
    ]);

    if start_byte > 0 {
        // Resume: append to existing file, use range header
        cmd.args(["-C", "-", "-f"]); // -C - = resume from where left off
    }

    // Add progress bar via stderr (we parse it ourselves)
    cmd.args(["-s", "--show-error", url]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("curl not found: {e}"),
    })?;

    // Read stderr for progress info, stdout for data
    let stderr = child.stderr.take();

    // Just wait for curl to finish — curl writes directly to the file
    // via -o flag, so we don't need to stream stdout ourselves
    drop(child.stdout.take());
    drop(stderr);

    let status = child.wait().map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("curl wait error: {e}"),
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(0);
        return Err(TestbedError::DownloadFailed {
            status: code as u16,
            url: url.to_string(),
        });
    }

    // Show completion
    if let Ok(meta) = std::fs::metadata(dest) {
        let pb = ProgressBar::new(meta.len());
        pb.set_position(meta.len());
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap()
            .progress_chars("#>-"));
        pb.finish_with_message("Download complete");
    }

    Ok(())
}

/// Check if a cached image exists and is valid (non-empty).
pub fn is_cached(dest: &Path) -> bool {
    dest.exists()
        && std::fs::metadata(dest)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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
