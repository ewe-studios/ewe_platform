//! `#[docker_container]` without a `clock = <expr>` argument, outside a valtron
//! pool (spec 72, feature 04 — issue #220).
//!
//! WHY: the macro used to build a clock-less `ContainerConfig`; starting it
//! reached `fork_ctx().clock()` and panicked. The macro must resolve its default
//! clock from the pool, and a missing pool must be a typed error, never a panic.
//!
//! WHAT: one `Unit`-returning function checks the non-panicking skip. One
//! `DockerResult<()>`-returning function checks the typed error the macro
//! returns.
//!
//! HOW: this binary never initializes a valtron pool, so the default clock
//! source has no pool to read.

use std::sync::atomic::{AtomicBool, Ordering};

use foundation_deployment_platform::docker::{
    ContainerGroup, DockerError, DockerResult, docker_err,
};
use foundation_deployment_platform::docker_container;

static UNIT_BODY_RAN: AtomicBool = AtomicBool::new(false);

#[docker_container(image = "redis:7-alpine", as = "cache", port = 6379)]
fn unit_body(_containers: ContainerGroup) {
    UNIT_BODY_RAN.store(true, Ordering::SeqCst);
}

/// A plain function returns without running its body when no pool can supply a
/// clock. It must not panic.
#[test]
fn the_macro_outside_a_pool_returns_without_panicking() {
    unit_body();

    assert!(
        !UNIT_BODY_RAN.load(Ordering::SeqCst),
        "the body must not run when the macro cannot resolve a clock"
    );
}

#[docker_container(image = "redis:7-alpine", as = "cache", port = 6379)]
fn start_without_a_clock(containers: ContainerGroup) -> DockerResult<()> {
    // The body must never run: the macro cannot resolve a clock. Return an
    // error anyway, so the function has a real error path.
    Err(docker_err(DockerError::InvalidConfig(format!(
        "the body ran with {} containers and no clock",
        containers.len()
    ))))
}

/// A function that returns a `DockerResult` gets the typed error when the macro
/// cannot resolve a clock.
#[test]
fn a_container_config_without_a_clock_is_a_typed_error() {
    let err = start_without_a_clock().expect_err("no pool means no default clock");

    assert!(
        matches!(err.current_context(), DockerError::MissingClock),
        "a missing clock must be a typed error, got {err:?}"
    );
}
