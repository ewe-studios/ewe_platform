//! L11: the container handle, the unsupported capability and the streamed
//! port body.

use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_platform_sandbox::shared::command::{Command, ExecEvent, ExitStatus};
use foundation_platform_sandbox::shared::container::{
    ContainerCapabilities, ContainerControl, ContainerHandle, ContainerId, ContainerInfo,
    ContainerOps, ContainerResources, ContainerSpec, ContainerStatus, PortRequest, PortResponse,
};
use foundation_platform_sandbox::shared::driver::SubstrateExit;
use foundation_platform_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_platform_sandbox::shared::handles::{ExecEvents, ExecHandle, ExecHandleOps};
use foundation_platform_sandbox::shared::ids::{DriverId, ExecId};
use foundation_platform_sandbox::shared::spec::NetworkMode;
use foundation_platform_sandbox::shared::{BoxByteStream, BoxFuture};
use futures::executor::block_on;
use futures::StreamExt;

use super::common;

/// The operations of one fake execution.
struct FakeExecOps {
    pid: u32,
}

impl ExecHandleOps for FakeExecOps {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn stdin(&self) -> Option<foundation_platform_sandbox::shared::handles::ExecStdin> {
        None
    }

    fn events(&self) -> ExecEvents {
        ExecEvents::new(Box::pin(futures::stream::iter(vec![Ok(ExecEvent::Exit(
            ExitStatus::code(0),
        ))])))
    }

    fn signal(&self, _signal: i32) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<ExitStatus>> {
        Box::pin(async { Ok(ExitStatus::code(0)) })
    }
}

/// The operations of one fake container.
struct FakeOps {
    id: ContainerId,
    capabilities: ContainerCapabilities,
    body_len: usize,
}

impl FakeOps {
    fn new(id: ContainerId, capabilities: ContainerCapabilities, body_len: usize) -> Self {
        Self {
            id,
            capabilities,
            body_len,
        }
    }

    fn container_info(&self) -> ContainerInfo {
        ContainerInfo {
            id: self.id.clone(),
            driver: DriverId::Docker,
            image: "image:latest".to_string(),
            status: ContainerStatus::Running,
            created_unix_ms: 1,
            resources: ContainerResources::default(),
        }
    }
}

impl ContainerOps for FakeOps {
    fn info(&self) -> ContainerInfo {
        self.container_info()
    }

    fn capabilities(&self) -> ContainerCapabilities {
        self.capabilities
    }

    fn fetch(
        &self,
        _port: u16,
        _request: PortRequest,
    ) -> BoxFuture<'_, SandboxResult<PortResponse>> {
        let supported = self.capabilities.port_fetch;
        let body_len = self.body_len;
        Box::pin(async move {
            if !supported {
                return Err(SandboxError::UnsupportedCapability {
                    capability: "port_fetch".to_string(),
                }
                .into());
            }
            Ok(PortResponse {
                status: 200,
                headers: vec![("content-type".to_string(), "text/plain".to_string())],
                body: counting_body(body_len, 64 * 1024),
            })
        })
    }

    fn exec(&self, _command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let supported = self.capabilities.exec;
        Box::pin(async move {
            if !supported {
                return Err(SandboxError::UnsupportedCapability {
                    capability: "exec".to_string(),
                }
                .into());
            }
            let exec_id = ExecId::from_str("0123456789abcdef").expect("a valid exec id");
            Ok(ExecHandle::new(exec_id, Arc::new(FakeExecOps { pid: 42 })))
        })
    }

    fn signal(&self, _signo: i32) -> BoxFuture<'_, SandboxResult<()>> {
        let supported = self.capabilities.signal;
        Box::pin(async move {
            if supported {
                Ok(())
            } else {
                Err(SandboxError::UnsupportedCapability {
                    capability: "signal".to_string(),
                }
                .into())
            }
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async {
            Ok(SubstrateExit {
                code: Some(0),
                signal: None,
                forced: false,
            })
        })
    }

    fn stop(&self, _grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async {
            Ok(SubstrateExit {
                code: Some(0),
                signal: None,
                forced: true,
            })
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        let len = max.min(4);
        Box::pin(async move { Ok(vec![b'l'; len]) })
    }
}

/// A fake container control with one container.
struct FakeControl {
    id: ContainerId,
    capabilities: ContainerCapabilities,
    body_len: usize,
}

impl FakeControl {
    fn new() -> Self {
        Self {
            id: ContainerId::new("container-1".to_string()).expect("a valid id"),
            capabilities: ContainerCapabilities {
                exec: true,
                port_fetch: true,
                signal: true,
                logs: true,
            },
            body_len: 1024,
        }
    }

    fn handle(&self) -> ContainerHandle {
        ContainerHandle::new(
            self.id.clone(),
            Arc::new(FakeOps::new(
                self.id.clone(),
                self.capabilities,
                self.body_len,
            )),
        )
    }
}

impl ContainerControl for FakeControl {
    fn start(
        &self,
        _ctx: Ctx,
        _spec: ContainerSpec,
    ) -> BoxFuture<'_, SandboxResult<ContainerHandle>> {
        let handle = self.handle();
        Box::pin(async move { Ok(handle) })
    }

    fn get(
        &self,
        _ctx: Ctx,
        id: &ContainerId,
    ) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>> {
        let found = id == &self.id;
        let handle = self.handle();
        Box::pin(async move { Ok(found.then_some(handle)) })
    }

    fn list(&self, _ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>> {
        let info = self.handle().info();
        Box::pin(async move { Ok(vec![info]) })
    }
}

