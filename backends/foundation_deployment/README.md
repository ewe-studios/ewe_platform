# foundation_deployment

Trait-based, multi-provider infrastructure deployment. You describe infrastructure
as ordinary Rust types; a type that implements [`Deployable`] can be **deployed**
and **destroyed** uniformly — no YAML, no TOML, no DSL.

- [The `Deployable` contract](#the-deployable-contract)
- [Why boxed futures (and not `async_trait` or RPITIT)](#why-boxed-futures)
- [Writing a provider](#writing-a-provider)
- [State persistence](#state-persistence)
- [Unique underlying mechanics — the Docker case](#unique-underlying-mechanics)
- [Driving a deploy](#driving-a-deploy)

## The `Deployable` contract

```rust
pub trait Deployable {
    const NAMESPACE: &'static str;              // e.g. "docker/containers"

    type DeployOutput:  Send + Sync;            // URLs, IDs, artifacts
    type DestroyOutput: Send + Sync;            // often ()
    type Error:    std::error::Error + Send + Sync + std::fmt::Debug;
    type Store:    StateStore + Send + Sync + 'static;
    type Resolver: DnsResolver + Clone + 'static;

    fn deploy(&self, instance_id: usize, client: ProviderClient<Self::Store, Self::Resolver>)
        -> BoxFuture<'static, Result<Self::DeployOutput, Self::Error>>;

    fn destroy(&self, instance_id: usize, client: ProviderClient<Self::Store, Self::Resolver>)
        -> BoxFuture<'static, Result<Self::DestroyOutput, Self::Error>>;

    // provided:
    fn store(&self, client: &ProviderClient<Self::Store, Self::Resolver>)
        -> NamespacedStore<Self::Store> { /* namespaced by NAMESPACE */ }
}
```

`deploy` and `destroy` are **async**. Each returns a

```rust
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
```

which the caller `.await`s (or drives on valtron via `from_future`). `instance_id`
lets one `Deployable` describe many instances of the same resource; `client`
carries the state store and (for HTTP-first providers) an API client.

## Why boxed futures

`deploy`/`destroy` return a **boxed, `Send` future** rather than `async fn` in
the trait, an `#[async_trait]` macro, or return-position `impl Trait` (RPITIT).
That is a deliberate trade:

| Approach | Object-safe (`dyn Deployable`) | `Send` guarantee | Cost |
|----------|:---:|:---:|------|
| `#[async_trait]` | ✅ | ✅ (boxes) | proc-macro dep + one alloc/call |
| RPITIT `-> impl Future + Send` | ❌ | inferred (fragile) | zero-alloc |
| **`-> BoxFuture` (this crate)** | ✅ | ✅ (explicit) | one alloc/call |

For deploy/destroy — coarse-grained, I/O-bound, called rarely — the single
`Box::pin` allocation is nanoseconds against a millisecond-scale network call, so
the cost is irrelevant. In exchange we get:

- **Object safety.** `Vec<Box<dyn Deployable<…>>>` works, so an orchestrator can
  hold and deploy a heterogeneous set of resources. RPITIT would forbid this.
- **`Send` stated in the type**, not inferred — no per-impl "the future isn't
  `Send`" surprises (the fragility that bites RPITIT/`async fn`-in-trait).
- **A nameable type** you can store in fields, `Vec`s, and channels.

Any valtron task or stream can be adapted into a `BoxFuture`, so this is the one
async currency the deployment layer speaks — no macro, no unstable features.

## Writing a provider

Write the real work as an `async move` block and `Box::pin` it:

```rust
use foundation_deployment::{BoxFuture, Deployable};
use foundation_deployment::provider_client::ProviderClient;
use foundation_db::core::state::FileStateStore;
use foundation_netio::shared::client::dns::SystemDnsResolver;

struct Worker { name: String }

impl Deployable for Worker {
    const NAMESPACE: &'static str = "cloudflare/workers/script";
    type DeployOutput = String;
    type DestroyOutput = ();
    type Error = std::io::Error;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(&self, id: usize, client: ProviderClient<Self::Store, Self::Resolver>)
        -> BoxFuture<'static, Result<String, std::io::Error>>
    {
        let name = self.name.clone();
        let store = self.store(&client);          // namespaced state store
        Box::pin(async move {
            // ... call the provider's HTTP API via client.http_client ...
            store.store_typed(&id.to_string(), &name).ok();
            Ok(name)
        })
    }

    fn destroy(&self, id: usize, client: ProviderClient<Self::Store, Self::Resolver>)
        -> BoxFuture<'static, Result<(), std::io::Error>>
    {
        let store = self.store(&client);
        Box::pin(async move {
            let name: Option<String> = store.get_typed(&id.to_string()).ok().flatten();
            // ... tear down `name` ...
            store.remove(&id.to_string()).ok();
            Ok(())
        })
    }
}
```

## State persistence

`self.store(&client)` returns a [`NamespacedStore`] scoped to `NAMESPACE`, so
every key is isolated per-deployable. The pattern is:

- **`deploy`** records what it created (`store.store_typed(&id.to_string(), &output)`).
- **`destroy`** reads it back (`store.get_typed(&id.to_string())`) to know exactly
  what to tear down, then clears it (`store.remove(...)`).

`store_typed` / `get_typed` / `remove` are synchronous (they drive the store's
stream internally), so they compose fine inside the async block.

## Unique underlying mechanics

`ProviderClient` carries a TCP+DNS HTTP client (`http_client: SimpleHttpClient<R>`)
tuned for cloud REST APIs. **A provider whose transport is different is free to
use its own client** and treat `ProviderClient` purely as the state-persistence
handle. `deploy`/`destroy` are just async blocks — they can dial anything.

The reference case is **`foundation_deployment_docker`'s `ContainerDeployment`**.
Docker's Engine API is served over a **Unix socket** (`/var/run/docker.sock`),
which does not fit `ProviderClient`'s TCP+DNS client at all. So its impl:

```rust
fn deploy(&self, id, client) -> BoxFuture<'static, Result<ContainerDeployOutput, DockerError>> {
    let docker = DockerClient::connect_unix(&self.socket_path); // its OWN client
    let store  = self.store(&client);                           // ProviderClient → state only
    Box::pin(async move {
        let created = docker.create_container(&body, name.as_deref()).await?;
        docker.start_container(&created.id).await?;
        let output = ContainerDeployOutput { container_id: created.id };
        store.store_typed(&id.to_string(), &output)?;           // persist for destroy
        Ok(output)
    })
}
```

`destroy` mirrors it: `get_typed` the container id, `stop` + `remove` it over the
same self-built Unix-socket client, then `remove` the state key.

This is the intended shape whenever the mechanics are unique: **own the
transport inside the future, use `ProviderClient` for state and namespacing.**
The `Deployable` surface stays uniform; how a provider talks to its backend is
entirely its business.

## Driving a deploy

Because `deploy`/`destroy` are plain futures, callers choose how to run them:

```rust
// Inside any async context (e.g. a #[valtron_test] or another future):
let output = deployment.deploy(0, client.clone()).await?;
// ...
deployment.destroy(0, client).await?;
```

```rust
// From sync code, on the valtron pool:
let task = foundation_core::valtron::from_future(deployment.deploy(0, client));
// drive `task` to completion via drive_iterator / run_until_next_state
```

Object safety means you can also fan out over a collection:

```rust
let resources: Vec<Box<dyn Deployable<Store = _, Resolver = _, DeployOutput = _, DestroyOutput = _, Error = _>>> = /* … */;
for r in &resources {
    r.deploy(0, client.clone()).await?;
}
```
