//! The run layer: an idempotent second run, an upgrade, a rollback, the lock
//! and the plan.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_core::valtron::valtron_test;
use foundation_installer::{
    Artifact, DeployTargetAdapter, Deployed, EntryOutcome, InstallError, InstallRecord, Installer,
    InstallerLock, Manifest, PlannedStep, SourceAdapter, SourceSpec,
};
use support::{
    deploy_none, entry, fake_context, test_config, FakeCommands, FakeDocker, FakeHttpClient,
};

struct FakeSource;

#[async_trait]
impl SourceAdapter for FakeSource {
    fn kind(&self) -> &'static str {
        "url"
    }

    async fn resolve(
        &self,
        entry: &foundation_installer::Entry,
        _ctx: &foundation_installer::InstallContext,
    ) -> Result<Artifact, InstallError> {
        if entry.version == "bad" {
            return Err(InstallError::Resolve {
                entry: entry.name.to_string(),
                reason: "the fake source refuses this version".to_string(),
            });
        }
        Ok(Artifact::File {
            path: PathBuf::from("/fake/artifact"),
            sha256: "unused".to_string(),
        })
    }
}

struct FakeTarget {
    fail_version: Option<String>,
}

#[async_trait]
impl DeployTargetAdapter for FakeTarget {
    fn kind(&self) -> &'static str {
        "none"
    }

    async fn apply(
        &self,
        entry: &foundation_installer::Entry,
        artifact: &Artifact,
        _previous: Option<&Deployed>,
        _ctx: &foundation_installer::InstallContext,
    ) -> Result<Deployed, InstallError> {
        if self.fail_version.as_deref() == Some(entry.version.as_str()) {
            return Err(InstallError::Apply {
                entry: entry.name.to_string(),
                reason: "the fake target refuses this version".to_string(),
            });
        }
        Ok(Deployed::Installed {
            artifact: artifact.clone(),
        })
    }

    async fn exists(
        &self,
        _entry: &foundation_installer::Entry,
        _deployed: &Deployed,
        _ctx: &foundation_installer::InstallContext,
    ) -> Result<bool, InstallError> {
        Ok(true)
    }
}

fn item(version: &str) -> foundation_installer::Entry {
    entry(
        "agent-rpc",
        version,
        SourceSpec::Url {
            url: "https://example.com/agent-rpc".to_string(),
            sha256: "0".repeat(64),
        },
        deploy_none(),
    )
}

fn manifest(entries: Vec<foundation_installer::Entry>) -> Manifest {
    Manifest {
        manifest_version: 1,
        entries,
    }
}

fn installer(
    config: &foundation_installer::InstallerConfig,
    fail_version: Option<&str>,
) -> Installer {
    Installer::with_defaults(config.clone())
        .with_source(Arc::new(FakeSource))
        .with_target(Arc::new(FakeTarget {
            fail_version: fail_version.map(str::to_string),
        }))
}

fn context(config: &foundation_installer::InstallerConfig) -> foundation_installer::InstallContext {
    fake_context(
        config.clone(),
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    )
}

#[valtron_test]
async fn the_second_run_changes_nothing() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    let installer = installer(&config, None);
    let manifest = manifest(vec![item("1.0.0")]);

    let first = installer.run(&manifest, &ctx).await.expect("the first run");
    assert!(matches!(first.entries[0], EntryOutcome::Applied { .. }));
    let second = installer
        .run(&manifest, &ctx)
        .await
        .expect("the second run");
    assert_eq!(
        second.entries[0],
        EntryOutcome::Unchanged {
            name: item("1.0.0").name,
            version: "1.0.0".to_string()
        }
    );

    let record = InstallRecord::load(&config.record_path()).expect("the record loads");
    assert_eq!(
        record
            .get(&item("1.0.0").name)
            .expect("the entry is recorded")
            .version,
        "1.0.0"
    );
}

#[valtron_test]
async fn an_upgrade_replaces_only_the_changed_entry() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    let installer = installer(&config, None);

    installer
        .run(&manifest(vec![item("1.0.0")]), &ctx)
        .await
        .expect("the install run");
    let upgrade = installer
        .run(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the upgrade run");
    assert!(matches!(upgrade.entries[0], EntryOutcome::Applied { .. }));

    let record = InstallRecord::load(&config.record_path()).expect("the record loads");
    assert_eq!(
        record
            .get(&item("2.0.0").name)
            .expect("the entry is recorded")
            .version,
        "2.0.0"
    );
}

