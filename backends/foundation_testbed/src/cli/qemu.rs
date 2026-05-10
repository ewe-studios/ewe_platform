//! QEMU-only CLI commands (snapshot, resize-disk, mount, adopt, refresh-network, export).
//! These require direct QEMU access (monitor socket, qcow2 manipulation).

use std::path::Path;

use clap::ArgMatches;
use crate::config::{get_profile, DisplayMode};

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<crate::config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
}

/// Attach to an already-running VM via its state file + monitor socket.
fn attach_running(profile: &crate::config::VmProfile) -> std::result::Result<crate::qemu::QemuVm, BoxedError> {
    let vm_state = crate::state::load(&profile.name).map_err(|e| Box::new(e) as BoxedError)?;
    let pid = vm_state.pid.ok_or_else(|| format!("VM '{}' is not running", profile.name))?;
    let monitor = std::path::PathBuf::from(&vm_state.monitor_socket);

    let resolved = crate::qemu::ResolvedPorts {
        ssh_port: vm_state.ssh_port,
        winrm_port: vm_state.winrm_port,
        rdp_port: vm_state.rdp_port,
        vnc_port: vm_state.vnc_port,
    };

    let vm = crate::qemu::QemuVm::adopt_running(
        profile.clone(),
        std::path::PathBuf::from(&vm_state.disk_path),
        pid,
        &monitor,
        resolved,
    )?;

    Ok(vm)
}

