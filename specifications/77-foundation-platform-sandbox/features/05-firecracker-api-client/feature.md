---
feature: "Firecracker API client"
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["00-protocol-and-api-types"]
crates: ["foundation_platform_sandbox"]
---

# Feature 05: The Firecracker API client

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); representational
state transfer (REST); Unix domain socket (UDS); Hypertext Transfer Protocol (HTTP);
JavaScript Object Notation (JSON); virtual machine monitor (VMM); continuous integration
(CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## 1. Goal

Give the crate typed request functions for the Firecracker REST API, generated from the
Firecracker Swagger file (root rule 5), and a small client on a UDS. This feature starts no
VM. Feature 06 uses it.

## 2. Measured facts

- The API file is `src/firecracker/swagger/firecracker.yaml` in the Firecracker repository,
  commit `778a26c`, API version `1.19.0-dev`. It is Swagger 2.0.
- The Docker client in this repository had the same input format. It converts Swagger 2.0
  to OpenAPI 3 with `backends/foundation_deployment_docker/specs/swagger2openapi.py`, then
  runs `gen_api`. See
  `specifications/completed/54-foundation-deployment-docker/decisions/03-type-replication-strategy.md`.
- Generated functions have this shape (from that decision file):
  `pub async fn <operation>_request<F>(client: DynNetClient, args: &<Operation>Args, builder_mod: Option<F>) -> Result<ApiResponse<T>, ApiError>`.
- `foundation_netio::HttpClientBuilder::new().unix_socket(path).build()` gives a
  `DynNetClient` for a UDS (`foundation_deployment_docker/src/client/mod.rs`, line 11).
- The generated code uses `ApiClient`, `ApiError`, `ApiResponse` and `BodyLimits` from
  `foundation_deployment::providers::common`. So this crate gets a dependency on
  `foundation_deployment`, behind the `firecracker` feature. Check the weight of that
  dependency with `cargo tree -p foundation_deployment -e normal --depth 1`. If it pulls
  cloud provider code that is not behind features, report it; do not copy the common types.

## 3. Work

### 3.1 Vendor the API file

1. Copy `firecracker.yaml` to `backends/foundation_platform_sandbox/specs/firecracker-v1.19.yaml`.
   Add a `specs/README.md` with the source URL, the commit and the date.
2. Convert it to OpenAPI 3 JSON with the Docker script. If the script has Docker-specific
   steps, move the shared part to `scripts/openapi/swagger2openapi.py` and make both crates
   use it. Do not keep two copies.
3. Put the JSON where `gen_api` reads it: `artefacts/cloud_providers/firecracker/`.

### 3.2 Generate

```bash
cargo run --bin ewe_platform gen_api analyze --provider firecracker \
  --spec artefacts/cloud_providers/firecracker/firecracker-v1.19.json
cargo run --bin ewe_platform gen_api generate --provider firecracker \
  --output-dir backends/foundation_platform_sandbox/src/firecracker
```

The output goes to `src/firecracker/generated/`. Check these points and fix the
**generator** when one is wrong (root rule 5). Record each generator fix in `learnings.md`.

| Point | Expected |
|---|---|
| Enum strings with a space | `InstanceInfo.state` has the values `Not started`, `Running`, `Paused`. The Rust enum needs `#[serde(rename = "Not started")]`. |
| Enum strings that start with a digit | `MachineConfiguration.huge_pages` has `None`, `Transparent`, `2M`. The variant needs a legal Rust name and a `rename`. |
| Optional fields | A field that is not in `required` is `Option<T>` with `skip_serializing_if = "Option::is_none"`. Firecracker rejects a `null` for several fields. |
| 204 answers | `PUT` calls return no body. The response type is `Empty`. |
| Path parameters | `drive_id` and `iface_id` are in the `Args` type and in the path. |
| Error body | `Error { fault_message }` is the error schema of each call. |
| Feature flags | The generator adds Cargo features for each group. They must sit below the crate feature `firecracker`. |

### 3.3 The client

File `src/firecracker/client.rs`:

```rust
#[derive(Clone)]
pub struct FirecrackerClient { /* DynNetClient, ApiClient, api_timeout */ }

impl FirecrackerClient {
    /// `socket` is the `--api-sock` path.
    pub fn new(ctx: &Ctx, socket: &Path, limits: &SandboxLimits) -> SandboxResult<Self>;

    pub async fn describe_instance(&self) -> SandboxResult<InstanceInfo>;
    pub async fn version(&self) -> SandboxResult<FirecrackerVersion>;
    pub async fn put_machine_config(&self, body: &MachineConfiguration) -> SandboxResult<()>;
    pub async fn put_boot_source(&self, body: &BootSource) -> SandboxResult<()>;
    pub async fn put_drive(&self, body: &Drive) -> SandboxResult<()>;
    pub async fn put_vsock(&self, body: &Vsock) -> SandboxResult<()>;
    pub async fn instance_start(&self) -> SandboxResult<()>;
}
```

- Each method calls the generated `*_request` function and maps the result.
- A status that is not 2xx becomes `SandboxError::VmmApi { status, fault_message }`. The
  generated code reads at most `BodyLimits::max_error_body` bytes of the body; the client
  sets that from `limits.max_error_message`.
- Each call has the time-out `limits.api_timeout`, from `ctx.clock()`.
- The client has only the calls that 0.1 uses. The other generated functions stay public
  in `firecracker::generated` for later features and for callers. This is not a stub:
  every method that exists is complete.

The client connects through `foundation_netio`, not through `SandboxHost.transport`. The
simulation tests of feature 06 need to replace it, so feature 06 puts a trait in front of
it (`FirecrackerApi`). This feature keeps the concrete type.

## 4. Tests

`backends/foundation_platform_sandbox/tests/firecracker_api_*.rs`. They use a mock HTTP
server on a UDS. Read `backends/foundation_deployment_platform/tests/mock_socket/` and the
Docker client tests in `backends/foundation_deployment_docker/tests/`; use the same mock.

| Test | Proves |
|---|---|
| `machine_config_body` | The server receives `PUT /machine-config` with exactly `{"vcpu_count":1,"mem_size_mib":512,"smt":false}`. No `null` fields. |
| `boot_source_body` | `kernel_image_path`, `initrd_path` and `boot_args` are present; no other key. |
| `drive_body_and_path` | The path is `/drives/rootfs`, and the body has `drive_id`, `path_on_host`, `is_root_device: false`, `is_read_only: true`. |
| `vsock_body` | `guest_cid` and `uds_path`. |
| `instance_start_body` | `{"action_type":"InstanceStart"}`. |
| `describe_instance_parses_state` | The answer `{"app_name":"Firecracker","id":"x","state":"Not started","vmm_version":"1.19.0"}` parses to the right enum value. |
| `fault_message_is_surfaced` | A 400 answer with `{"fault_message":"bad"}` gives `VmmApi { status: 400, fault_message: "bad" }`. |
| `error_body_is_bounded` | A 400 answer with a 10 MiB body gives a message of at most `max_error_message` bytes. |
| `call_times_out` | A server that does not answer gives a time-out error after `api_timeout`. |
| `types_match_real_firecracker` | Needs the `firecracker` program. Starts `firecracker --api-sock`, calls `version()` and `describe_instance()`, and puts a machine configuration. It returns with a message when the program is not found. This is the test against the real service (specifications rule 3). |

## 5. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --profile uat firecracker_api
cargo check -p foundation_platform_sandbox --no-default-features --profile uat
cargo check -p foundation_platform_sandbox --no-default-features --features libkrun --profile uat
```

The last two commands prove that the generated code and the `foundation_deployment`
dependency are behind the `firecracker` feature.

## 6. Done when

- `src/firecracker/generated/` comes from the generator, with no hand edit that the
  generator does not make. A second generator run gives no diff.
- The client methods of section 3.3 exist, and the tests of section 4 pass.
- The crate builds without the `firecracker` feature.
- Each generator fix is in `learnings.md`.
