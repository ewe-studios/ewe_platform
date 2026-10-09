//! Build pipeline — uses mounted project directory, tool installation, cargo build, artifact retrieval.
//!
//! Uses the mounted project directory (via 9p/virtiofs) instead of tar+scp sync.
//! All commands run via nushell (`nu -c "..."`) for cross-platform consistency.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{GuestOs, Result, TestbedError, VmProfile};
use crate::ssh::VmSession;

pub mod logs;
pub mod multiarch;

/// Build target triple for the given guest OS.
pub fn target_triple(os: GuestOs) -> &'static str {
    match os {
        GuestOs::Windows => "x86_64-pc-windows-msvc",
        GuestOs::Linux => "aarch64-unknown-linux-gnu",
        GuestOs::MacOS => "aarch64-apple-darwin",
    }
}

/// Get the guest mount path for the project directory.
pub fn guest_project_path(os: GuestOs) -> &'static str {
    match os {
        GuestOs::Windows => "C:\\Users\\vagrant\\project",
        GuestOs::Linux => "/mnt/project",
        GuestOs::MacOS => "/Volumes/project",
    }
}

/// Run the full build pipeline inside a VM.
///
/// 1. Pre-flight check
/// 2. Verify mount is accessible
/// 3. Tool installation (mise install)
/// 4. Build (cargo tauri build)
/// 5. Artifact retrieval (tar + scp back)
pub fn build_in_vm(
    profile: &VmProfile,
    session: &mut VmSession,
    project_dir: &Path,
) -> Result<PathBuf> {
    // Pre-flight
    preflight_check(project_dir)?;

    // Verify mount is accessible
    verify_mount(session, profile.os)?;

    // Tool installation
    install_project_tools(session, profile.os)?;

    // Build
    let exit_code = run_build(session, profile)?;

    // Artifact retrieval
    let artifact_dir = retrieve_artifacts(session, profile)?;

    if exit_code != 0 {
        // Extract errors from build log
        let _errors = logs::dump_build_log_errors(session)?;
        return Err(TestbedError::BuildFailed {
            target: target_triple(profile.os).to_string(),
            code: exit_code,
        });
    }

    Ok(artifact_dir)
}

/// Pre-flight: verify project has required tools in mise.toml.
fn preflight_check(project_dir: &Path) -> Result<()> {
    let mise_toml = project_dir.join("mise.toml");
    if !mise_toml.exists() {
        return Err(TestbedError::BootstrapFailed {
            step: "preflight".to_string(),
            message: format!("no mise.toml found in {}", project_dir.display()),
        });
    }

    let content = std::fs::read_to_string(&mise_toml).map_err(|e| TestbedError::BootstrapFailed {
        step: "preflight".to_string(),
        message: format!("reading {}: {e}", mise_toml.display()),
    })?;

    if !content.contains("rust") {
        return Err(TestbedError::BootstrapFailed {
            step: "preflight".to_string(),
            message: "mise.toml must declare 'rust' tool".to_string(),
        });
    }

    Ok(())
}

/// Verify the project mount is accessible inside the VM.
fn verify_mount(session: &mut VmSession, os: GuestOs) -> Result<()> {
    let guest_path = guest_project_path(os);

    let check_cmd = match os {
        GuestOs::Windows => {
            format!(
                "if (Test-Path '{}') {{ 'MOUNT_OK' }} else {{ 'MOUNT_MISSING' }}",
                guest_path
            )
        }
        GuestOs::Linux | GuestOs::MacOS => {
            format!("[ -d '{}' ] && echo 'MOUNT_OK' || echo 'MOUNT_MISSING'", guest_path)
        }
    };

    let result = crate::ssh::exec(session, &check_cmd)?;
    if !result.contains("MOUNT_OK") {
        return Err(TestbedError::BootstrapFailed {
            step: "verify_mount".to_string(),
            message: format!(
                "project mount not accessible at {}. Run 'testbed bootstrap {}' first.",
                guest_path,
                if os == GuestOs::Windows { "windows-build" } else { "linux-build" }
            ),
        });
    }

    // Check for Cargo.toml or package.json to confirm it's a valid project
    let project_check = match os {
        GuestOs::Windows => {
            format!(
                "if (Test-Path '{}\\Cargo.toml') {{ 'PROJECT_OK' }} else {{ 'NO_PROJECT' }}",
                guest_path
            )
        }
        GuestOs::Linux | GuestOs::MacOS => {
            format!("[ -f '{}/Cargo.toml' ] && echo 'PROJECT_OK' || echo 'NO_PROJECT'", guest_path)
        }
    };

    let result = crate::ssh::exec(session, &project_check)?;
    if !result.contains("PROJECT_OK") {
        return Err(TestbedError::BootstrapFailed {
            step: "verify_mount".to_string(),
            message: format!(
                "no Cargo.toml found in mounted project at {}. Is the project mounted correctly?",
                guest_path
            ),
        });
    }

    Ok(())
}

