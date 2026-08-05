//! F01 macro tests (unix): `daemons!` builds defs; `#[daemon_process]` boots a
//! group around a fn body.

use foundation_core::valtron::valtron_test;
use foundation_macros::{daemon_process, daemons};
use foundation_nativeapis::daemon::{DaemonGroup, DaemonStatus, ReadinessConfig};

#[test]
fn daemons_macro_builds_defs() {
    let defs = daemons! {
        { name = "db", run = ["sleep", "30"], readiness_port = 5432 },
        { name = "api", run = ["sleep", "30"], depends = ["db"],
          readiness_delay = 2, env = [("LOG", "info")] },
    };
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].name, "db");
    assert!(matches!(defs[0].readiness, ReadinessConfig::Port(5432)));
    assert_eq!(defs[1].depends, vec!["db".to_string()]);
    assert!(matches!(defs[1].readiness, ReadinessConfig::Delay(_)));
    assert_eq!(defs[1].env.get("LOG").map(String::as_str), Some("info"));
}

#[daemon_process({ name = "svc", run = ["sleep", "30"] })]
#[valtron_test]
fn daemon_process_boots_group(group: &DaemonGroup) {
    let handle = group.daemon("svc").expect("svc handle");
    assert_eq!(handle.status(), DaemonStatus::Ready);
    assert!(handle.pid().is_some());
    // Group drops at end of body -> daemon stopped.
}
