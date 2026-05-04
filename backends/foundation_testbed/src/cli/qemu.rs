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
