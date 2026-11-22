//! Metrics tests (spec 82 feature 01, T6).

mod support;

use std::sync::Arc;

use foundation_registrar::{
    AppMetrics, CgroupMetrics, CompositeMetrics, DockerMetrics, MetricsSource, RunHandle,
};
use support::{container, FakeContainerApi};

fn process(unit: &str) -> RunHandle {
    RunHandle::Process {
        pid: 1,
        unit: unit.to_string(),
    }
}

#[test]
fn cgroup_files_give_cpu_memory_and_the_limit() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let unit = dir.path().join("ewe-agent-rpc.service");
    std::fs::create_dir_all(&unit).expect("unit dir");
    std::fs::write(unit.join("cpu.stat"), "usage_usec 1234\nuser_usec 1000\n").expect("cpu");
    std::fs::write(unit.join("memory.current"), "4096\n").expect("current");
    std::fs::write(unit.join("memory.max"), "8192\n").expect("max");
    let source = CgroupMetrics::new(dir.path());
    let metrics = futures_lite::future::block_on(source.read(&process("ewe-agent-rpc.service")))
        .expect("read");
    assert_eq!(
        metrics,
        AppMetrics {
            cpu_usage_micros: Some(1234),
            memory_bytes: Some(4096),
            memory_limit_bytes: Some(8192),
        }
    );
}

#[test]
fn an_unreadable_metric_is_left_out() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let unit = dir.path().join("ewe-agent-rpc.service");
    std::fs::create_dir_all(&unit).expect("unit dir");
    // No cpu.stat, and a `max` limit means "no limit".
    std::fs::write(unit.join("memory.current"), "4096\n").expect("current");
    std::fs::write(unit.join("memory.max"), "max\n").expect("max");
    let source = CgroupMetrics::new(dir.path());
    let metrics = futures_lite::future::block_on(source.read(&process("ewe-agent-rpc.service")))
        .expect("read");
    assert_eq!(metrics.cpu_usage_micros, None);
    assert_eq!(metrics.memory_bytes, Some(4096));
    assert_eq!(metrics.memory_limit_bytes, None);
}

#[test]
fn a_whole_missing_unit_reads_as_no_metrics() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let source = CgroupMetrics::new(dir.path());
    let metrics =
        futures_lite::future::block_on(source.read(&process("ewe-missing.service"))).expect("read");
    assert_eq!(metrics, AppMetrics::default());
}

#[test]
fn docker_stats_convert_nanoseconds_to_microseconds() {
    let api = Arc::new(FakeContainerApi::new());
    api.set_stats(serde_json::json!({
        "cpu_stats": { "cpu_usage": { "total_usage": 5_000_000 } },
        "memory_stats": { "usage": 100, "limit": 200 }
    }));
    let source = DockerMetrics::new(api);
    let metrics = futures_lite::future::block_on(source.read(&container("abc"))).expect("read");
    assert_eq!(
        metrics,
        AppMetrics {
            cpu_usage_micros: Some(5_000),
            memory_bytes: Some(100),
            memory_limit_bytes: Some(200),
        }
    );
}

#[test]
fn unreadable_docker_stats_leave_the_metrics_out() {
    let api = Arc::new(FakeContainerApi::new());
    let source = DockerMetrics::new(api);
    let metrics = futures_lite::future::block_on(source.read(&container("abc"))).expect("read");
    assert_eq!(metrics, AppMetrics::default());
}

#[test]
fn the_composite_picks_the_source_for_the_handle() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let unit = dir.path().join("ewe-app.service");
    std::fs::create_dir_all(&unit).expect("unit dir");
    std::fs::write(unit.join("memory.current"), "11\n").expect("current");
    let api = Arc::new(FakeContainerApi::new());
    api.set_stats(serde_json::json!({ "memory_stats": { "usage": 22 } }));
    let composite = CompositeMetrics::new(
        Some(Arc::new(CgroupMetrics::new(dir.path()))),
        Some(Arc::new(DockerMetrics::new(api))),
    );
    let from_process = futures_lite::future::block_on(composite.read(&process("ewe-app.service")))
        .expect("process");
    assert_eq!(from_process.memory_bytes, Some(11));
    let from_container =
        futures_lite::future::block_on(composite.read(&container("abc"))).expect("container");
    assert_eq!(from_container.memory_bytes, Some(22));
}
