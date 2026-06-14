//! Linux x86_64 cross-compile setup from an ARM64 Linux VM.
//!
//! Enables multiarch, installs amd64 system libraries, and configures
//! cross-compilation environment variables.

use crate::vms::config::Result;
use crate::vms::ssh::VmSession;

/// Debian/Ubuntu packages needed for x86_64 cross-compilation.
const CROSS_DEPS: &[&str] = &[
    "gcc-x86-64-linux-gnu",
    "g++-x86-64-linux-gnu",
];

/// Enable multiarch support for x86_64 cross-compilation.
///
/// This should be called before `build_in_vm` when targeting
/// `x86_64-unknown-linux-gnu` from an ARM64 VM.
pub fn setup_multiarch(session: &mut VmSession) -> Result<()> {
    // Step 1: Enable multiarch
    crate::vms::ssh::exec(session, "dpkg --add-architecture amd64")?;
    crate::vms::ssh::exec(session, "apt-get update -qq")?;

    // Step 2: Install cross-compilation toolchain
    let deps = CROSS_DEPS.join(" ");
    crate::vms::ssh::exec(
        session,
        &format!("DEBIAN_FRONTEND=noninteractive apt-get install -y {deps}"),
    )?;

    Ok(())
}

/// Get environment variable settings for x86_64 cross-compilation.
///
/// Returns a shell snippet that sets CARGO_TARGET_*, PKG_CONFIG_*, etc.
pub fn cross_compile_env() -> &'static str {
    "export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc\n\
     export PKG_CONFIG_ALLOW_CROSS=1\n\
     export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_compile_env_contains_expected_vars() {
        let env = cross_compile_env();
        assert!(env.contains("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"));
        assert!(env.contains("PKG_CONFIG_ALLOW_CROSS"));
        assert!(env.contains("PKG_CONFIG_PATH"));
    }

    #[test]
    fn test_cross_deps_not_empty() {
        assert!(!CROSS_DEPS.is_empty());
    }
}
