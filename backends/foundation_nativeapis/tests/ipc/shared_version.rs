//! `shared::ipc::version` tests (moved from `src/shared/ipc/version.rs`).

use foundation_nativeapis::ipc::version::Version;

#[test]
fn pre_zero_minor_must_match() {
    let v1 = Version::from_parts(0, 2, 0);
    let v2 = Version::from_parts(0, 2, 5);
    let v3 = Version::from_parts(0, 3, 0);
    assert!(v1.compatible(v2));
    assert!(!v1.compatible(v3));
}

#[test]
fn post_one_major_must_match() {
    let v1 = Version::from_parts(1, 0, 0);
    let v2 = Version::from_parts(1, 5, 0);
    let v3 = Version::from_parts(2, 0, 0);
    assert!(v1.compatible(v2));
    assert!(!v1.compatible(v3));
}

#[test]
fn zero_vs_one() {
    let v1 = Version::from_parts(0, 5, 0);
    let v2 = Version::from_parts(1, 0, 0);
    assert!(!v1.compatible(v2));
}

#[test]
fn version_display() {
    let v = Version::from_parts(1, 2, 3);
    assert_eq!(format!("{}", v), "1.2.3");
}
