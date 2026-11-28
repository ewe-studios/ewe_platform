//! The container detection of the slave.
//!
//! The slave skips its virtual-machine boot duties in a container. The
//! Cloudflare container runtime writes neither of the two file markers, so the
//! detection also reads its environment (issue #1133).

use foundation_sandbox::native::slave::pid1::container_from_markers;

/// A Docker or a Podman marker says that the process runs in a container.
#[test]
fn a_runtime_marker_is_a_container() {
    assert!(container_from_markers(true, false, false));
    assert!(container_from_markers(false, true, false));
}

/// The Cloudflare environment says that the process runs in a container.
#[test]
fn the_cloudflare_environment_is_a_container() {
    assert!(container_from_markers(false, false, true));
}

/// A bare process is not in a container.
#[test]
fn a_bare_process_is_not_a_container() {
    assert!(!container_from_markers(false, false, false));
}
