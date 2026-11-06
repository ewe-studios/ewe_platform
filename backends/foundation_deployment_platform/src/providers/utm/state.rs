//! UTM VM state — delegates to the global state module.
//!
//! WHY: Both QEMU and UTM providers share a single state file format
//! (`VmState`) with `provider_id/provider_internal_id` fields.

use foundation_core::valtron::Clock;

use crate::config::Result;
use crate::state;

/// Re-export the global load/save/delete/list for UTM-specific callers.
pub use crate::state::{delete, list, load};

/// Check if a UTM VM has state.
#[must_use]
pub fn exists(name: &str) -> bool {
    state::exists(name)
}

/// Build a UTM VM state entry (saves immediately).
///
/// # Errors
/// Returns an error if the state cannot be saved.
pub fn save_from_runtime(
    profile_name: &str,
    bundle_path: &str,
    uuid: &str,
    ssh_port: u16,
    bootstrapped: bool,
    clock: &dyn Clock,
) -> Result<()> {
    let vm_state = state::from_runtime(
        profile_name,
        std::path::Path::new(bundle_path),
        crate::providers::ProviderId::Utm,
        uuid,
        ssh_port,
        None, // winrm_port
        None, // rdp_port
        0,    // vnc_port (UTM uses different display)
        bootstrapped,
        clock,
    );
    state::save(&vm_state)
}
