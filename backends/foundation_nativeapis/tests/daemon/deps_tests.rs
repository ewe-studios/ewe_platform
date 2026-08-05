//! F01 dependency-graph tests: topological ordering, cycle detection, id
//! resolution.

use foundation_nativeapis::daemon::{
    resolve_id, shutdown_order, startup_order, DaemonDef, DaemonId,
};
use tracing_test::traced_test;

fn def(name: &str, depends: &[&str]) -> DaemonDef {
    DaemonDef::new(name, vec!["true".into()]).depends(depends)
}

#[test]
#[traced_test]
fn linear_chain_produces_ordered_levels() {
    // c depends on b depends on a  =>  [[a], [b], [c]]
    let defs = vec![def("c", &["b"]), def("a", &[]), def("b", &["a"])];
    let levels = startup_order(&defs).expect("no cycle");
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], vec![DaemonId::in_default("a")]);
    assert_eq!(levels[1], vec![DaemonId::in_default("b")]);
    assert_eq!(levels[2], vec![DaemonId::in_default("c")]);
}

#[test]
#[traced_test]
fn independent_daemons_share_a_level() {
    let defs = vec![def("a", &[]), def("b", &[]), def("c", &["a", "b"])];
    let levels = startup_order(&defs).expect("no cycle");
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].len(), 2, "a and b start concurrently");
    assert_eq!(levels[1], vec![DaemonId::in_default("c")]);
}

#[test]
#[traced_test]
fn cycle_is_detected_with_members() {
    // a -> b -> a
    let defs = vec![def("a", &["b"]), def("b", &["a"])];
    let err = startup_order(&defs).expect_err("cycle");
    let names: Vec<String> = err.cycle.iter().map(|id| id.name.clone()).collect();
    assert!(names.contains(&"a".to_string()));
    assert!(names.contains(&"b".to_string()));
}

#[test]
#[traced_test]
fn unknown_dependency_is_ignored_for_ordering() {
    // `x` depends on an external (undefined) name; it must still be orderable.
    let defs = vec![def("x", &["not-defined-here"])];
    let levels = startup_order(&defs).expect("no cycle");
    assert_eq!(levels, vec![vec![DaemonId::in_default("x")]]);
}

#[test]
#[traced_test]
fn shutdown_order_reverses_startup() {
    let defs = vec![def("a", &[]), def("b", &["a"])];
    let up = startup_order(&defs).expect("ok");
    let down = shutdown_order(&up);
    assert_eq!(down[0], vec![DaemonId::in_default("b")], "dependents first");
    assert_eq!(down[1], vec![DaemonId::in_default("a")]);
}

#[test]
#[traced_test]
fn resolve_id_handles_bare_and_qualified() {
    let self_id = DaemonId::new("app", "api");
    assert_eq!(resolve_id(&self_id, "db"), DaemonId::new("app", "db"));
    assert_eq!(
        resolve_id(&self_id, "other/cache"),
        DaemonId::new("other", "cache")
    );
}
