//! Upload exported VM images to remote stores (R2, S3, Local, HTTP).

use std::path::Path;
use std::process::Command;

use crate::vms::config::{ImageStore, Result, StoreType, TestbedError};

/// Upload an exported image and its manifest to a store.
pub fn upload_to_store(store: &ImageStore, image_path: &Path, manifest_path: &Path) -> Result<()> {
    match store.store_type {
        StoreType::R2 => upload_rclone(store, image_path, manifest_path),
        StoreType::S3 => upload_aws(store, image_path, manifest_path),
        StoreType::Local => upload_local(store, image_path, manifest_path),
        StoreType::Http => upload_http(store, image_path, manifest_path),
    }
}

fn upload_rclone(store: &ImageStore, image_path: &Path, manifest_path: &Path) -> Result<()> {
    ensure_tool("rclone", "https://rclone.org/install/")?;

    let remote_path = format!("{}:{}", store.name, store_destination(store, image_path));

    let status = Command::new("rclone")
        .args(["copy", &image_path.to_string_lossy(), &remote_path])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning rclone: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("rclone upload failed (exit {status})"),
        });
    }

    // Upload manifest too
    let manifest_remote = format!("{}:{}", store.name, store_destination(store, manifest_path));
    Command::new("rclone")
        .args(["copy", &manifest_path.to_string_lossy(), &manifest_remote])
        .status()
        .ok(); // Best effort for manifest

    Ok(())
}

fn upload_aws(store: &ImageStore, image_path: &Path, manifest_path: &Path) -> Result<()> {
    ensure_tool("aws", "pip install awscli")?;

    let s3_path = format!("s3://{}/{}", store.destination, store_destination(store, image_path));

    let status = Command::new("aws")
        .args(["s3", "cp", &image_path.to_string_lossy(), &s3_path])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning aws CLI: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("aws upload failed (exit {status})"),
        });
    }

    // Upload manifest too
    let manifest_s3 = format!("s3://{}/{}", store.destination, store_destination(store, manifest_path));
    Command::new("aws")
        .args(["s3", "cp", &manifest_path.to_string_lossy(), &manifest_s3])
        .status()
        .ok();

    Ok(())
}

fn upload_local(store: &ImageStore, image_path: &Path, manifest_path: &Path) -> Result<()> {
    let dest_dir = std::path::PathBuf::from(&store.destination);
    std::fs::create_dir_all(&dest_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating local store dir: {e}"),
    })?;

    let dest_file = dest_dir.join(store_destination(store, image_path));
    std::fs::copy(image_path, &dest_file).map_err(|e| TestbedError::Qcow2Error {
        message: format!("copying to local store: {e}"),
    })?;

    let manifest_dest = dest_dir.join(store_destination(store, manifest_path));
    std::fs::copy(manifest_path, &manifest_dest).ok();

    Ok(())
}

fn upload_http(store: &ImageStore, image_path: &Path, _manifest_path: &Path) -> Result<()> {
    // Simple HTTP PUT — assumes the server accepts raw file uploads
    let url = format!("{}/{}", store.destination, store_destination(store, image_path));

    // Use curl for the PUT
    let status = Command::new("curl")
        .args(["-X", "PUT", "-T", &image_path.to_string_lossy(), &url])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("spawning curl: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("HTTP PUT failed (exit {status})"),
        });
    }

    Ok(())
}

fn store_destination(store: &ImageStore, path: &Path) -> String {
    let filename = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "export.qcow2".to_string());

    if let Some(ref prefix) = store.key_prefix {
        format!("{prefix}/{filename}")
    } else {
        filename
    }
}

fn ensure_tool(tool: &str, install_hint: &str) -> Result<()> {
    Command::new(tool)
        .arg("--version")
        .output()
        .map_err(|_| TestbedError::Qcow2Error {
            message: format!("{tool} not found on PATH — install via: {install_hint}"),
        })?;
    Ok(())
}