pub fn cmd_snapshot(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    match args.subcommand() {
        Some(("save", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            let profile = resolve_profile(name)?;
            let mut vm = attach_running(&profile)?;
            crate::qemu::snapshot::save(&mut vm, snap_name)?;
            println!("Snapshot '{}' saved for VM '{}'", snap_name, name);
        }
        Some(("load", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            let profile = resolve_profile(name)?;
            let mut vm = attach_running(&profile)?;
            crate::qemu::snapshot::load(&mut vm, snap_name)?;
            println!("Restored VM '{}' to snapshot '{}'", name, snap_name);
        }
        Some(("delete", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            let profile = resolve_profile(name)?;
            let mut vm = attach_running(&profile)?;
            crate::qemu::snapshot::delete(&mut vm, snap_name)?;
            println!("Snapshot '{}' deleted for VM '{}'", snap_name, name);
        }
        Some(("list", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let profile = resolve_profile(name)?;
            let mut vm = attach_running(&profile)?;
            let snapshots = crate::qemu::snapshot::list(&mut vm)?;
            if snapshots.is_empty() {
                println!("No snapshots found for VM '{}'", name);
            } else {
                println!("{:<8} {:<20} {:<12} {:<22}", "ID", "TAG", "SIZE", "DATE");
                println!("{}", "-".repeat(65));
                for snap in &snapshots {
                    let size = if snap.vm_clock_size > 1_073_741_824 {
                        format!("{:.1} GB", snap.vm_clock_size as f64 / 1_073_741_824.0)
                    } else {
                        format!("{:.0} MB", snap.vm_clock_size as f64 / 1_048_576.0)
                    };
                    println!("{:<8} {:<20} {:<12} {:<22}", snap.id, snap.tag, size, snap.date);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn cmd_resize_disk(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let plus_gb = *args.get_one::<u32>("plus_gb").unwrap();
    let profile = resolve_profile(name)?;

    let disk_path = profile.image_cache_path();
    crate::qemu::disk::resize(&disk_path, plus_gb)?;

    println!("Resized {} by +{}GB", disk_path.display(), plus_gb);
    Ok(())
}

pub fn cmd_mount(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    match args.subcommand() {
        Some(("status", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let profile = resolve_profile(name)?;

            let disk_path = profile.image_cache_path();
            let vm_state = crate::state::load(name).ok();

            println!("Mount configuration for VM '{}':", profile.name);
            println!("  OS: {:?}", profile.os);
            println!("  Disk: {}", disk_path.display());
            if let Some(ref s) = vm_state {
                println!("  PID: {:?}", s.pid);
                println!("  Running: {}", s.pid.is_some());
            } else {
                println!("  Running: no");
            }

            println!("  Host path (project): . (current directory)");
            println!("  Guest tag: {}", crate::qemu::mount::DEFAULT_TAG);
            println!("  Guest path: {}", crate::qemu::mount::DEFAULT_GUEST_PATH);
            println!("  Protocol: 9p/virtio");
        }
        Some(("verify", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let profile = resolve_profile(name)?;

            let mut session = crate::ssh::connect(&profile)?;
            let verify_cmd = crate::qemu::mount::verify_mount_command(
                crate::qemu::mount::DEFAULT_GUEST_PATH,
            );
            let output = crate::ssh::exec(&mut session, &verify_cmd)?;
            if output.contains("OK") {
                println!("Project mount is active in VM '{}'", profile.name);
            } else {
                println!("Project mount is NOT active in VM '{}'", profile.name);
                println!("  Ensure the VM was started with project mount enabled.");
            }
        }
        _ => {}
    }
    Ok(())
}

/// Wait for VM SSH to be ready, with timeout.
/// Returns the SSH session once connected.
fn wait_for_ssh_ready(
    profile: &crate::config::VmProfile,
    timeout_secs: u64,
) -> Result<crate::ssh::VmSession, BoxedError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    while std::time::Instant::now() < deadline {
        match crate::ssh::connect(profile) {
            Ok(session) => {
                return Ok(session);
            }
            Err(e) => {
                if e.to_string().contains("authentication") {
                    return Err(format!("SSH auth failed: {}", e).into());
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }

    Err("Timeout waiting for SSH".into())
}

/// Run mount verification and return results.
fn verify_mount(
    session: &mut crate::ssh::VmSession,
    mount_type: &str,
    guest_path: &str,
    profile: &crate::config::VmProfile,
) -> std::result::Result<(bool, String), BoxedError> {
    let mut success = false;
    let mut diagnosis = String::new();

    match mount_type {
        "virtiofs" => {
            diagnosis.push_str("  Checking virtiofs...\n");

            // Check if virtiofs.exe is available — FAST PATH first
            let check = crate::ssh::exec(session,
                "$p1='C:\\Program Files\\Virtio-Win\\VioFS\\virtiofs.exe'; $p2='C:\\Program Files (x86)\\Virtio-Win\\VioFS\\virtiofs.exe'; if (Test-Path $p1) { 'VIRTIOFS_OK' } elseif (Test-Path $p2) { 'VIRTIOFS_OK_X86' } else { 'VIRTIOFS_MISSING' }");
            match check {
                Ok(output) if output.contains("VIRTIOFS_OK") => {
                    diagnosis.push_str("  ✓ virtiofs.exe found\n");
                }
                _ => {
                    // FALLBACK: Check registry
                    let reg_check = crate::ssh::exec(session,
                        "$reg=Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Virtio-win-driver-installer' -ErrorAction SilentlyContinue; if ($reg -and $reg.InstallLocation -and (Test-Path (Join-Path $reg.InstallLocation 'VioFS\\virtiofs.exe'))) { 'VIRTIOFS_OK_REG' } else { 'VIRTIOFS_MISSING' }");
                    match reg_check {
                        Ok(output) if output.contains("VIRTIOFS_OK") => {
                            diagnosis.push_str("  ✓ virtiofs.exe found via registry\n");
                        }
                        _ => {
                            diagnosis.push_str("  ✗ virtiofs.exe not found\n");
                            diagnosis.push_str(&format!("    Fix: Run 'testbed bootstrap {}'\n", profile.name));
                        }
                    }
                }
            }

            // Check mount point
            let mount_check = crate::ssh::exec(session,
                &format!("if (Test-Path '{}') {{ 'MOUNT_OK' }} else {{ 'MOUNT_MISSING' }}", guest_path));
            match mount_check {
                Ok(output) if output.contains("MOUNT_OK") => {
                    // Try to read a file to verify it's working
                    let file_check = crate::ssh::exec(session,
                        &format!("Test-Path (Join-Path '{}' 'Cargo.toml')", guest_path));
                    match file_check {
                        Ok(_) => {
                            diagnosis.push_str(&format!("  ✓ Mount accessible: {}\n", guest_path));
                            success = true;
                        }
                        Err(_) => {
                            diagnosis.push_str(&format!("  ⚠ Mount exists but files not readable\n"));
                        }
                    }
                }
                _ => {
                    diagnosis.push_str(&format!("  ✗ Mount point missing: {}\n", guest_path));
                    diagnosis.push_str("    The scheduled task may not have run.\n");
                    diagnosis.push_str(&format!("    Check: testbed exec {} --method winrm 'Get-ScheduledTask -TaskName FoundationTestbed*'\n", profile.name));
                }
            }
        }
        "smb" => {
            diagnosis.push_str("  Checking SMB...\n");

            // Check if SMB is already mounted
            let smb_check = crate::ssh::exec(session, "if (Test-Path 'Z:\\') { 'SMB_OK' } else { 'SMB_MISSING' }");
            match smb_check {
                Ok(output) if output.contains("SMB_OK") => {
                    diagnosis.push_str("  ✓ SMB mounted at Z:\\\n");
                    success = true;
                }
                _ => {
                    diagnosis.push_str("  → SMB not mounted\n");
                    diagnosis.push_str("    To mount inside VM: net use Z: \\\\\\10.0.2.4\\\\qemu\n");
                    diagnosis.push_str(&format!("    Or: testbed exec {} --method winrm \"net use Z: \\\\\\\\\\10.0.2.4\\\\qemu\"\n", profile.name));
                }
            }
        }
        "9p" => {
            diagnosis.push_str("  Checking 9p mount...\n");
            let mount_check = crate::ssh::exec(session,
                &format!("mount | grep '{}' || echo 'NOT_MOUNTED'", guest_path));
            match mount_check {
                Ok(output) if output.contains("9p") => {
                    diagnosis.push_str(&format!("  ✓ 9p mount active at {}\n", guest_path));
                    success = true;
                }
                _ => {
                    diagnosis.push_str(&format!("  → 9p not mounted, attempting auto-mount...\n"));
                    let mount_cmd = format!("sudo mkdir -p {} && sudo mount -t 9p -o trans=virtio,version=9p2000.L project {}",
                        guest_path, guest_path);
                    match crate::ssh::exec(session, &mount_cmd) {
                        Ok(_) => {
                            diagnosis.push_str("  ✓ Auto-mount successful\n");
                            success = true;
                        }
                        Err(e) => {
                            diagnosis.push_str(&format!("  ✗ Auto-mount failed: {}\n", e));
                            diagnosis.push_str(&format!("    Try manual: testbed shell {}\n", profile.name));
                        }
                    }
                }
            }
        }
        "none" => {
            diagnosis.push_str("  No mount configured (--type=none)\n");
            success = true;
        }
        _ => {
            diagnosis.push_str(&format!("  Unknown mount type: {}\n", mount_type));
        }
    }

    Ok((success, diagnosis))
}

/// Start a VM with specific network/share configuration for testing.
///
/// Without --daemonize: Start VM, wait for ready, verify mount, keep running
/// With --daemonize: Start VM in background thread, wait for ready, verify mount,
///                   print diagnosis, stop VM
pub fn cmd_network(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    use crate::config::{DisplayMode, GuestOs};
    use crate::providers::default_provider;
    use crate::state;
    use std::sync::mpsc;

    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    let mount_type = args.get_one::<String>("type").map(|s| s.as_str()).unwrap_or("virtiofs");
    let host_dir = args.get_one::<String>("host-dir").map(|s| s.to_string()).unwrap_or_else(|| ".".to_string());
    let guest_dir = args.get_one::<String>("guest-dir").map(|s| s.to_string());
    let headful = args.get_flag("headful");
    let daemonize = args.get_flag("daemonize");

    let display = if headful { DisplayMode::Headful } else { DisplayMode::Headless };

    // Canonicalize the path
    let canonical_host = std::env::current_dir()?
        .join(&host_dir)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve host directory '{}': {}", host_dir, e))?;

    println!("Starting VM '{}' with network mount configuration:", profile.name);
    println!("  Mount type: {}", mount_type);
    println!("  Host path: {}", canonical_host.display());

    // Determine guest path
    let effective_guest_path = match guest_dir {
        Some(path) => path,
        None => {
            match profile.os {
                GuestOs::Windows => r"C:\Users\vagrant\project".to_string(),
                _ => "/mnt/project".to_string(),
            }
        }
    };
    println!("  Guest path: {}", effective_guest_path);

    // Validate mount type for OS
    match mount_type {
        "virtiofs" => {
            if profile.os != GuestOs::Windows {
                println!("  Note: virtiofs is primarily for Windows. Use '9p' for Linux/macOS.");
            }
        }
        "smb" => {
            if profile.os != GuestOs::Windows {
                return Err("SMB mount is only supported for Windows guests".into());
            }
            println!("  SMB share will be available at: \\10.0.2.4\\qemu");
        }
        "9p" => {
            if profile.os == GuestOs::Windows {
                return Err("9p mount is not supported for Windows guests. Use virtiofs or SMB".into());
            }
        }
        "none" => {}
        _ => unreachable!(),
    }

    // Launch the VM using the default provider
    let provider = default_provider()?;
    let handle = provider.launch(&profile, display)?;

    // Save state
    let pid = handle.pid().map(|p| p as u32);
    let vm_state = state::VmState {
        profile_name: profile.name.to_string(),
        disk_path: profile.image_cache_path().to_string_lossy().to_string(),
        pid,
        provider_id: handle.provider_id,
        provider_internal_id: handle.internal_id.clone(),
        monitor_socket: String::new(),
        ssh_port: handle.resolved_ports.ssh_port,
        winrm_port: handle.resolved_ports.winrm_port,
        rdp_port: handle.resolved_ports.rdp_port,
        vnc_port: handle.resolved_ports.vnc_port,
        bootstrapped: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    state::save(&vm_state)?;

    println!("\n✓ VM '{}' started", profile.name);
    println!("  SSH: 127.0.0.1:{}", handle.resolved_ports.ssh_port);
    if let Some(p) = handle.resolved_ports.winrm_port {
        println!("  WinRM: 127.0.0.1:{p}");
    }
    println!("  VNC: 127.0.0.1:{}", handle.resolved_ports.vnc_port);

    if daemonize {
        println!("\n→ Running in daemonize mode - waiting for VM ready in background...");

        // Create channel for SSH ready signal
        let (ready_tx, ready_rx) = mpsc::channel::<Result<crate::ssh::VmSession, String>>();
        let profile_clone = profile.clone();

        // Spawn background thread to wait for SSH
        std::thread::spawn(move || {
            match wait_for_ssh_ready(&profile_clone, 120) {
                Ok(session) => {
                    let _ = ready_tx.send(Ok(session));
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            }
        });

        // Wait for ready signal (blocking)
        let session = match ready_rx.recv() {
            Ok(Ok(session)) => {
                println!("✓ SSH connected");
                session
            }
            Ok(Err(e)) => {
                println!("✗ SSH connection failed: {}", e);
                println!("\n→ Stopping VM...");
                let _ = provider.stop(&handle);
                return Err("VM did not become ready".into());
            }
            Err(_) => {
                println!("✗ Channel error");
                let _ = provider.stop(&handle);
                return Err("Failed to receive ready signal".into());
            }
        };

        // Run mount verification
        println!("\n→ Running mount verification...");
        let mut session = session;
        let (success, diagnosis) = verify_mount(&mut session, mount_type, &effective_guest_path, &profile)?;

        println!("{}", diagnosis);

        if success {
            println!("\n✓ Mount verification PASSED");
        } else {
            println!("\n⚠ Mount verification FAILED - see diagnosis above");
        }

        // Stop the VM
        println!("\n→ Stopping VM...");
        provider.stop(&handle)?;
        println!("✓ VM stopped");
        println!("\nNetwork test complete.");
        return Ok(());
    }

    // Non-daemonize: wait for SSH in main thread
    println!("\n→ Waiting for VM to be ready...");
    let mut session = match wait_for_ssh_ready(&profile, 120) {
        Ok(session) => {
            println!("✓ SSH connected");
            session
        }
        Err(e) => {
            println!("✗ SSH connection failed: {}", e);
            println!("\nDiagnosis:");
            println!("  1. Check if VM is running: testbed doctor {}", profile.name);
            println!("  2. View VM logs: ls ~/.cache/foundation_testbed/logs/");
            return Err("SSH connection failed".into());
        }
    };

    // Run mount verification
    println!("\n→ Running mount verification...");
    let (success, diagnosis) = verify_mount(&mut session, mount_type, &effective_guest_path, &profile)?;
    println!("{}", diagnosis);

    if success {
        println!("\n✓ Mount verification PASSED");
    } else {
        println!("\n⚠ Mount verification FAILED");
    }

    println!("\n✓ VM is running. Press Ctrl+C to stop or run 'testbed stop {}'", profile.name);

    // Keep process alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

pub fn cmd_adopt(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let disk = args.get_one::<String>("disk").unwrap();
    let profile = resolve_profile(name)?;

    let (adopted_profile, disk_path) = crate::qemu::adopt(&profile, Path::new(disk))?;
    println!("Adopted disk {} as VM '{}'", disk_path.display(), adopted_profile.name);
    println!("Start with: testbed start {}", adopted_profile.name);
    Ok(())
}

pub fn cmd_refresh_network(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    let vm = crate::qemu::refresh_network(&profile, DisplayMode::Headless)?;
    println!("VM '{}' restarted with fresh ports:", profile.name);
    println!("  SSH: 127.0.0.1:{}", vm.resolved_ports.ssh_port);
    if let Some(p) = vm.resolved_ports.winrm_port {
        println!("  WinRM: 127.0.0.1:{p}");
    }
    println!("  VNC: 127.0.0.1:{}", vm.resolved_ports.vnc_port);
    Ok(())
}

pub fn cmd_export(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let out = args.get_one::<String>("out").map(|s| std::path::PathBuf::from(s));
    let store = args.get_one::<String>("store").map(|s| s.clone());
    let version = args.get_one::<String>("version").map(|s| s.clone())
        .unwrap_or_else(|| "latest".to_string());
    let notes = args.get_one::<String>("notes").map(|s| s.clone())
        .unwrap_or_default();
    let include_bootstrap = args.get_flag("include-bootstrap");
    let clean = args.get_flag("clean");
    let shrink = args.get_flag("shrink");
    let compression = match args.get_one::<String>("compress").map(|s| s.as_str()) {
        Some("xz") | Some("lzma") => crate::export::Compression::Xz,
        Some("gzip") | Some("gz") => crate::export::Compression::Gzip,
        Some("none") => crate::export::Compression::None,
        Some(other) => return Err(format!("Unknown compression type: {other}. Use xz, gzip, or none.").into()),
        None => crate::export::Compression::Xz, // default: best compression
    };

    let opts = crate::export::ExportOptions {
        out,
        store,
        version,
        notes,
        include_bootstrap,
        clean,
        shrink,
        compression,
    };

    crate::export::export_vm(name, &opts)?;
    Ok(())
}