#[valtron_test]
async fn a_failed_upgrade_keeps_the_old_version() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    installer(&config, None)
        .run(&manifest(vec![item("1.0.0")]), &ctx)
        .await
        .expect("the install run");

    let failing = installer(&config, Some("2.0.0"));
    let report = failing
        .run(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the upgrade run reports the entry");
    match &report.entries[0] {
        EntryOutcome::Failed { kept, error, .. } => {
            assert_eq!(kept.as_deref(), Some("1.0.0"));
            assert!(matches!(error, InstallError::Apply { .. }));
        }
        other => panic!("expected a failed upgrade, got {other:?}"),
    }
    assert!(report.has_failures());

    let record = InstallRecord::load(&config.record_path()).expect("the record loads");
    assert_eq!(
        record
            .get(&item("1.0.0").name)
            .expect("the entry is recorded")
            .version,
        "1.0.0"
    );

    let steps = failing
        .plan(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the plan");
    assert_eq!(
        steps[0],
        PlannedStep::Upgrade {
            name: item("2.0.0").name,
            from: "1.0.0".to_string(),
            to: "2.0.0".to_string()
        }
    );
}

#[valtron_test]
async fn one_failed_entry_does_not_stop_the_others() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    let installer = installer(&config, None);
    let bad = entry(
        "broken",
        "bad",
        SourceSpec::Url {
            url: "https://example.com/broken".to_string(),
            sha256: "0".repeat(64),
        },
        deploy_none(),
    );

    let report = installer
        .run(&manifest(vec![item("1.0.0"), bad]), &ctx)
        .await
        .expect("the run reports every entry");
    assert_eq!(report.entries.len(), 2);
    assert!(matches!(report.entries[0], EntryOutcome::Applied { .. }));
    assert!(matches!(report.entries[1], EntryOutcome::Failed { .. }));
    assert!(report.has_failures());
}

#[valtron_test]
async fn the_plan_reports_install_upgrade_and_unchanged() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    let installer = installer(&config, None);

    let plan = installer
        .plan(&manifest(vec![item("1.0.0")]), &ctx)
        .await
        .expect("the plan");
    assert_eq!(
        plan[0],
        PlannedStep::Install {
            name: item("1.0.0").name,
            version: "1.0.0".to_string()
        }
    );

    installer
        .run(&manifest(vec![item("1.0.0")]), &ctx)
        .await
        .expect("the install run");
    let plan = installer
        .plan(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the plan");
    assert_eq!(
        plan[0],
        PlannedStep::Upgrade {
            name: item("2.0.0").name,
            from: "1.0.0".to_string(),
            to: "2.0.0".to_string()
        }
    );

    installer
        .run(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the upgrade run");
    let plan = installer
        .plan(&manifest(vec![item("2.0.0")]), &ctx)
        .await
        .expect("the plan");
    assert_eq!(
        plan[0],
        PlannedStep::Unchanged {
            name: item("2.0.0").name
        }
    );
}

#[valtron_test]
async fn the_lock_refuses_a_second_holder() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let ctx = context(&config);
    let _held = InstallerLock::acquire(&config, &ctx)
        .await
        .expect("the first lock");
    let error = InstallerLock::acquire(&config, &ctx)
        .await
        .expect_err("the second lock is refused");
    assert!(matches!(error, InstallError::Locked(_)));
}

#[test]
fn the_record_round_trips() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("installed.json");
    let item = item("1.0.0");
    let mut record = InstallRecord::default();
    record.insert(
        &item.name,
        foundation_installer::RecordedEntry {
            version: "1.0.0".to_string(),
            artifact: Artifact::File {
                path: PathBuf::from("/fake"),
                sha256: "unused".to_string(),
            },
            deployed: Deployed::Installed {
                artifact: Artifact::Tool {
                    program: "rg".to_string(),
                    path: PathBuf::from("/usr/bin/rg"),
                },
            },
            installed_at: 42,
        },
    );
    record.save(&path).expect("the record saves");
    let loaded = InstallRecord::load(&path).expect("the record loads");
    assert_eq!(loaded, record);
}
