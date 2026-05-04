//! HTTP download of pre-built qcow2 images with progress bar and resume support.
//!
//! Downloads go to `dest.partial` first, atomically rename to `dest` on
//! completion. On resume, `.partial` file is detected and HTTP `Range` is used.
//! Server not supporting 206 Partial Content triggers a fresh download.

use std::path::Path;
use std::process::Stdio;

use indicatif::{ProgressBar, ProgressStyle};

use crate::config::{Result, TestbedError};

/// Download a file from `url` to `dest`, showing a progress bar.
///
/// Uses curl as a subprocess. Downloads to `dest.partial` first, then
/// atomically renames to `dest` on success. If a `.partial` file already
/// exists, resume is attempted via HTTP Range. If the server doesn't support
/// 206 Partial Content, the `.partial` is deleted and a fresh download starts.
pub fn download(url: &str, dest: &Path) -> Result<()> {
    let partial = dest.with_extension(dest.extension().map_or_else(
        || "partial".to_string(),
        |ext| format!("{}.partial", ext.to_string_lossy()),
    ));

    // If the final dest already exists and is valid, skip download
    if is_cached(dest) {
        return Ok(());
    }

    // Check if we have a partial file to resume from
    let start_byte = if partial.exists() {
        let metadata = std::fs::metadata(&partial).map_err(|e| TestbedError::DownloadFailed {
            status: 0,
            url: format!("stat {partial:?}: {e}"),
        })?;
        Some(metadata.len())
    } else {
        None
    };

    // Build curl command targeting the partial file
    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "-L", // follow redirects
        "--fail", // exit non-zero on HTTP error
        "--create-dirs",
        "-o",
        partial.to_str().unwrap(),
    ]);

    if let Some(offset) = start_byte
        && offset > 0 {
            // Resume: use Range header
            cmd.args(["-C", "-"]);
        }

    cmd.args(["-s", "--show-error", url]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("curl not found: {e}"),
    })?;

    drop(child.stdout.take());
    drop(child.stderr.take());

    let status = child.wait().map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("curl wait error: {e}"),
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(0);
        // If resume failed (likely server doesn't support 206), delete partial and retry
        if start_byte.is_some() && start_byte.unwrap() > 0 {
            let _ = std::fs::remove_file(&partial);
            return download(url, dest); // retry fresh
        }
        return Err(TestbedError::DownloadFailed {
            status: code as u16,
            url: url.to_string(),
        });
    }

    // Validate download size — reject files < 100 MB as likely failed downloads
    let meta = std::fs::metadata(&partial).map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("stat {partial:?}: {e}"),
    })?;
    if meta.len() < 100 * 1024 * 1024 {
        let _ = std::fs::remove_file(&partial);
        return Err(TestbedError::DownloadFailed {
            status: 0,
            url: format!(
                "downloaded file is {} bytes (< 100 MB threshold), likely corrupted or truncated",
                meta.len()
            ),
        });
    }

    // Atomic rename: partial → dest
    std::fs::rename(&partial, dest).map_err(|e| TestbedError::DownloadFailed {
        status: 0,
        url: format!("rename {partial:?} → {dest:?}: {e}"),
    })?;

    // Show completion
    let pb = ProgressBar::new(meta.len());
    pb.set_position(meta.len());
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
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
        std::fs::File::create(path).unwrap();
        assert!(!is_cached(path));
        let _ = std::fs::remove_file(path);
    }
}
