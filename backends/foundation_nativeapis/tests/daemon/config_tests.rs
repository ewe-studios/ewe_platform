//! F01 config tests: builder chain, `must_env`, TOML loading, layering, and
//! readiness spec validation.

use std::io::Write;

use foundation_nativeapis::daemon::{
    ConfigError, DaemonConfig, DaemonDef, ReadinessConfig, ReadinessSpec,
};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn builder_sets_all_fields() {
    let def = DaemonDef::new("api", vec!["./api".into(), "--serve".into()])
        .depends(&["db", "cache"])
        .readiness(ReadinessConfig::Port(3000))
        .env("LOG", "info")
        .cpu_limit(80)
        .memory_limit("500MB")
        .restart(false)
        .max_restarts(3)
        .stop_timeout(20)
        .boot_start();

    assert_eq!(def.name, "api");
    assert_eq!(def.run, vec!["./api".to_string(), "--serve".to_string()]);
    assert_eq!(def.depends, vec!["db".to_string(), "cache".to_string()]);
    assert!(matches!(def.readiness, ReadinessConfig::Port(3000)));
    assert_eq!(def.env.get("LOG").map(String::as_str), Some("info"));
    assert_eq!(def.cpu_limit, Some(80));
    assert_eq!(def.memory_limit.as_deref(), Some("500MB"));
    assert!(!def.restart);
    assert_eq!(def.max_restarts, Some(3));
    assert_eq!(def.stop_timeout, Some(20));
    assert!(def.boot_start);
}

#[test]
#[traced_test]
fn defaults_are_sane() {
    let def = DaemonDef::new("x", vec!["true".into()]);
    assert!(def.restart, "restart defaults on");
    assert!(matches!(def.readiness, ReadinessConfig::Immediate));
    assert!(def.depends.is_empty());
    assert!(!def.boot_start);
    assert_eq!(def.max_restarts, None);
}

#[test]
#[traced_test]
fn must_env_reads_present_var_and_panics_on_missing() {
    // SAFETY: single-threaded test; unique var name avoids cross-test races.
    unsafe { std::env::set_var("DAEMON_TEST_MUST_ENV", "secret-value") };
    let def = DaemonDef::new("x", vec!["true".into()]).must_env("DAEMON_TEST_MUST_ENV");
    assert_eq!(
        def.env.get("DAEMON_TEST_MUST_ENV").map(String::as_str),
        Some("secret-value")
    );

    let missing = std::panic::catch_unwind(|| {
        DaemonDef::new("x", vec!["true".into()]).must_env("DAEMON_TEST_ABSENT_VAR_XYZ")
    });
    assert!(missing.is_err(), "missing must_env must panic");
}

#[test]
#[traced_test]
fn readiness_spec_rejects_multiple_strategies() {
    let spec = ReadinessSpec {
        port: Some(5432),
        http: Some("http://x/".into()),
        ..ReadinessSpec::default()
    };
    assert!(matches!(
        spec.into_config(),
        Err(ConfigError::AmbiguousReadiness)
    ));
}

#[test]
#[traced_test]
fn readiness_spec_compiles_output_regex() {
    let spec = ReadinessSpec {
        output: Some(r"listening on \d+".into()),
        ..ReadinessSpec::default()
    };
    assert!(matches!(spec.into_config(), Ok(ReadinessConfig::Output(_))));

    let bad = ReadinessSpec {
        output: Some("(".into()),
        ..ReadinessSpec::default()
    };
    assert!(matches!(bad.into_config(), Err(ConfigError::InvalidRegex(_))));
}

#[test]
#[traced_test]
fn empty_spec_is_immediate() {
    let spec = ReadinessSpec::default();
    assert!(matches!(spec.into_config(), Ok(ReadinessConfig::Immediate)));
}

#[test]
#[traced_test]
fn toml_loads_and_converts_to_defs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut f = std::fs::File::create(dir.path().join("daemon.toml")).expect("create");
    write!(
        f,
        r#"
namespace = "myapp"
root = true

[daemons.db]
run = ["postgres", "-D", "/data"]
readiness = {{ port = 5432 }}

[daemons.api]
run = ["./api"]
depends = ["db"]
readiness = {{ http = "http://localhost:3000/health" }}
memory_limit = "500MB"
"#
    )
    .expect("write");

    let config = DaemonConfig::load_from_path(dir.path()).expect("load");
    assert_eq!(config.namespace.as_deref(), Some("myapp"));

    let defs = config.into_daemon_defs().expect("convert");
    assert_eq!(defs.len(), 2);
    let api = defs.iter().find(|d| d.name == "api").expect("api");
    assert_eq!(api.depends, vec!["db".to_string()]);
    assert!(matches!(api.readiness, ReadinessConfig::Http(_)));
    assert_eq!(api.memory_limit.as_deref(), Some("500MB"));
    let db = defs.iter().find(|d| d.name == "db").expect("db");
    assert!(matches!(db.readiness, ReadinessConfig::Port(5432)));
}

#[test]
#[traced_test]
fn toml_layering_nearer_file_wins_and_root_stops_recursion() {
    // outer/  (root=true, defines `shared` with restart=true)
    // outer/inner/ (defines `shared` with restart=false, overrides)
    let outer = tempfile::tempdir().expect("tempdir");
    let inner = outer.path().join("inner");
    std::fs::create_dir(&inner).expect("mkdir");

    std::fs::write(
        outer.path().join("daemon.toml"),
        "root = true\n[daemons.shared]\nrun = [\"a\"]\nrestart = true\n[daemons.outeronly]\nrun = [\"o\"]\n",
    )
    .expect("write outer");
    std::fs::write(
        inner.join("daemon.toml"),
        "[daemons.shared]\nrun = [\"b\"]\nrestart = false\n",
    )
    .expect("write inner");

    let config = DaemonConfig::load_from_path(&inner).expect("load");
    let defs = config.into_daemon_defs().expect("convert");

    let shared = defs.iter().find(|d| d.name == "shared").expect("shared");
    assert_eq!(shared.run, vec!["b".to_string()], "nearer file wins");
    assert!(!shared.restart, "nearer override applied");
    assert!(
        defs.iter().any(|d| d.name == "outeronly"),
        "outer-only daemon still present via merge"
    );
}
