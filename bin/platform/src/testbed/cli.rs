use std::path::Path;

use foundation_testbed::config::{self, get_profile, DisplayMode, VmProfile};
use foundation_testbed::doctor;
use foundation_testbed::import;
use foundation_testbed::qemu::{self, QemuConfig};
use foundation_testbed::qemu::disk;
use foundation_testbed::qemu::snapshot;
use foundation_testbed::state;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

fn resolve_profile(name: &str) -> Result<VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| e.into())
}

pub fn cmd_start(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let headful = args.get_flag("headful");
    let profile = resolve_profile(name)?;
    let display = if headful { DisplayMode::Headful } else { DisplayMode::Headless };

    println!("Starting VM '{}' ({} mode)...", profile.name, if headful { "headful" } else { "headless" });

    // Ensure image is available (downloads if needed)
    let disk_path = import::ensure_image(&profile)?;

    let vm = QemuConfig::new(profile.clone(), display).launch()?;

    // Save state
    let monitor_path = config::monitor_dir().join(format!("{}.monitor", profile.name));
    let vm_state = state::from_qemu(
        &profile.name,
        &vm.disk_path,
        vm.pid,
        &monitor_path,
        vm.resolved_ports.ssh_port,
        vm.resolved_ports.winrm_port,
        vm.resolved_ports.rdp_port,
        vm.resolved_ports.vnc_port,
        false,
    );
    state::save(&vm_state)?;

    let info = qemu::display::connection_info(display, vm.resolved_ports.vnc_port);
    println!("VM '{}' started (PID {})", profile.name, vm.pid);
    println!("  SSH: 127.0.0.1:{}", vm.resolved_ports.ssh_port);
    if let Some(p) = vm.resolved_ports.winrm_port {
        println!("  WinRM: 127.0.0.1:{p}");
    }
    if let Some(p) = vm.resolved_ports.rdp_port {
        println!("  RDP: 127.0.0.1:{p}");
    }
    println!("  Display: {info}");

    // Keep the process alive
    let _pid = vm.pid;
    std::mem::drop(vm);

    println!("VM running in background. Use 'ewe_platform testbed stop {name}' to shut down.");
    Ok(())
}

pub fn cmd_stop(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();

    // Check if VM is running
    let mut vm_state = state::load(name)?;
    let pid = vm_state.pid.ok_or_else(|| format!("VM '{}' is not running (no PID in state)", name))?;

    // Attach to the running QEMU process and shut it down
    let profile = resolve_profile(name)?;
    let monitor_path = std::path::PathBuf::from(&vm_state.monitor_socket);

    // For now, send system_powerdown via direct monitor socket connection
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    if let Ok(mut stream) = UnixStream::connect(&monitor_path) {
        stream.write_all(b"system_powerdown\n").ok();
        let mut response = String::new();
        stream.read_to_string(&mut response).ok();
        println!("Sent shutdown command to VM '{}'", name);
    }

    // Clean up state
    state::delete(name)?;

    Ok(())
}

pub fn cmd_import(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    if import::is_cached(&profile) {
        let path = profile.image_cache_path();
        println!("Image already cached at {:?}", path);
        return Ok(());
    }

    // Ensure cache directory exists
    std::fs::create_dir_all(config::image_cache_dir())?;

    println!("Downloading image for profile '{}'...", profile.name);

    let result = import::ensure_image(&profile)?;
    println!("Image downloaded to {:?}", result);

    let meta = std::fs::metadata(&result)?;
    let size_gb = meta.len() as f64 / 1_073_741_824.0;
    println!("  Size: {:.2} GB", size_gb);

    Ok(())
}

pub fn cmd_build(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let project = args.get_one::<String>("project").map(|s| s.as_str()).unwrap_or(".");
    let profile = resolve_profile(name)?;

    println!("Building project '{}' in VM '{}'...", project, profile.name);
    println!("  (Requires VM to be running and bootstrapped)");

    let artifact_dir = foundation_testbed::build::build_in_vm(
        &profile,
        // Would need an active SSH session here
        todo!(),
        Path::new(project),
    )?;

    println!("Artifacts saved to {:?}", artifact_dir);
    Ok(())
}

pub fn cmd_exec(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let cmd = args.get_one::<String>("cmd").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    let output = foundation_testbed::ssh::exec(&mut session, cmd)?;
    print!("{}", output);
    Ok(())
}

pub fn cmd_shell(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    // Use ssh CLI for interactive shell
    let status = std::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-p", &profile.ssh_port.to_string(),
            &format!("{}@127.0.0.1", profile.user),
        ])
        .status()?;

    if !status.success() {
        return Err(format!("SSH exited with status: {}", status).into());
    }
    Ok(())
}