/// Install project tools via mise in the mounted project directory.
fn install_project_tools(session: &mut VmSession, os: GuestOs) -> Result<()> {
    let guest_path = guest_project_path(os);

    // Set sccache environment variables for caching
    let env_prefix = match os {
        GuestOs::Windows => {
            "$env:MISE_CARGO_BINSTALL = $true; $env:CARGO_INCREMENTAL = 0; $env:RUSTC_WRAPPER = 'sccache';"
        }
        GuestOs::Linux | GuestOs::MacOS => {
            "export MISE_CARGO_BINSTALL=true CARGO_INCREMENTAL=0 RUSTC_WRAPPER=sccache"
        }
    };

    // Run mise install in the mounted project directory
    let install_cmd = match os {
        GuestOs::Windows => {
            format!(
                "{} cd '{}'; mise install",
                env_prefix,
                guest_path
            )
        }
        GuestOs::Linux | GuestOs::MacOS => {
            format!("{} && cd '{}' && mise install", env_prefix, guest_path)
        }
    };

    crate::ssh::exec(session, &install_cmd)?;
    Ok(())
}

/// Run cargo tauri build inside the VM using the mounted project directory.
fn run_build(session: &mut VmSession, profile: &VmProfile) -> Result<i32> {
    let target = target_triple(profile.os);
    let guest_path = guest_project_path(profile.os);

    // Set up build environment and run build
    let build_cmd = match profile.os {
        GuestOs::Windows => {
            format!(
                r#"
$env:TESTBED_PROJECT_DIR = '{}';
$env:TESTBED_TARGET = '{}';
$env:TESTBED_LOG = "$env:USERPROFILE\.testbed-build\build.log";
$env:RUSTC_WRAPPER = "sccache";
$env:CARGO_INCREMENTAL = "0";
New-Item -Path "$env:USERPROFILE\.testbed-build" -ItemType Directory -Force | Out-Null;
cd '{}';
cargo tauri build --target {} 2>&1 | Tee-Object -FilePath "$env:USERPROFILE\.testbed-build\build.log"
"#,
                guest_path, target, guest_path, target
            )
        }
        GuestOs::MacOS => {
            format!(
                r#"
export TESTBED_PROJECT_DIR='{}';
export TESTBED_TARGET='{}';
export TESTBED_LOG="$HOME/.testbed-build/build.log";
export RUSTC_WRAPPER="sccache";
export CARGO_INCREMENTAL="0";
mkdir -p ~/.testbed-build;
cd '{}';
cargo tauri build --target {} 2>&1 | tee ~/.testbed-build/build.log
"#,
                guest_path, target, guest_path, target
            )
        }
        GuestOs::Linux => {
            format!(
                r#"
export TESTBED_PROJECT_DIR='{}';
export TESTBED_TARGET='{}';
export TESTBED_LOG="$HOME/.testbed-build/build.log";
export RUSTC_WRAPPER="sccache";
export CARGO_INCREMENTAL="0";
mkdir -p ~/.testbed-build;
cd '{}';
cargo tauri build --target {} 2>&1 | tee ~/.testbed-build/build.log
"#,
                guest_path, target, guest_path, target
            )
        }
    };

    let (_output, exit_code) = crate::ssh::exec_with_exit(session, &build_cmd)?;
    Ok(exit_code)
}

/// Retrieve build artifacts from the VM back to the host.
fn retrieve_artifacts(session: &mut VmSession, profile: &VmProfile) -> Result<PathBuf> {
    let target = target_triple(profile.os);
    let guest_path = guest_project_path(profile.os);
    let platform = match profile.os {
        GuestOs::Windows => "windows",
        GuestOs::Linux => "linux",
        GuestOs::MacOS => "macos",
    };
    let arch = match target {
        "x86_64-pc-windows-msvc" => "x86_64",
        "aarch64-unknown-linux-gnu" => "arm64",
        "aarch64-apple-darwin" => "arm64",
        _ => "unknown",
    };

    // The bundle dir is at target/{target}/release/bundle relative to project
    let bundle_dir = format!("{}/target/{}/release/bundle", guest_path, target);

    // Tar the bundle dir on VM
    crate::ssh::exec(session, &format!(
        "cd ~ && tar czf /tmp/artifacts.tar.gz -C {} . 2>/dev/null || true",
        bundle_dir
    ))?;

    // SCP back to host
    let host_artifact_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/home/darkvoid"))
        .join(format!(".build/{platform}/{arch}"));
    std::fs::create_dir_all(&host_artifact_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating {host_artifact_dir:?}: {e}"),
    })?;

    let local_tar = std::env::temp_dir().join("testbed-artifacts.tar.gz");
    crate::ssh::download(session, "/tmp/artifacts.tar.gz", &local_tar)?;

    // Extract on host
    let status = Command::new("tar")
        .args([
            "xzf",
            local_tar.to_str().unwrap(),
            "-C",
            host_artifact_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("extracting artifacts: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: "tar extract failed".to_string(),
        });
    }

    let _ = std::fs::remove_file(&local_tar);

    Ok(host_artifact_dir)
}

