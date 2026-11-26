//! The tests of the Cloudflare container control.

use std::sync::Arc;
use std::time::Duration;

use foundation_sandbox::shared::cloudflare::testing::{container_transport, slave_calls};
use foundation_sandbox::shared::cloudflare::transport::PortConnector;
use foundation_sandbox::shared::cloudflare::{CloudflareContainerControl, DoContainerClient};
use foundation_sandbox::shared::command::{Command, StdinMode};
use foundation_sandbox::shared::container::{
    ContainerControl, ContainerHandle, ContainerResources, ContainerSpec, PortRequest,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;
use foundation_sandbox::shared::spec::NetworkMode;

use super::support::{self, block_on, Harness};

fn connector(h: &Harness) -> Arc<dyn foundation_sandbox::shared::host::SlaveConnector> {
    Arc::new(PortConnector {
        container: h.container.clone(),
        config: Arc::clone(&h.host.config),
        do_slave: None,
    })
}

fn control(h: &Harness) -> Arc<CloudflareContainerControl> {
    Arc::new(CloudflareContainerControl::new(
        h.host.clone(),
        connector(h),
    ))
}

fn container_spec() -> ContainerSpec {
    ContainerSpec {
        image: "sandbox".to_string(),
        entrypoint: Vec::new(),
        env: vec![("A".to_string(), "B".to_string())],
        network: NetworkMode::None,
        resources: ContainerResources::default(),
        labels: Vec::new(),
    }
}

fn echo_script(h: &Harness) {
    h.scripted.with_exec_events(vec![
        ExecEventKind::Started { pid: 1 },
        ExecEventKind::Stdout {
            data: b"hi\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
}

#[test]
fn container_start_handle() {
    let h = Harness::new(40, true, false);
    let handle = block_on(control(&h).start(h.ctx.clone(), container_spec())).expect("start");
    let options = h.container.start_options().expect("started");
    assert_eq!(options.entrypoint, h.host.config.slave_entrypoint());
    assert!(!options.enable_internet);
    assert_eq!(options.env, vec![("A".to_string(), "B".to_string())]);
    assert_eq!(handle.id().as_str(), h.host.config.name);
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
}

#[test]
fn container_exec_via_slave() {
    let h = Harness::new(41, true, false);
    echo_script(&h);
    let handle = block_on(control(&h).start(h.ctx.clone(), container_spec())).expect("start");
    let exec = block_on(handle.exec(Command::new(["echo", "hi"]))).expect("exec");
    let output = block_on(foundation_sandbox::shared::control::run_capture(
        exec,
        1024 * 1024,
    ))
    .expect("run");
    assert_eq!(output.stdout, b"hi\n");
    assert_eq!(output.status.exit_code(), Some(0));
}

#[test]
fn container_no_exec_without_slave() {
    let h = Harness::new(42, true, false);
    let mut spec = container_spec();
    spec.entrypoint = vec!["/bin/sh".to_string()];
    let handle = block_on(control(&h).start(h.ctx.clone(), spec)).expect("start");
    assert!(!handle.capabilities().exec);
    let error = super::support::error_of(
        block_on(handle.exec(Command::new(["echo", "hi"]))),
        "no exec",
    );
    assert!(matches!(
        error,
        SandboxError::UnsupportedCapability { capability } if capability == "exec"
    ));
}

#[test]
fn container_capabilities() {
    let h = Harness::new(43, true, false);
    let handle = block_on(control(&h).start(h.ctx.clone(), container_spec())).expect("start");
    let capabilities = handle.capabilities();
    assert!(capabilities.exec);
    assert!(capabilities.port_fetch);
    assert!(capabilities.signal);
    assert!(!capabilities.logs);
}

#[test]
fn container_destroy_idempotent() {
    let h = Harness::new(44, true, false);
    let handle = block_on(control(&h).start(h.ctx.clone(), container_spec())).expect("start");
    block_on(handle.destroy()).expect("first destroy");
    block_on(handle.destroy()).expect("the second destroy succeeds");
    assert!(!h.container.is_running());
    let error = super::support::error_of(
        block_on(handle.fetch(
            8080,
            PortRequest {
                method: "GET".to_string(),
                path: "/".to_string(),
                headers: Vec::new(),
                body: Vec::new(),
            },
        )),
        "a destroyed container is gone",
    );
    assert!(matches!(error, SandboxError::NotFound { .. }));
    assert_eq!(
        handle.info().status,
        foundation_sandbox::shared::container::ContainerStatus::Stopped
    );
}

#[test]
fn container_fetch_streams() {
    let h = Harness::new(45, true, false);
    let chunks = 10;
    let body: foundation_sandbox::shared::BoxByteStream = Box::pin(futures::stream::iter(
        (0..chunks).map(|_| Ok(vec![7_u8; 1024 * 1024])),
    ));
    h.container
        .set_raw_response(foundation_sandbox::shared::cloudflare::PortResponse {
            status: 200,
            headers: vec![("x-test".to_string(), "1".to_string())],
            body,
        });
    let handle = block_on(control(&h).start(h.ctx.clone(), container_spec())).expect("start");
    let response = block_on(handle.fetch(
        8080,
        PortRequest {
            method: "GET".to_string(),
            path: "/large".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        },
    ))
    .expect("fetch");
    assert_eq!(response.status, 200);
    let mut seen_chunks = 0;
    let mut total = 0;
    let mut stream = response.body;
    while let Some(item) = block_on(futures::StreamExt::next(&mut stream)) {
        let bytes = item.expect("a chunk");
        seen_chunks += 1;
        total += bytes.len();
    }
    assert_eq!(total, chunks * 1024 * 1024);
    assert_eq!(seen_chunks, chunks);
}

#[test]
fn container_remote_roundtrip() {
    let h = Harness::new(46, true, false);
    echo_script(&h);
    let control = control(&h);
    let handle = block_on(control.start(h.ctx.clone(), container_spec())).expect("start");
    let calls = slave_calls(&h.ctx, &h.host, &support::limits()).expect("slave calls");
    let transport = container_transport(
        h.ctx.clone(),
        control.clone() as Arc<dyn ContainerControl>,
        support::limits(),
        calls,
        h.id(),
    );
    let client = DoContainerClient::new(h.ctx.clone(), transport, support::limits())
        .with_info(handle.info());
    let remote = ContainerHandle::new(handle.id().clone(), Arc::new(client));
    let exec = block_on(remote.exec(Command::new(["echo", "hi"]))).expect("exec");
    let output = block_on(foundation_sandbox::shared::control::run_capture(
        exec,
        1024 * 1024,
    ))
    .expect("run");
    assert_eq!(output.stdout, b"hi\n");
    assert_eq!(output.status.exit_code(), Some(0));
}

#[test]
fn container_remote_exec_input() {
    let h = Harness::new(70, true, false);
    h.scripted.with_gated_exec(
        vec![
            ExecEventKind::Started { pid: 1 },
            ExecEventKind::Stdout {
                data: b"first".to_vec(),
            },
        ],
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    );
    let control = control(&h);
    let handle = block_on(control.start(h.ctx.clone(), container_spec())).expect("start");
    let calls = slave_calls(&h.ctx, &h.host, &support::limits()).expect("slave calls");
    let transport = container_transport(
        h.ctx.clone(),
        control as Arc<dyn ContainerControl>,
        support::limits(),
        calls,
        h.id(),
    );
    let client = DoContainerClient::new(h.ctx.clone(), transport, support::limits())
        .with_info(handle.info());
    let remote = ContainerHandle::new(handle.id().clone(), Arc::new(client));
    let mut command = Command::new(["cat"]);
    command.stdin = StdinMode::Piped;
    let mut exec = block_on(remote.exec(command)).expect("exec");
    let mut stdin = exec.stdin().expect("the command reads input");
    block_on(stdin.write(b"hello")).expect("write the input");
    assert!(
        h.scripted
            .applied_input()
            .iter()
            .any(|input| input.data == b"hello"),
        "{:?}",
        h.scripted.applied_input()
    );
}

#[test]
fn container_remote_exec_signal() {
    let h = Harness::new(71, true, false);
    h.scripted.with_gated_exec(
        vec![
            ExecEventKind::Started { pid: 1 },
            ExecEventKind::Stdout {
                data: b"first".to_vec(),
            },
        ],
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    );
    let control = control(&h);
    let handle = block_on(control.start(h.ctx.clone(), container_spec())).expect("start");
    let calls = slave_calls(&h.ctx, &h.host, &support::limits()).expect("slave calls");
    let transport = container_transport(
        h.ctx.clone(),
        control as Arc<dyn ContainerControl>,
        support::limits(),
        calls,
        h.id(),
    );
    let client = DoContainerClient::new(h.ctx.clone(), transport, support::limits())
        .with_info(handle.info());
    let remote = ContainerHandle::new(handle.id().clone(), Arc::new(client));
    let exec = block_on(remote.exec(Command::new(["cat"]))).expect("exec");
    block_on(exec.signal(9)).expect("signal");
    assert_eq!(h.scripted.signals(), vec![9]);
}

#[test]
fn container_wait_deadline() {
    let h = Harness::new(47, true, false);
    let control = control(&h);
    let handle = block_on(control.start(h.ctx.clone(), container_spec())).expect("start");
    let calls = slave_calls(&h.ctx, &h.host, &support::limits()).expect("slave calls");
    let transport = container_transport(
        h.ctx.clone(),
        control as Arc<dyn ContainerControl>,
        support::limits(),
        calls,
        h.id(),
    );
    let client = DoContainerClient::new(h.ctx.clone(), transport, support::limits())
        .with_info(handle.info())
        .with_wait_timeout(Some(Duration::from_millis(100)));
    let remote = ContainerHandle::new(handle.id().clone(), Arc::new(client));
    let error = block_on(remote.wait()).expect_err("the wait has a deadline");
    assert!(matches!(
        error.current_context(),
        SandboxError::Rpc { code, .. } if code == "deadline_exceeded"
    ));
}