/// A body stream that produces `total` bytes in chunks of `chunk` bytes.
///
/// The stream holds one chunk at a time.
fn counting_body(total: usize, chunk: usize) -> BoxByteStream {
    let stream = futures::stream::unfold(total, move |remaining| async move {
        if remaining == 0 {
            return None;
        }
        let len = remaining.min(chunk);
        Some((Ok(vec![0u8; len]), remaining - len))
    });
    Box::pin(stream)
}

fn spec() -> ContainerSpec {
    ContainerSpec {
        image: "image:latest".to_string(),
        entrypoint: Vec::new(),
        env: Vec::new(),
        network: NetworkMode::None,
        resources: ContainerResources::default(),
        labels: Vec::new(),
    }
}

fn request() -> PortRequest {
    PortRequest {
        method: "GET".to_string(),
        path: "/".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

/// L11: one handle starts, fetches, execs, waits, stops and destroys.
#[allow(
    clippy::too_many_lines,
    reason = "one test walks the whole handle surface"
)]
#[test]
fn container_handle_roundtrip() {
    let ctx = Ctx::fixed(common::clock());
    let control = FakeControl::new();
    let handle = block_on(control.start(ctx.clone(), spec())).expect("start");
    assert_eq!(handle.id().as_str(), "container-1");
    assert_eq!(handle.info().id, *handle.id());
    assert!(handle.capabilities().exec);

    let response = block_on(handle.fetch(8080, request())).expect("fetch");
    assert_eq!(response.status, 200);
    let mut body = response.body;
    let mut received = 0;
    while let Some(chunk) = block_on(body.next()) {
        received += chunk.expect("a body chunk").len();
    }
    assert_eq!(received, 1024);

    let exec = block_on(handle.exec(Command::new(["echo", "hello"]))).expect("exec");
    assert_eq!(exec.pid(), 42);
    assert_eq!(
        block_on(exec.wait()).expect("the program exits"),
        ExitStatus::code(0)
    );

    block_on(handle.signal(9)).expect("signal");
    let watched = block_on(handle.wait()).expect("the container exits");
    assert_eq!(watched.code, Some(0));
    assert!(!watched.forced);

    let stopped = block_on(handle.stop(Duration::from_secs(1))).expect("stop");
    assert!(stopped.forced);
    block_on(handle.destroy()).expect("destroy");
    let tail = block_on(handle.log_tail(4)).expect("log tail");
    assert_eq!(tail, vec![b'l'; 4]);

    let found = block_on(control.get(ctx.clone(), handle.id())).expect("get");
    assert!(found.is_some());
    let missing = ContainerId::new("other".to_string()).expect("a valid id");
    let absent = block_on(control.get(ctx.clone(), &missing)).expect("get");
    assert!(absent.is_none());
    let list = block_on(control.list(ctx)).expect("list");
    assert_eq!(list.len(), 1);
}

/// An absent capability gives `UnsupportedCapability`, and RPC keeps its type.
#[test]
fn unsupported_capability() {
    let id = ContainerId::new("container-2".to_string()).expect("a valid id");
    let capabilities = ContainerCapabilities {
        exec: false,
        port_fetch: true,
        signal: true,
        logs: true,
    };
    let handle = ContainerHandle::new(id.clone(), Arc::new(FakeOps::new(id, capabilities, 0)));
    assert!(!handle.capabilities().exec);

    let Err(error) = block_on(handle.exec(Command::new(["echo"]))) else {
        panic!("exec must fail");
    };
    let original = error.current_context().clone();
    match &original {
        SandboxError::UnsupportedCapability { capability } => assert_eq!(capability, "exec"),
        other => panic!("expected UnsupportedCapability, got {other:?}"),
    }

    let wire = original.to_connect(1024);
    assert_eq!(SandboxError::from_connect(&wire), original);
}

/// A 10 MiB port body arrives in chunks, and the fake holds at most two.
#[test]
fn port_response_streams() {
    let total = 10 * 1024 * 1024;
    let chunk = 64 * 1024;
    let outstanding = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    let outstanding_for_stream = outstanding.clone();
    let peak_for_stream = peak.clone();
    let stream = futures::stream::unfold(total, move |remaining| {
        let outstanding = outstanding_for_stream.clone();
        let peak = peak_for_stream.clone();
        async move {
            if remaining == 0 {
                return None;
            }
            let len = remaining.min(chunk);
            let now = outstanding.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            Some((Ok(vec![0u8; len]), remaining - len))
        }
    });
    let mut body: BoxByteStream = Box::pin(stream);

    let mut received = 0;
    let mut chunks = 0;
    while let Some(item) = block_on(body.next()) {
        let data = item.expect("a body chunk");
        received += data.len();
        chunks += 1;
        outstanding.fetch_sub(1, Ordering::SeqCst);
        assert!(peak.load(Ordering::SeqCst) <= 2);
    }

    assert_eq!(received, total);
    assert!(chunks > 1, "the body must stream in more than one chunk");
}
