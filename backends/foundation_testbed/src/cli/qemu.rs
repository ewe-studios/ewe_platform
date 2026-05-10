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

/// Start a VM with specific network/share configuration for testing.
/// This allows testing different mount types (virtiofs, SMB, 9p) without modifying profiles.
pub fn cmd_network(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    use crate::config::{DisplayMode, GuestOs};
    use crate::providers::default_provider;
    use crate::state;

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
            println!("  SMB share will be available at: \\\\10.0.2.4\\\\qemu");
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

    println!("\nVM '{}' started successfully", profile.name);
    println!("  SSH: 127.0.0.1:{}", handle.resolved_ports.ssh_port);
    if let Some(p) = handle.resolved_ports.winrm_port {
        println!("  WinRM: 127.0.0.1:{p}");
    }
    println!("  VNC: 127.0.0.1:{}", handle.resolved_ports.vnc_port);

    // Print mount-specific instructions
    println!("\nMount instructions:");
    match mount_type {
        "virtiofs" => {
            println!("  1. Run bootstrap to install virtio drivers: cargo run -p ewe_platform -- testbed bootstrap {}", profile.name);
            println!("  2. After bootstrap, verify mount: cargo run -p ewe_platform -- testbed exec {} --method winrm \"Test-Path '{}'\"", profile.name, effective_guest_path);
        }
        "smb" => {
            println!("  1. Inside Windows VM, mount the share:");
            println!("     net use Z: \\\\\\10.0.2.4\\\\qemu");
            println!("  2. Or via PowerShell:");
            println!("     cargo run -p ewe_platform -- testbed exec {} --method winrm \"net use Z: \\\\\\\\\\10.0.2.4\\\\qemu\"", profile.name);
        }
        "9p" => {
            println!("  1. Mount should be automatic via bootstrap");
            println!("  2. Verify: cargo run -p ewe_platform -- testbed exec {} \"mount | grep {}\"", profile.name, effective_guest_path);
        }
        "none" => {
            println!("  No mount configured");
        }
        _ => {}
    }

    if daemonize {
        println!("\n✓ VM running in background. Use 'testbed stop {}' to stop.", profile.name);
        println!("\nTo diagnose mount issues:");
        println!("  testbed network {} --type {} (without --daemonize)", profile.name, mount_type);
        return Ok(());
    }

    // Wait for VM to be ready and verify mount
    println!("\n→ Waiting for VM to be ready...");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut session = None;

    while std::time::Instant::now() < deadline {
        match crate::ssh::connect(&profile) {
            Ok(s) => {
                session = Some(s);
                println!("✓ SSH connected");
                break;
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }

    if session.is_none() {
        println!("✗ Timeout waiting for SSH");
        println!("\nDiagnosis:");
        println!("  1. Check if VM is running: testbed doctor {}", profile.name);
        println!("  2. View VM logs: ls ~/.cache/foundation_testbed/logs/");
        return Err("SSH connection failed".into());
    }

    let mut session = session.unwrap();

    // Verify mount based on type
    println!("\n→ Checking mount...");
    match mount_type {
        "virtiofs" => {
            // Check if virtiofs.exe is available
            let check = crate::ssh::exec(&mut session,
                "if (Test-Path 'C:\\Program Files\\Virtio-Win\\VioFS\\virtiofs.exe') { 'VIRTIOFS_OK' } else { 'VIRTIOFS_MISSING' }");
            match check {
                Ok(output) if output.contains("VIRTIOFS_OK") => {
                    println!("✓ virtiofs.exe found");
                }
                _ => {
                    println!("✗ virtiofs.exe not found (drivers not installed)");
                    println!("  Run: testbed bootstrap {}", profile.name);
                }
            }

            // Check mount point
            let mount_check = crate::ssh::exec(&mut session,
                &format!("if (Test-Path '{}') {{ 'MOUNT_OK' }} else {{ 'MOUNT_MISSING' }}", effective_guest_path));
            match mount_check {
                Ok(output) if output.contains("MOUNT_OK") => {
                    println!("✓ Mount point exists: {}", effective_guest_path);
                }
                _ => {
                    println!("✗ Mount point missing: {}", effective_guest_path);
                    println!("  The scheduled task may not have run yet.");
                    println!("  Check Event Log: testbed exec {} --method winrm 'Get-EventLog -LogName System -Source \"VirtIO*\" -Newest 10'", profile.name);
                }
            }
        }
        "smb" => {
            println!("→ SMB requires manual mount inside the VM:");
            println!("  net use Z: \\\\\\10.0.2.4\\\\qemu");
            println!("\n  Or run: testbed exec {} --method winrm \"net use Z: \\\\\\\\\\10.0.2.4\\\\qemu\"", profile.name);
        }
        "9p" => {
            let mount_check = crate::ssh::exec(&mut session,
                &format!("mount | grep '{}' || echo 'NOT_MOUNTED'", effective_guest_path));
            match mount_check {
                Ok(output) if output.contains("9p") => {
                    println!("✓ 9p mount active at {}", effective_guest_path);
                }
                _ => {
                    println!("✗ 9p mount not found at {}", effective_guest_path);
                    println!("  Attempting to mount...");
                    let mount_cmd = format!("sudo mkdir -p {} && sudo mount -t 9p -o trans=virtio,version=9p2000.L project {}",
                        effective_guest_path, effective_guest_path);
                    match crate::ssh::exec(&mut session, &mount_cmd) {
                        Ok(_) => println!("✓ Mount successful"),
                        Err(e) => println!("✗ Mount failed: {}", e),
                    }
                }
            }
        }
        "none" => {
            println!("  No mount configured (--type=none)");
        }
        _ => {}
    }

    println!("\n✓ VM is running. Press Ctrl+C to stop or run 'testbed stop {}'", profile.name);

    // Keep process alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
