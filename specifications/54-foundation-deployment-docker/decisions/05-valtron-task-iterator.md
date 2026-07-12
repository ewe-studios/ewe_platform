# 05 — Valtron TaskIterator Format

**Date:** 2026-07-10
**Updated:** 2026-07-12 (F51 alignment — Feature 01)
**Status:** Resolved

## Decision

Generated deployment functions should use `async fn` going forward. Valtron already
has first-class support for futures via `from_future()`, `run_future()`, and
`#[valtron]` macros — an `async fn` is the most ergonomic way to write sequential
HTTP request logic.

The generator should emit `async fn` that calls `HttpClient::send_async()` /
`send_sse_async()` directly, rather than producing `TaskIterator` chains. This is:

1. **Simpler to generate** — sequential code, no combinator chains
2. **More ergonomic** — callers use `.await`, not `.map_ready().map_pending()`
3. **Already supported** — valtron drives futures via `from_future()`, `run_future()`
4. **Better for streaming** — `Stream` (futures_core) is the natural streaming primitive

## Two patterns: non-streaming vs. streaming

### Non-streaming: async fn + `send_async()`

For single-response endpoints, call `HttpClient::send_async()`:

```rust
async fn create_container(
    client: DynNetClient,
    config: &ContainerCreateBody,
    args: &CreateContainerArgs,
) -> Result<ApiResponse<ContainerCreateResponse>, DockerApiError> {
    let endpoint_url = format!("http://docker/v{}/containers/create", API_VERSION);
    let req = PreparedRequestBuilder::post(&endpoint_url)?
        .query("name", args.name.as_deref())
        .body_json(config)
        .map_err(|e| DockerApiError::RequestBuildFailed(e.to_string()))?
        .build();

    let response = client.send_async(req).await
        .map_err(|e| DockerApiError::Transport(e.to_string()))?;

    let status: u16 = response.status().into();
    let headers = response.headers().clone();
    let body: ContainerCreateResponse = response.json().await
        .map_err(|e| DockerApiError::JsonParse(e.to_string()))?;

    Ok(ApiResponse { status, headers, body })
}
```

### Streaming: async fn + `open_exchange()` → split

For streaming endpoints, use `open_exchange()` and valtron's split pattern:

```rust
async fn container_logs(
    client: DynNetClient,
    container_id: &str,
    follow: bool,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
) {
    let endpoint_url = format!(
        "http://docker/v{}/containers/{}/logs", API_VERSION, container_id,
    );
    let req = PreparedRequestBuilder::get(&endpoint_url)?
        .query("stdout", Some("1"))
        .query("stderr", Some("1"))
        .query("follow", Some(if follow { "1" } else { "0" }))
        .build();

    let pump = client.open_exchange(req);
    let (head, body, cont) = body_reader::split_exchange_task(pump);

    // Send the continuation to valtron.
    valtron::send(cont.map_ready(|_| ()))?;

    (head, body)
}
```

## Why async fn

### 1. Better ergonomics

`async fn` is standard Rust. Callers use `.await`, not combinator chains:
```rust
let response = create_container(&client, &config, &args).await?;
```

### 2. Valtron already supports it

Valtron bridges futures into the engine via `from_future()`, `run_future()`, and
`#[valtron]` entry points:
```rust
#[valtron]
async fn main() {
    let client = HttpClientBuilder::new().build();
    let result = create_container(&client, &config, &args).await.unwrap();
}
```

### 3. Simpler code generation

The generator emits sequential code — no `TaskIterator` combinator chains, no
`impl TaskIterator<Ready = ..., Pending = ..., Spawner = ...>` type signatures.

### 4. Streaming via `Stream` (futures_core)

For streaming endpoints, the split observers convert to `Stream<Item = ...>` via
`into_next_stream()` — callers use `StreamExt::next().await`:

## Deployable integration

Docker implements `Deployable` using async functions. Valtron executes the async
fn as a `TaskIterator` internally via `from_future()`:

```rust
#[async_trait::async_trait]
impl Deployable for DockerContainer {
    const NAMESPACE: &'static str = "docker/containers";
    type DeployOutput = ContainerDeployOutput;
    type DestroyOutput = ();
    type Error = DockerError;
    type Store = FileStateStore;

    async fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store>,
    ) -> Result<Self::DeployOutput, Self::Error> {
        let config = ContainerCreateBody {
            image: Some(self.image.clone()),
            cmd: None,
            env: Some(self.env.iter().map(|(k, v)| format!("{k}={v}")).collect()),
            ..Default::default()
        };

        let response = create_container(&client.http, &config, &CreateContainerArgs::default()).await?;
        start_container(&client.http, &response.body.id).await?;
        wait_container_healthy(&client.http, &response.body.id).await?;

        Ok(ContainerDeployOutput {
            container_id: response.body.id,
            // ...
        })
    }

    async fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store>,
    ) -> Result<(), Self::Error> {
        let container_id = client.store.load_state(instance_id)?;
        stop_container(&client.http, &container_id).await?;
        remove_container(&client.http, &container_id).await?;
        Ok(())
    }
}
```

## Related decisions

- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP client pattern
- **[08 — Streaming responses](08-streaming-responses.md)** — Streaming endpoint handling
- **[Feature 03 — generator async fn codegen](../features/03-generator-async-fn-codegen/feature.md)** — Implementation of this decision (generator emit + regeneration)
- **[Feature 01 — DynNetClient + PreparedRequestBuilder surface](../features/01-dynnetclient-preparedrequest-surface/feature.md)** — The HTTP primitives this builds on
