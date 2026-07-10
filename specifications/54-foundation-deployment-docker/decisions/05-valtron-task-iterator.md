# 05 — Valtron TaskIterator Format

**Date:** 2026-07-10
**Status:** Resolved

## Decision

All Docker API operations are implemented as valtron `TaskIterator` chains — the same pattern
used by every provider in `foundation_deployment`. No `async fn`, no `Future`, no tokio.

## The TaskIterator pattern

Every Docker API operation returns `impl TaskIterator<Ready = Result<T, DockerApiError>, Pending = DockerPending, Spawner = BoxedSendExecutionAction>`:

```rust
// Pending states for Docker operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockerPending {
    #[default]
    Connecting,
    Sending,
    Waiting,
    Streaming,
    Processing,
}

// Every operation is a TaskIterator chain
fn list_containers<R: DnsResolver>(
    client: &SimpleHttpClient<R>,
    options: Option<&ListContainersOptions>,
) -> Result<
    impl TaskIterator<Ready = Result<Vec<ContainerSummary>, DockerApiError>, Pending = DockerPending, Spawner = BoxedSendExecutionAction>,
    DockerApiError,
> {
    let url = format!("http://docker/v{}/containers/json?{}", API_VERSION, query_string(options));
    client.get(&url)
        .build_send_request()?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, .. } => parse_json_body::<Vec<ContainerSummary>>(stream),
            RequestIntro::Failed(e) => Err(DockerApiError::Transport(e.to_string())),
        })
        .map_pending(|_| DockerPending::Connecting)
}
```

## Why TaskIterator over async fn

### 1. Platform consistency

Every other provider in the workspace uses `TaskIterator`:
- `foundation_deployment_cloudflare` — DNS operations
- `foundation_deployment_stripe` — Payment operations
- `foundation_deployment_aws` — CLI wrapping via `ShellExecutor`

Docker should follow the same pattern.

### 2. Progress reporting

`TaskIterator` allows intermediate progress states:
```
DockerPending::Connecting → DockerPending::Sending → DockerPending::Waiting → Ready(...)
```

With `async fn` you'd need a separate progress channel or polling mechanism.

### 3. Composition with Deployable

The `Deployable` trait requires `TaskIterator` return types:
```rust
impl Deployable for DockerContainer {
    fn deploy(&self, instance_id: usize, client: ProviderClient<...>) -> Result<
        impl TaskIterator<Ready = Result<ContainerDeployOutput, Self::Error>, Pending = Deploying, ...>,
        Self::Error,
    > {
        // Chain: create → start → wait for healthy
    }
}
```

### 4. No executor dependency

`TaskIterator` is an iterator — it works with any executor. `async fn` requires a specific
runtime (tokio, async-std, etc.). Valtron manages its own thread pool.

## Streaming operations

For streaming endpoints (logs, events, stats), the `TaskIterator` yields progress states
while the stream is consumed:

```rust
fn container_logs<R: DnsResolver>(
    client: &SimpleHttpClient<R>,
    container_id: &str,
    options: &LogsOptions,
) -> Result<impl TaskIterator<Ready = Result<Vec<LogOutput>, DockerApiError>, Pending = DockerPending, ...>, DockerApiError> {
    // Build request → send → read stream → decode log frames → collect
    // Each chunk read yields DockerPending::Streaming
    // Final result yields Ready(Ok(Vec<LogOutput>))
}
```

## Composing operations

Complex workflows chain multiple `TaskIterator` operations:

```rust
// Container start workflow
fn start_container_workflow(...) -> impl TaskIterator<...> {
    // 1. Create container
    create_container(client, options, config)?
    // 2. Start container
    .and_then(|response| start_container(client, &response.id)?)
    // 3. Wait for ready
    .and_then(|_| wait_container_healthy(client, &response.id)?)
}
```

## Deployable integration

Docker implements `Deployable` for container lifecycle:

```rust
pub struct DockerContainer {
    pub image: String,
    pub name: String,
    pub ports: Vec<PortMapping>,
    pub env: Vec<(String, String)>,
    // ...
}

impl Deployable for DockerContainer {
    const NAMESPACE: &'static str = "docker/containers";
    type DeployOutput = ContainerDeployOutput;
    type DestroyOutput = ();
    type Error = DockerError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(&self, instance_id: usize, client: ProviderClient<Self::Store, Self::Resolver>) -> Result<
        impl TaskIterator<Ready = Result<Self::DeployOutput, Self::Error>, Pending = Deploying, Spawner = BoxedSendExecutionAction>,
        Self::Error,
    > {
        let config = ContainerCreateBody {
            image: Some(self.image.clone()),
            cmd: None,
            env: Some(self.env.iter().map(|(k, v)| format!("{k}={v}")).collect()),
            ..Default::default()
        };

        // Chain: create → start → inspect → return deploy output
        self.update(&client, instance_id, &config, /* task iterator chain */)
    }

    fn destroy(&self, instance_id: usize, client: ProviderClient<...>) -> Result<
        impl TaskIterator<...>,
        Self::Error,
    > {
        // Read state store to get container ID → stop → remove
    }
}
```
