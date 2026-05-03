//! VM Export — export running/stopped VMs as qcow2 + manifest images.
//!
//! Supports local export, shrink/optimization, and upload to R2/S3/Local/HTTP stores.

mod manifest;
mod shrink;
mod store_upload;

pub use manifest::ExportManifest;
pub use shrink::{compress_qcow2, shrink_disk, size_comparison};
pub use store_upload::upload_to_store;

use std::path::{Path, PathBuf};

use crate::config::{self, get_profile, ImageStore, VmProfile, Result, TestbedError};
use crate::ssh;
use crate::state;

/// Options for exporting a VM.
#[derive(Debug, Default)]
pub struct ExportOptions {
    /// Local output path (default: `.build/export/<name>.qcow2`).
    pub out: Option<PathBuf>,
    /// Upload to a named image store.
    pub store: Option<String>,
    /// Version tag for the export.
    pub version: String,
    /// Include bootstrap marker (VM is pre-bootstrapped).
    pub include_bootstrap: bool,
    /// Clean up VM internal state before export.
    pub clean: bool,
    /// Zero-fill and compress qcow2 (slower, smaller file).
    pub shrink: bool,
    /// Human-readable notes for the export.
    pub notes: String,
}

/// Export a VM to a local file or upload to a store.
pub fn export_vm(name: &str, opts: &ExportOptions) -> Result<PathBuf> {
    let profile = get_profile(name).map(|p| p.clone()).map_err(|e| e.into())?;

    // Step 1: Determine output path
    let export_dir = opts
        .out
        .as_ref()
        .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
        .unwrap_or_else(|| {
            let dir = PathBuf::from(".build/export");
            let _ = std::fs::create_dir_all(&dir);
            dir
        });

    let output_name = opts
        .out
        .clone()
        .unwrap_or_else(|| export_dir.join(format!("{}-{}.qcow2", name, opts.version)));

    // Ensure output directory exists
    if let Some(parent) = output_name.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TestbedError::Qcow2Error {
            message: format!("creating export directory: {e}"),
        })?;
    }

    // Step 2: Check if VM is running
    let vm_state = state::load(name).ok();
    let is_running = vm_state.as_ref().and_then(|s| s.pid).is_some();

    println!("Exporting VM '{}' ({})...", profile.name, if is_running { "running" } else { "stopped" });

    // Step 3: If shrink requested and VM is running, zero-fill before stopping
    if opts.shrink && is_running {
        println!("  Shrinking disk (fstrim + zero-fill)...");
        let mut session = ssh::connect(&profile)?;
        shrink_disk(&mut session, profile.os)?;
    }

    // Step 4: Stop VM if running
    let was_running = is_running;
    if is_running {
        println!("  Stopping VM...");
        stop_vm(&profile, &vm_state)?;
    }

    // Step 5: Copy disk image to export location
    let disk_path = profile.image_cache_path();
    println!("  Copying disk image -> {}", output_name.display());
    std::fs::copy(&disk_path, &output_name).map_err(|e| TestbedError::Qcow2Error {
        message: format!("copying disk image: {e}"),
    })?;

    // Step 6: If shrink requested, compress the copy
    if opts.shrink {
        let compressed_path = output_name.with_extension("qcow2.compressed");
        println!("  Compressing qcow2...");
        compress_qcow2(&output_name, &compressed_path)?;

        let (orig, comp, ratio) = size_comparison(&output_name, &compressed_path)?;
        println!("  Size: {:.2} GB -> {:.2} GB ({:.1}%)",
            orig as f64 / 1_073_741_824.0,
            comp as f64 / 1_073_741_824.0,
            ratio);

        std::fs::rename(&compressed_path, &output_name).map_err(|e| TestbedError::Qcow2Error {
            message: format!("replacing with compressed: {e}"),
        })?;
    }

    // Step 7: Detect installed tools if VM was running
    let installed_tools = if was_running {
        let mut session = ssh::connect(&profile).ok();
        session.as_mut().and_then(|s| detect_installed_tools(s, profile.os).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let bootstrap_version = if opts.include_bootstrap { 3 } else { 0 };

    // Step 8: Generate and write manifest
    let manifest = ExportManifest::new(
        &profile,
        &output_name,
        &opts.version,
        installed_tools,
        bootstrap_version,
        &opts.notes,
    )?;
    manifest.write_to(&output_name)?;
    println!("  Manifest written: {}", output_name.with_extension("qcow2.manifest.json").display());

    // Step 9: Upload to store if specified
    if let Some(ref store_name) = opts.store {
        let store = resolve_store(store_name)?;
        let manifest_path = output_name.with_extension("qcow2.manifest.json");
        println!("  Uploading to store '{}'...", store_name);
        upload_to_store(&store, &output_name, &manifest_path)?;
    }

    // Print import instructions
    println!("\nExport complete: {}", output_name.display());
    if let Some(ref store_name) = opts.store {
        println!("  Also uploaded to store: {store_name}");
        println!("  Import: add [[image_stores]] entry for '{store_name}' in testbed.toml, then:");
        println!("    ewe_platform testbed import {name}");
    } else {
        println!("  Import: ewe_platform testbed adopt {name} --disk {}", output_name.display());
    }

    Ok(output_name)
}

fn stop_vm(profile: &VmProfile, vm_state: &Option<state::VmState>) -> Result<()> {
    let Some(s) = vm_state else {
        return Err(TestbedError::VmNotRunning { name: profile.name.to_string() });
    };

    // Try graceful shutdown via monitor socket
    if s.monitor_socket.is_empty() {
        if let Some(pid) = s.pid {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        return Ok(());
    }

    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    if let Ok(mut stream) = UnixStream::connect(&s.monitor_socket) {
        let _ = stream.write_all(b"system_powerdown\n");
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
    }

    // Wait briefly for process to exit
    if let Some(pid) = s.pid {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if unsafe { libc::kill(pid as i32, 0) } != 0 {
                return Ok(()); // Process exited
            }
        }
        // Force kill after timeout
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }

    // Clean up state
    let _ = state::delete(&profile.name);

    Ok(())
}

fn resolve_store(name: &str) -> Result<ImageStore> {
    config::get_image_store(name).ok_or_else(|| TestbedError::Qcow2Error {
        message: format!("image store '{name}' not found in testbed.toml"),
    })
}

/// Detect installed tools via SSH.
fn detect_installed_tools(
    session: &mut ssh::VmSession,
    os: crate::config::GuestOs,
) -> Result<std::collections::HashMap<String, String>> {
    let mut tools = std::collections::HashMap::new();

    match os {
        crate::config::GuestOs::Linux => {
            // rustc
            if let Ok(out) = ssh::exec(session, "rustc --version 2>/dev/null || echo MISSING") {
                if let Some(ver) = out.trim().strip_prefix("rustc ") {
                    tools.insert("rust".to_string(), ver.trim().to_string());
                }
            }
            // node
            if let Ok(out) = ssh::exec(session, "node --version 2>/dev/null || echo MISSING") {
                if let Some(ver) = out.trim().strip_prefix('v') {
                    tools.insert("node".to_string(), ver.to_string());
                }
            }
            // nu
            if let Ok(out) = ssh::exec(session, "nu --version 2>/dev/null || echo MISSING") {
                if let Some(ver) = out.trim().split_whitespace().last() {
                    tools.insert("nu".to_string(), ver.to_string());
                }
            }
            // mise
            if let Ok(out) = ssh::exec(session, "mise --version 2>/dev/null || echo MISSING") {
                if let Some(ver) = out.trim().lines().next() {
                    tools.insert("mise".to_string(), ver.trim().to_string());
                }
            }
        }
        crate::config::GuestOs::Windows => {
            if let Ok((out, _)) = ssh::exec_ps_windows(session,
                "try { (rustc --version).Split(' ')[1] } catch { 'MISSING' }",
            ) {
                if out.trim() != "MISSING" {
                    tools.insert("rust".to_string(), out.trim().to_string());
                }
            }
            if let Ok((out, _)) = ssh::exec_ps_windows(session,
                "try { (node --version).TrimStart('v') } catch { 'MISSING' }",
            ) {
                if out.trim() != "MISSING" {
                    tools.insert("node".to_string(), out.trim().to_string());
                }
            }
        }
    }

    Ok(tools)
}
