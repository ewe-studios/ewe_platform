//! Probe whether this kernel can run the ptrace VFS spawn tests (spec 70
//! feature 83).
//!
//! WHY: the `ptrace_vfs` spawn tests fail when `EWE_REQUIRE_PTRACE=1` and the
//! kernel cannot trace a child. CI sets that variable, so the job must record
//! first why a failure happened. An emulated amd64 kernel returns `EIO` from
//! `PTRACE_GETREGS`; a native amd64 kernel returns the registers.
//!
//! WHAT: fork, have the child call `PTRACE_TRACEME` and stop with `SIGSTOP`,
//! then read its registers from the parent. Print one line:
//! `ptrace probe: available` or
//! `ptrace probe: unavailable (<errno name>: <number>, <cause>)`.
//!
//! HOW: `cargo test -p foundation_nativeapis --features vfs-native,vfs-ptrace
//! --test ptrace_probe -- --nocapture` runs in its own process. The probe test
//! is Linux-only, but [`ptrace_requirement`] and its tests compile and run on
//! any host, so a macOS machine can check the decision rule.
//!
//! A native amd64 Linux kernel comes from the `ubuntu-latest` CI runner, not
//! from a macOS runner: macOS has no Linux ptrace backend.
// The probe calls Unix `ptrace` errors, so it builds on Unix only.
#![cfg(unix)]

/// The decision the probe must apply for one kernel result.
#[derive(Debug, PartialEq, Eq)]
enum ProbeOutcome {
    /// `PTRACE_GETREGS` returned the registers.
    Available,
    /// The kernel refused the probe. `required` is `EWE_REQUIRE_PTRACE=1`.
    Unavailable { errno: i32, required: bool },
}

/// Decide what the probe reports and whether the caller must fail.
///
/// This function is pure, so a host without a Linux ptrace backend can test
/// both branches.
fn ptrace_requirement(available: Result<(), i32>, required: bool) -> ProbeOutcome {
    match available {
        Ok(()) => ProbeOutcome::Available,
        Err(errno) => ProbeOutcome::Unavailable { errno, required },
    }
}

/// The symbolic name of an errno the probe expects to see.
fn errno_name(errno: i32) -> &'static str {
    match errno {
        libc::EIO => "EIO",
        libc::EPERM => "EPERM",
        libc::ESRCH => "ESRCH",
        libc::EINVAL => "EINVAL",
        libc::ENOSYS => "ENOSYS",
        _ => "errno",
    }
}

/// The measured cause of a probe failure.
fn errno_cause(errno: i32) -> &'static str {
    match errno {
        libc::EIO => "emulated kernel (OrbStack/qemu)",
        libc::EPERM => "seccomp or Yama ptrace_scope",
        libc::ESRCH => "the tracee is gone",
        _ => "unknown cause",
    }
}

#[cfg(all(target_os = "linux", feature = "vfs-ptrace-nix"))]
mod common;

#[cfg(all(target_os = "linux", feature = "vfs-ptrace-nix"))]
#[test]
fn probe_ptrace() {
    let required = std::env::var("EWE_REQUIRE_PTRACE").as_deref() == Ok("1");
    match ptrace_requirement(common::ptrace_available(), required) {
        ProbeOutcome::Available => println!("ptrace probe: available"),
        ProbeOutcome::Unavailable { errno, required } => {
            let line = format!(
                "ptrace probe: unavailable ({}: {errno}, {})",
                errno_name(errno),
                errno_cause(errno)
            );
            println!("{line}");
            assert!(!required, "EWE_REQUIRE_PTRACE=1 but {line}");
        }
    }
}

#[test]
fn ptrace_requirement_accepts_an_available_kernel() {
    assert_eq!(ptrace_requirement(Ok(()), true), ProbeOutcome::Available);
    assert_eq!(ptrace_requirement(Ok(()), false), ProbeOutcome::Available);
}

#[test]
fn ptrace_requirement_records_the_errno_and_the_requirement() {
    assert_eq!(
        ptrace_requirement(Err(libc::EIO), true),
        ProbeOutcome::Unavailable {
            errno: libc::EIO,
            required: true,
        }
    );
    assert_eq!(
        ptrace_requirement(Err(libc::EPERM), false),
        ProbeOutcome::Unavailable {
            errno: libc::EPERM,
            required: false,
        }
    );
}

#[test]
fn errno_mapping_names_the_measured_causes() {
    assert_eq!(errno_name(libc::EIO), "EIO");
    assert_eq!(errno_cause(libc::EIO), "emulated kernel (OrbStack/qemu)");
    assert_eq!(errno_name(libc::EPERM), "EPERM");
    assert_eq!(errno_cause(libc::EPERM), "seccomp or Yama ptrace_scope");
    assert_eq!(errno_name(libc::ESRCH), "ESRCH");
    assert_eq!(errno_cause(libc::ESRCH), "the tracee is gone");
}