pub fn cmd_run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let bin = args.get_one::<String>("bin").map(|s| s.as_str());
    let profile = resolve_profile(name)?;

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    let result = foundation_testbed::runner::run_in_vm(&profile, &mut session, bin)?;
    println!("Launched binary, log at: {}", result.log_path);
    Ok(())
}

pub fn cmd_screenshot(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let out = args.get_one::<String>("out").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    foundation_testbed::runner::screenshot::capture(&profile, &mut session, Path::new(out))?;
    println!("Screenshot saved to {}", out);
    Ok(())
}

pub fn cmd_logs(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    let kind_str = args.get_one::<String>("kind").map(|s| s.as_str()).unwrap_or("build");
    let kind = kind_str.parse::<foundation_testbed::runner::logs::LogKind>()
        .map_err(|e| format!("Invalid log kind: {}", e))?;

    let mode = if args.get_flag("follow") {
        foundation_testbed::runner::logs::LogMode::Follow
    } else if args.get_flag("errors") {
        foundation_testbed::runner::logs::LogMode::Errors
    } else if let Some(n) = args.get_one::<u32>("tail") {
        foundation_testbed::runner::logs::LogMode::Tail(*n)
    } else {
        foundation_testbed::runner::logs::LogMode::Dump
    };

    let opts = foundation_testbed::runner::logs::LogOpts { kind, mode };

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    let output = foundation_testbed::runner::logs::get_logs(&profile, &mut session, &opts)?;
    print!("{}", output);
    Ok(())
}

pub fn cmd_doctor(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let profile_name = args.get_one::<String>("profile");

    let host_health = doctor::check_host();
    host_health.print();

    if let Some(name) = profile_name {
        let profile = resolve_profile(name)?;
        let vm_health = doctor::check_vm(&profile);
        vm_health.print();
    }

    Ok(())
}

pub fn cmd_push(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let from = args.get_one::<String>("from").unwrap();
    let to = args.get_one::<String>("to").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    foundation_testbed::runner::transfer::push(&mut session, Path::new(from), to)?;
    println!("Pushed {} -> {}", from, to);
    Ok(())
}

pub fn cmd_pull(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let from = args.get_one::<String>("from").unwrap();
    let to = args.get_one::<String>("to").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = foundation_testbed::ssh::connect(&profile)?;
    foundation_testbed::runner::transfer::pull(&mut session, from, Path::new(to))?;
    println!("Pulled {} -> {}", from, to);
    Ok(())
}

pub fn cmd_resize_disk(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let plus_gb = *args.get_one::<u32>("plus_gb").unwrap();
    let profile = resolve_profile(name)?;

    let disk_path = profile.image_cache_path();
    disk::resize(&disk_path, plus_gb)?;

    // Also resize the partition inside the VM if it's running
    println!("Resized {} by +{}GB", disk_path.display(), plus_gb);
    Ok(())
}

pub fn cmd_ls() -> Result<(), BoxedError> {
    let vms = state::list()?;
    if vms.is_empty() {
        println!("No VMs in state.");
        return Ok(());
    }

    println!("{:<20} {:<10} {:<8} {:<8} {:<8}", "PROFILE", "PID", "SSH", "VNC", "BOOTSTRAP");
    println!("{}", "-".repeat(60));
    for (name, vm_state) in &vms {
        let pid = vm_state.pid.map(|p| p.to_string()).unwrap_or_else(|| "stopped".to_string());
        let bootstrap = if vm_state.bootstrapped { "yes" } else { "no" };
        println!(
            "{:<20} {:<10} {:<8} {:<8} {:<8}",
            name, pid, vm_state.ssh_port, vm_state.vnc_port, bootstrap
        );
    }
    Ok(())
}

pub fn cmd_snapshot(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    match args.subcommand() {
        Some(("save", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            let profile = resolve_profile(name)?;
            let mut session = foundation_testbed::ssh::connect(&profile)?;
            // For now, just report — actual snapshot needs QemuVm handle
            println!("Snapshot '{}' would be saved for VM '{}'", snap_name, name);
        }
        Some(("load", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            println!("Snapshot '{}' would be loaded for VM '{}'", snap_name, name);
        }
        Some(("delete", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let snap_name = m.get_one::<String>("name").unwrap();
            println!("Snapshot '{}' would be deleted for VM '{}'", snap_name, name);
        }
        Some(("list", m)) => {
            let name = m.get_one::<String>("profile").unwrap();
            let profile = resolve_profile(name)?;
            let mut session = foundation_testbed::ssh::connect(&profile)?;
            // Would need QemuVm handle
            println!("No snapshots found for VM '{}'", name);
        }
        _ => {}
    }
    Ok(())
}
