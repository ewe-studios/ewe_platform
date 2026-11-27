//! The tests of the agent-rpc container (spec 83 feature 06, test cases 4 and
//! 5).
//!
//! The tests run on a native host. A fake container control returns a handle
//! over a fake container, so no Cloudflare account is necessary.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_agent_rpc::cloudflare::container::{
    AgentContainer, ContainerOptions, AGENT_RPC_PORT,
};
use foundation_context::Ctx;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::container::{
    ContainerCapabilities, ContainerControl, ContainerHandle, ContainerId, ContainerInfo,
    ContainerOps, ContainerResources, ContainerSpec, ContainerStatus, PortRequest, PortResponse,
};
use foundation_sandbox::shared::driver::SubstrateExit;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::handles::ExecHandle;
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::spec::NetworkMode;
use foundation_sandbox::shared::{BoxByteStream, BoxFuture};
use futures::StreamExt;

/// The fake container of one test.
struct FakeOps {
    capabilities: ContainerCapabilities,
    fetches: Mutex<Vec<(u16, PortRequest)>>,
    execs: AtomicUsize,
}

impl FakeOps {
    fn new(capabilities: ContainerCapabilities) -> Self {
        Self {
            capabilities,
            fetches: Mutex::new(Vec::new()),
            execs: AtomicUsize::new(0),
        }
    }

    fn fetches(&self) -> Vec<(u16, PortRequest)> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn execs(&self) -> usize {
        self.execs.load(Ordering::SeqCst)
    }
}

impl ContainerOps for FakeOps {
    fn info(&self) -> ContainerInfo {
        ContainerInfo {
            id: ContainerId::new("agent-rpc".to_string()).expect("a valid container id"),
            driver: DriverId::CloudflareContainer,
            image: "agent-rpc:test".to_string(),
            status: ContainerStatus::Running,
            created_unix_ms: 0,
            resources: ContainerResources::default(),
        }
    }

    fn capabilities(&self) -> ContainerCapabilities {
        self.capabilities
    }

    fn fetch(&self, port: u16, request: PortRequest) -> BoxFuture<'_, SandboxResult<PortResponse>> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((port, request));
        Box::pin(async {
            let chunks: Vec<SandboxResult<Vec<u8>>> =
                vec![Ok(b"hello, ".to_vec()), Ok(b"agent".to_vec())];
            let body: BoxByteStream = Box::pin(futures::stream::iter(chunks));
            Ok(PortResponse {
                status: 200,
                headers: vec![("content-type".to_string(), "application/proto".to_string())],
                body,
            })
        })
    }

    fn exec(&self, _command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        self.execs.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(SandboxError::UnsupportedCapability {
                capability: "exec".to_string(),
            }
            .into())
        })
    }

    fn signal(&self, _signo: i32) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
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
                forced: false,
            })
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn log_tail(&self, _max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// A fake container control that returns a handle over [`FakeOps`].
struct FakeControl {
    ops: Arc<FakeOps>,
    specs: Mutex<Vec<ContainerSpec>>,
}

impl FakeControl {
    fn new(ops: Arc<FakeOps>) -> Self {
        Self {
            ops,
            specs: Mutex::new(Vec::new()),
        }
    }

    fn specs(&self) -> Vec<ContainerSpec> {
        self.specs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ContainerControl for FakeControl {
    fn start(
        &self,
        _ctx: Ctx,
        spec: ContainerSpec,
    ) -> BoxFuture<'_, SandboxResult<ContainerHandle>> {
        self.specs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(spec);
        let id = ContainerId::new("agent-rpc".to_string()).expect("a valid container id");
        let ops: Arc<dyn ContainerOps> = self.ops.clone();
        Box::pin(async move { Ok(ContainerHandle::new(id, ops)) })
    }

    fn get(
        &self,
        _ctx: Ctx,
        _id: &ContainerId,
    ) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>> {
        Box::pin(async { Ok(None) })
    }

    fn list(&self, _ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

fn capabilities(exec: bool) -> ContainerCapabilities {
    ContainerCapabilities {
        exec,
        port_fetch: true,
        signal: true,
        logs: false,
    }
}

#[test]
fn cloudflare_container_call_reaches_fetch_and_streams_the_body() {
    // Oracle: test case 4. `start` uses the control, `call` reaches
    // `ContainerHandle::fetch`, and the response body yields its chunks in
    // order.
    futures_lite::future::block_on(async {
        let ops = Arc::new(FakeOps::new(capabilities(true)));
        let control = FakeControl::new(Arc::clone(&ops));
        let container = AgentContainer::start_with(
            &control,
            Ctx::host(),
            "agent-rpc:test".to_string(),
            ContainerOptions::default(),
        )
        .await
        .expect("the fake control starts the container");

        let specs = control.specs();
        assert_eq!(specs.len(), 1, "the control starts one container");
        assert_eq!(specs[0].image, "agent-rpc:test");
        assert_eq!(specs[0].network, NetworkMode::Host);

        let request = PortRequest {
            method: "POST".to_string(),
            path: "/ewe.agent_rpc.v1.AgentService/ChatStream".to_string(),
            headers: vec![("content-type".to_string(), "application/proto".to_string())],
            body: b"prompt".to_vec(),
        };
        let response = container
            .call(AGENT_RPC_PORT, request.clone())
            .await
            .expect("the port answers");

        assert_eq!(response.status, 200);
        let fetches = ops.fetches();
        assert_eq!(fetches.len(), 1, "the call reaches the handle once");
        assert_eq!(fetches[0].0, AGENT_RPC_PORT);
        assert_eq!(fetches[0].1, request);

        let mut chunks = Vec::new();
        let mut body = response.body;
        while let Some(chunk) = body.next().await {
            chunks.extend(chunk.expect("a body chunk"));
        }
        assert_eq!(chunks, b"hello, agent", "the body streams its chunks");
    });
}

#[test]
fn cloudflare_container_without_exec_capability_refuses_exec() {
    // Oracle: test case 5. A handle with `exec == false` answers
    // `UnsupportedCapability { capability: "exec" }`, and no exec call reaches
    // the container.
    futures_lite::future::block_on(async {
        let ops = Arc::new(FakeOps::new(capabilities(false)));
        let control = FakeControl::new(Arc::clone(&ops));
        let container = AgentContainer::start_with(
            &control,
            Ctx::host(),
            "agent-rpc:test".to_string(),
            ContainerOptions::default(),
        )
        .await
        .expect("the fake control starts the container");

        let Err(error) = container.exec(Command::new(["true"])).await else {
            panic!("the handle has no exec capability");
        };
        assert_eq!(
            error.current_context(),
            &SandboxError::UnsupportedCapability {
                capability: "exec".to_string(),
            }
        );
        assert_eq!(ops.execs(), 0, "no exec call reaches the container");
    });
}

#[test]
fn cloudflare_container_default_options_name_the_agent_entrypoint() {
    // Oracle: the default start keeps the image entry point and enables the
    // container network, so the model server may reach a remote provider.
    let options = ContainerOptions::default();
    let spec = options.spec("agent-rpc:test".to_string());
    assert!(
        spec.entrypoint.is_empty(),
        "the image default entry point starts agent-slave"
    );
    assert_eq!(spec.network, NetworkMode::Host);
    assert!(spec.env.is_empty(), "no secret is in the image");
}
