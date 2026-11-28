---
feature: "GCP KMS over gRPC, tested on localgcp"
status: "pending"
priority: "high"
model: "sonnet"
phase: "1"
depends_on: ["00-live-proof"]
crates: ["foundation_auth", "foundation_connectrpc"]
---

# Feature 03: GCP KMS over gRPC, tested on localgcp

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

`GcpKmsBackend` speaks **gRPC** as well as REST, so the tests run against **localgcp**, a
local Google Cloud emulator, on every machine and in a free CI job. The real project stays
the gated proof.

The transport already exists. `foundation_connectrpc` has the h2c transport
(`native/transport/h2.rs:338`), real HTTP/2 trailers for `grpc-status` (`:1471`), and a
protobuf codec over `buffa` (`shared/codec/proto.rs`). The missing pieces are the Cloud KMS
protobuf messages and a protocol choice in the backend.

## 2. Tasks

| Task | Work |
|---|---|
| T1 | Vendor the Cloud KMS protos under `backends/foundation_auth/proto/google/cloud/kms/v1/`: `service.proto` and `resources.proto`, with the `google.protobuf` imports. Record the commit or release tag they come from in a `README.md` beside them. |
| T2 | Generate the messages with `buffa-build` in `foundation_auth`'s build script, behind the `kms-grpc` feature (native only). Add a drift check that the committed output matches the vendored protos, as `foundation_registrar` does. |
| T3 | `GcpKmsBackend` gains a protocol: `KmsProtocol::Rest { base_url }` (today) and `KmsProtocol::Grpc { endpoint }`. gRPC calls `KeyManagementService/Encrypt`, `Decrypt`, `CreateKey`, `DescribeKey`, `GenerateDataKey` and `ScheduleKeyDeletion`, with the bearer token in the `authorization` metadata. |
| T4 | The localgcp runner is a `foundation_ci` `TaskSet` task, `services localgcp` (the owner chose the task system over `mise` on 2026-11-27): it downloads the pinned release (v0.6.0) for the host, checks the sha256 from the release `checksums.txt`, starts `localgcp up`, saves the PID file, waits for the KMS port, and stops by that PID. `services localgcp --keep-running` leaves it up. |
| T5 | Tests in `foundation_auth/tests/kms/gcp_grpc.rs`: with `GCP_KMS_ENDPOINT` set (for example `http://127.0.0.1:8091`) they create a key ring and a key, seal and open, refuse a wrong key, and destroy. Without it they print `skip: set GCP_KMS_ENDPOINT` and return. |
| T6 | A separate workflow `.github/workflows/kms-gcp-local.yaml` (its own concurrency group, per rule 16): run `foundation-ci services localgcp --keep-running`, then T5, then stop the task. No secret, free tier. Do not edit `check.yaml`. |

## 3. Configuration

| Field | Meaning | Default |
|---|---|---|
| `endpoint` | The KMS endpoint | the regional REST URL, unchanged |
| `protocol` | `Rest` or `Grpc` | `Rest`, so no existing caller changes |
| `timeout` | One call's deadline | 30 s, as the REST path uses |
| `project`, `location`, `key_ring` | The resource names the gRPC calls need | none; a missing value is a typed `Config` error |

`GCP_KMS_ENDPOINT` and `GCP_KMS_PROTOCOL=grpc` set the values for a test. A secret never
appears in a log.

## 4. Tests

| Test | Oracle |
|---|---|
| `gcp_grpc_seal_and_open` (against localgcp) | the sealer contract |
| `gcp_grpc_refuses_a_wrong_key` | expected outcome |
| `gcp_grpc_create_and_destroy` | conservation: the key exists, then it is gone |
| the REST tests of feature 08 remain unchanged | no regression |
| proto drift check | identity |

## 5. Verify

```bash
cargo run --profile uat -p foundation_ci -- services localgcp --keep-running
GCP_KMS_ENDPOINT=http://127.0.0.1:8091 GCP_KMS_PROTOCOL=grpc cargo test -p foundation_auth --features "testing kms-grpc" --profile uat kms::gcp_grpc
cargo test -p foundation_auth --features testing --profile uat kms
cargo fmt -p foundation_auth -- --check
cargo clippy -p foundation_auth --features "testing kms-grpc" --all-targets --profile uat --no-deps -- -D warnings
```

## 6. Done when

- `GcpKmsBackend` makes gRPC calls against localgcp and the tests above pass.
- The REST path is unchanged and its tests pass.
- The workflow `kms-gcp-local` is green.
- The real-project test of feature 04 uses the same backend with its own endpoint.
