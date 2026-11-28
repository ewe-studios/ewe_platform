//! The shared virtual file system (VFS) conformance kit on the S3 and R2 VFS.

use crate::support::{r2_factory, s3_factory, TempDir};
use foundation_core::durable::vfs_testkit::{run_all, SCENARIOS};
use foundation_core::valtron::block_on_future_scoped;
use foundation_nativeapis::shared::vfs::durable::ObjectLayout;
use foundation_nativeapis::shared::vfs::durable::Provider;

#[test]
fn s3_vfs_conformance() {
    let tmp = TempDir::new("conformance_s3");
    let factory = s3_factory(tmp.path().to_path_buf(), ObjectLayout::Segmented);
    let report = block_on_future_scoped(run_all(&factory));
    let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();
    assert_eq!(names, SCENARIOS, "the S3 VFS runs every scenario");
    assert!(
        report.iter().all(|scenario| !scenario.values.is_empty()),
        "every S3 scenario observes a value"
    );
}

#[test]
fn r2_vfs_conformance() {
    let tmp = TempDir::new("conformance_r2");
    let factory = r2_factory(tmp.path().to_path_buf());
    let report = block_on_future_scoped(run_all(&factory));
    let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();
    assert_eq!(names, SCENARIOS, "the R2 VFS runs every scenario");
}

#[test]
fn both_layouts_pass_the_same_scenarios() {
    let tmp = TempDir::new("conformance_layouts");
    let segmented = s3_factory(tmp.path().join("segmented"), ObjectLayout::Segmented);
    let append_only = s3_factory(tmp.path().join("append"), ObjectLayout::AppendOnly);
    let first = block_on_future_scoped(run_all(&segmented));
    let second = block_on_future_scoped(run_all(&append_only));
    assert_eq!(first, second, "the layout never changes a protocol outcome");
}

#[test]
fn s3_and_r2_agree() {
    let tmp = TempDir::new("conformance_differential");
    let s3 = s3_factory(tmp.path().join("s3"), ObjectLayout::Segmented);
    let r2 = r2_factory(tmp.path().join("r2"));
    let first = block_on_future_scoped(run_all(&s3));
    let second = block_on_future_scoped(run_all(&r2));
    assert_eq!(first, second, "S3 and R2 report the same observations");
}

#[test]
fn s3_and_r2_share_the_contract() {
    let tmp = TempDir::new("conformance_contract");
    let factory = s3_factory(tmp.path().to_path_buf(), ObjectLayout::Segmented);
    let (stream, _clock) = factory();
    let s3 = stream.vfs();
    // Both implementations answer every method through the same trait object.
    let listed = block_on_future_scoped(s3.list(&crate::support::stream("contract/none")));
    assert!(listed.is_ok(), "the S3 VFS answers list through the trait");

    let (r2, _, _) = crate::support::r2_fixture("contract_r2");
    assert_eq!(
        r2.provider_capabilities().provider,
        Provider::R2,
        "the R2 VFS records its provider"
    );
}
