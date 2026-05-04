//! Provider-agnostic CLI commands (start, stop, import, doctor, ls, init).

use clap::ArgMatches;
use crate::providers::default_provider;
use crate::config::{self, get_profile, DisplayMode};
use crate::state;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
}

pub fn cmd_start(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let headful = args.get_flag("headful");
    let profile = resolve_profile(name)?;
    let display = if headful { DisplayMode::Headful } else { DisplayMode::Headless };

    eprintln!("Starting VM '{}' ({} mode)...", profile.name, if headful { "headful" } else { "headless" });

    let provider = default_provider()?;
    let handle = provider.launch(&profile, display)?;

    eprintln!("VM '{}' started", profile.name);
    eprintln!("  SSH: 127.0.0.1:{}", handle.resolved_ports.ssh_port);
    if let Some(p) = handle.resolved_ports.winrm_port {
        eprintln!("  WinRM: 127.0.0.1:{p}");
    }
    if let Some(p) = handle.resolved_ports.rdp_port {
        eprintln!("  RDP: 127.0.0.1:{p}");
    }
    eprintln!("  VNC: 127.0.0.1:{}", handle.resolved_ports.vnc_port);

    // Release handle (provider has already saved state)
    drop(handle);

    println!("VM running in background. Use 'testbed stop {name}' to shut down.");
    Ok(())
}

pub fn cmd_stop(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();

    let vm_state = state::load(name)?;
    let _pid = vm_state.pid.ok_or_else(|| format!("VM '{}' is not running (no PID in state)", name))?;

    let provider = default_provider()?;

    // Build a minimal handle from state for the provider
    let profile = resolve_profile(name)?;
    let handle = crate::providers::VmHandle {
        profile,
        provider_id: vm_state.provider_id,
        internal_id: vm_state.provider_internal_id.clone(),
        resolved_ports: crate::providers::ResolvedPorts {
            ssh_port: vm_state.ssh_port,
            winrm_port: vm_state.winrm_port,
            rdp_port: vm_state.rdp_port,
            vnc_port: vm_state.vnc_port,
        },
        display_mode: DisplayMode::Headless,
    };

    provider.stop(&handle)?;
    println!("VM '{}' stopped.", name);
    Ok(())
}

pub fn cmd_import(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    let provider = default_provider()?;
    let path = provider.ensure_image(&profile)?;

    println!("Image available at {:?}", path);
    let meta = std::fs::metadata(&path)?;
    let size_gb = meta.len() as f64 / 1_073_741_824.0;
    println!("  Size: {:.2} GB", size_gb);
    Ok(())
}

pub fn cmd_doctor(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let profile_name = args.get_one::<String>("profile");

    let provider = default_provider()?;
    let host_health = provider.host_health();
    host_health.print();

    if let Some(name) = profile_name {
        let profile = resolve_profile(name)?;
        let vm_health = crate::doctor::check_vm(&profile);
        vm_health.print();
    }

    Ok(())
}

pub fn cmd_ls() -> std::result::Result<(), BoxedError> {
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

pub fn cmd_init(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let vms: Vec<String> = args
        .get_many::<String>("vms")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_else(|| vec!["linux-build".to_string()]);

    crate::init::init(std::path::Path::new("."), &vms)?;
    println!("Scaffolded .testbed/ directory with:");
    println!("  testbed.toml — VM definitions");
    println!("  .gitignore — ignores state/logs/artifacts");
    for vm in &vms {
        println!("  scripts/{vm}/startup/ and shutdown/ — custom scripts");
    }
    Ok(())
}
