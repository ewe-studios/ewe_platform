//! AppleScript wrapper for UTM configuration via `osascript`.
//!
//! UTM doesn't expose a CLI for network/resource config or bundle import.
//! We use AppleScript to drive UTM's internal configuration:
//! - Network: discover emulated NICs, configure port forwards
//! - Resources: set CPU, memory via `update configuration`
//! - Bundle import: `import new virtual machine from POSIX file`

use std::process::Command;

use crate::vms::config::{Result, TestbedError};

/// Run an AppleScript snippet via `osascript`.
fn run_applescript(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("running osascript: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "osascript failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Find the emulated network interface index for a VM.
///
/// Returns the NIC index (e.g. 0) or None if no emulated NIC found.
fn find_emulated_nic(uuid: &str) -> Result<u32> {
    let script = format!(
        r#"
            tell application "UTM"
                set vm to virtual machine id "{uuid}"
                set cfg to configuration of vm
                set nis to network interfaces of cfg
                repeat with ni in nis
                    if mode of ni is emulated then
                        return index of ni
                    end if
                end repeat
                return -1
            end tell
        "#
    );
    let result = run_applescript(&script)?;
    let idx = result.trim().parse::<u32>().map_err(|_| {
        TestbedError::Qcow2Error {
            message: format!("parsing emulated NIC index from osascript output: {result:?}"),
        }
    })?;
    if idx == u32::MAX || idx == 0 {
        // osascript may return -1 as "4294967295" or similar
    }
    Ok(idx)
}

/// Configure port forwarding for a UTM VM.
///
/// Finds the emulated NIC, then sets host->guest port forwards.
/// UTM uses user-mode networking (SLIRP) by default, so port
/// forwards are required for host-to-guest access.
///
/// Arguments:
/// - `uuid`: UTM VM UUID
/// - `ssh_port`: Host port to forward to guest :22
/// - `extra_forwards`: Additional (guest_port, host_port) pairs
pub fn configure_port_forwards(
    uuid: &str,
    ssh_port: u16,
    extra_forwards: &[(u16, u16)],
) -> Result<()> {
    let nic_index = find_emulated_nic(uuid)?;

    // Build port forward rules
    let mut rules = vec![format!(
        "set newPortForward to {{protocol:\"TcPp\", guest address:\"\", guest port:\"22\", host address:\"127.0.0.1\", host port:\"{ssh_port}\"}}"
    )];
    for (guest_port, host_port) in extra_forwards {
        rules.push(format!(
            "set newPortForward to {{protocol:\"TcPp\", guest address:\"\", guest port:\"{guest_port}\", host address:\"127.0.0.1\", host port:\"{host_port}\"}}"
        ));
    }

    let set_rules: String = rules
        .iter()
        .map(|r| format!("\n      {r}\n      copy newPortForward to the end of portForwards"))
        .collect();

    let script = format!(
        r#"
            tell application "UTM"
                set vm to virtual machine id "{uuid}"
                set config to configuration of vm
                set networkInterfaces to network interfaces of config
                repeat with anInterface in networkInterfaces
                    if index of anInterface is {nic_index} then
                        set portForwards to {{}}
                        {set_rules}
                        set port forwards of anInterface to portForwards
                    end if
                end repeat
                update configuration of vm with config
            end tell
        "#
    );
    run_applescript(&script)?;
    Ok(())
}

/// Configure CPU and memory resources for a UTM VM.
///
/// Uses `update configuration` pattern which is the correct UTM
/// AppleScript API — not direct property assignment.
pub fn configure_resources(uuid: &str, memory_mib: u32, cpu_cores: u32) -> Result<()> {
    let script = format!(
        r#"
            tell application "UTM"
                set vm to virtual machine id "{uuid}"
                set cfg to configuration of vm
                set memory of cfg to {memory_mib}
                set cpu cores of cfg to {cpu_cores}
                update configuration of vm with cfg
            end tell
        "#
    );
    run_applescript(&script)?;
    Ok(())
}

/// Import a .utm bundle into UTM.app.
///
/// Uses the correct AppleScript API: `import new virtual machine from POSIX file`.
/// Returns the UUID of the imported VM.
///
/// Arguments:
/// - `bundle_path`: Absolute path to the .utm bundle directory
pub fn import_bundle(bundle_path: &str) -> Result<String> {
    // Snapshot existing VM UUIDs so we can find the newly imported one
    let before: std::collections::HashSet<String> = super::utmctl::list_vms()?
        .into_iter()
        .map(|e| e.uuid)
        .collect();

    // Import via correct AppleScript API
    let script = format!(
        r#"tell application "UTM" to import new virtual machine from POSIX file "{}""#,
        bundle_path
    );
    run_applescript(&script)?;

    // Wait for the new VM to appear (up to 30s)
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 30 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        for entry in super::utmctl::list_vms()? {
            if !before.contains(&entry.uuid) {
                return Ok(entry.uuid);
            }
        }
    }

    Err(TestbedError::Qcow2Error {
        message: "import succeeded but no new VM appeared in utmctl list within 30s".to_string(),
    })
}

/// Verify that UTM placed the imported bundle on disk.
///
/// After import, UTM stores bundles at:
/// `~/Library/Containers/com.utmapp.UTM/Data/Documents/{display_name}.utm`
pub fn verify_imported_bundle(display_name: &str) -> bool {
    dirs::home_dir()
        .map(|home| {
            home.join("Library/Containers/com.utmapp.UTM/Data/Documents")
                .join(format!("{display_name}.utm"))
                .exists()
        })
        .unwrap_or(false)
}

/// Configure a shared directory for a UTM VM.
///
/// UTM shares host directories with guests via its built-in shared
/// directory feature. The directory is exposed inside the guest using
/// the 9p/virtio protocol with the given mount tag.
///
/// Arguments:
/// - `uuid`: UTM VM UUID
/// - `host_path`: Absolute path on the host
/// - `mount_tag`: Tag name used to identify the mount inside the guest
/// - `readonly`: If true, guest has read-only access
pub fn configure_shared_directory(uuid: &str, host_path: &str, mount_tag: &str, readonly: bool) -> Result<()> {
    let read_only_str = if readonly { "true" } else { "false" };
    let script = format!(
        r#"
        tell application "UTM"
            set vm to first virtual machine where id is "{uuid}"
            set sharedDirectory to {{sharedTag:"{mount_tag}", hostPath:"{host_path}", readOnly:{read_only_str}}}
            set shared directories of vm to {{sharedDirectory}}
        end tell
        "#
    );
    run_applescript(&script)?;
    Ok(())
}

