//! UTM VM state — delegates to the global state module.
//!
//! WHY: Both QEMU and UTM providers share a single state file format
//! (VmState) with provider_id/provider_internal_id fields.

use crate::vms::config::Result;
use crate::vms::state;

/// Re-export the global load/save/delete/list for UTM-specific callers.
pub use crate::vms::state::{load, delete, list};

/// Check if a UTM VM has state.
pub fn exists(name: &str) -> bool {
    state::exists(name)
}

/// Build a UTM VM state entry (saves immediately).
pub fn save_from_runtime(
    profile_name: &str,
    bundle_path: &str,
    uuid: &str,
    ssh_port: u16,
    bootstrapped: bool,
) -> Result<()> {
    let vm_state = state::from_runtime(
        profile_name,
        std::path::Path::new(bundle_path),
        crate::vms::providers::ProviderId::Utm,
        uuid,
        ssh_port,
        None,  // winrm_port
        None,  // rdp_port
        0,     // vnc_port (UTM uses different display)
        bootstrapped,
    );
    state::save(&vm_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_from_runtime_creates_valid_state() {
        let _ = save_from_runtime("test-utm", "/tmp/test.utm", "test-uuid", 2222, false);
        assert!(exists("test-utm"));
        let _ = delete("test-utm");
    }
}
