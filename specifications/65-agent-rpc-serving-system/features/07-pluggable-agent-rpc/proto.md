# Wire contract changes (`agent.proto`)

_Part of [feature 07](./feature.md). Written in ASD-STE100 Simplified Technical English (STE).
Code and identifiers stay as they are._

**Abbreviations in this file.** remote procedure call (RPC); Protocol Buffers (protobuf);
uniform resource locator (URL); Hypertext Transfer Protocol (HTTP); JavaScript Object
Notation (JSON); Transport Layer Security (TLS); Secure Hash Algorithm with 256 bits
(SHA-256); Distinguished Encoding Rules (DER), a binary form of a certificate; Certificate
Authority (CA); certificate signing request (CSR); Quick Response code (QR code); central
processing unit (CPU); graphics processing unit (GPU); Compute Unified Device Architecture
(CUDA), the NVIDIA GPU platform; Generative Pre-trained Transformer (GPT);
GPT-Generated Unified Format (GGUF), the model file format of llama.cpp; Durable Object (DO); enumeration (enum). gRPC is the name of a Google RPC
protocol over HTTP/2. gRPC-Web is its variant for browsers. Connect is an RPC protocol that
works over HTTP/1.1 and HTTP/2. AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100. The [glossary in feature.md](./feature.md#2-glossary) explains
each term.

## 1. What this file covers

This file gives the complete change to the wire contract of agent-rpc.

- The **wire contract** is the set of messages and calls that a client and a server send to each other.
- We write the contract in Protocol Buffers (protobuf). Protobuf is a language for messages. A tool turns it into Rust types.
- The contract file is `backends/foundation_agent_rpc/proto/ewe/agent_rpc/v1/agent.proto`.
- `build.rs` turns it into Rust with `buffa-build` (messages) and `foundation_connectrpc_codegen` (services). Do not write these types by hand (CLAUDE.md rule 5).

The changes add four things:

1. A new remote procedure call (RPC), `GetCapabilities`. An RPC is a function that a client calls over the network.
2. A model kind and a device on `CreateInstanceRequest`.
3. Paging on `ListInstances`.
4. One address form for `BootstrapPayload`: a uniform resource locator (URL) for every host, native or Cloudflare Worker. The payload has no version field (owner decision, PR #126 review, 2026-10-25). Section 6 gives the reasons.

## 2. Compatibility rules

These rules keep old clients and old servers working.

| Rule | Why |
|---|---|
| Add new fields with new field numbers only. | Old readers skip unknown fields. |
| Never change the number or the type of a field. | A changed number makes old bytes decode wrong. |
| Every new enumeration (enum) starts with `..._UNSPECIFIED = 0`. | Zero is the value an old client sends. It must mean "use the old behavior". |
| Prefix each enum value with the enum name, for example `MODEL_KIND_API`. | In protobuf, all enum values of a package share one name space. |
| `BootstrapPayload` removes `version`, `addr` and `bootstrap_addr`. Their numbers (1, 2, 3) and names are `reserved`. | The feature 01 payload was never released, so no deployed client reads it. `reserved` stops a later change from giving the old numbers a new meaning. |
| A server answers an unknown enum value with `INVALID_ARGUMENT`. | `buffa` keeps unknown values as `EnumValue::Unknown(i32)`. The server must not guess. |

Generated Rust names: `buffa` keeps the protobuf value names. So the Rust code uses
`ModelKind::MODEL_KIND_CANDLE`. An enum field is a `buffa::EnumValue<ModelKind>`, which is
`Known(ModelKind)` or `Unknown(i32)`.

## 3. The new `agent.proto` (complete file)

The file below replaces the current file. Each `NEW (07)` comment marks an addition.
Lines without that comment are the same as today.

```proto
// agent-rpc wire contract (spec 65 features 01 and 07).
//
// AgentService is the client <-> agent-rpc API.
// BootstrapService is the signed secret + CSR -> client certificate exchange.
// BootstrapPayload is what an operator hands a client (base64url / QR code).
syntax = "proto3";

package ewe.agent_rpc.v1;

service AgentService {
  // Liveness plus what the server hosts.
  rpc Health(HealthRequest) returns (HealthResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
  // NEW (07): the model kinds, devices, models, protocols and limits of this host.
  rpc GetCapabilities(GetCapabilitiesRequest) returns (GetCapabilitiesResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
  // Create a named instance on a hosted model (no model load for local
  // models that are already loaded).
  rpc CreateInstance(CreateInstanceRequest) returns (InstanceInfo);
  rpc ListInstances(ListInstancesRequest) returns (ListInstancesResponse) {
    option idempotency_level = NO_SIDE_EFFECTS;
  }
  rpc DestroyInstance(DestroyInstanceRequest) returns (DestroyInstanceResponse);
  // Run one turn and return its text.
  rpc Chat(ChatRequest) returns (ChatResponse);
  // Run one turn and stream its events as they happen.
  rpc ChatStream(ChatRequest) returns (stream TurnEvent);
}

service BootstrapService {
  // Present a signed bootstrap secret and a credential (a client public key,
  // or a CSR); receive a client_id, the credential, and a server proof.
  // Any client can call it, a browser included (auth.md section 9.1).
  rpc Enroll(EnrollRequest) returns (EnrollResponse);
  // NEW (07): a new access secret for a client-key client, signed with its key.
  rpc RefreshAccess(RefreshAccessRequest) returns (RefreshAccessResponse);
}

// ---------------------------------------------------------------------------
// Model kinds and devices. NEW (07).

// The family of a model. Zero means "any kind".
enum ModelKind {
  MODEL_KIND_UNSPECIFIED = 0;
  // A model at a remote provider, reached over HTTP (OpenAI-compatible,
  // Anthropic, Workers AI).
  MODEL_KIND_API = 1;
  // A local safetensors model that runs on Candle.
  MODEL_KIND_CANDLE = 2;
  // A local GGUF model that runs on llama.cpp.
  MODEL_KIND_GGUF = 3;
  // Kept for future video models. No host supports it yet.
  MODEL_KIND_VIDEO = 4;
  // Kept for future audio models. No host supports it yet.
  MODEL_KIND_AUDIO = 5;
}

// Where a model runs. Zero means "any device".
enum DeviceKind {
  DEVICE_KIND_UNSPECIFIED = 0;
  DEVICE_KIND_CPU = 1;
  DEVICE_KIND_CUDA = 2;
  DEVICE_KIND_METAL = 3;
  DEVICE_KIND_VULKAN = 4;
  // The model runs at the remote provider. Only for MODEL_KIND_API.
  DEVICE_KIND_REMOTE = 5;
}

message Device {
  DeviceKind kind = 1;
  // The GPU index. Always 0 for CPU and REMOTE.
  uint32 ordinal = 2;
}

// The load state of one configured model.
enum ModelLoadState {
  MODEL_LOAD_STATE_UNSPECIFIED = 0;
  MODEL_LOAD_STATE_NOT_LOADED = 1;
  MODEL_LOAD_STATE_LOADING = 2;
  MODEL_LOAD_STATE_LOADED = 3;
  MODEL_LOAD_STATE_FAILED = 4;
  // Nothing to load: the remote provider holds the model.
  MODEL_LOAD_STATE_REMOTE = 5;
}

message GetCapabilitiesRequest {}

message GetCapabilitiesResponse {
  // One entry per model kind that this host can run.
  repeated ModelKindSupport model_kinds = 1;
  // Every model that the operator configured on this host.
  repeated ModelDescriptor models = 2;
  // "connect", "grpc", "grpc-web".
  repeated string protocols = 3;
  // "mtls", "signed-secret", "jwt", "cloudflare-mtls", "insecure".
  repeated string auth_modes = 4;
  InstanceLimits limits = 5;
  // "native" or "worker".
  string host_kind = 6;
}

message ModelKindSupport {
  ModelKind kind = 1;
  // The devices on which this host can run the kind.
  repeated Device devices = 2;
}

message ModelDescriptor {
  string name = 1;
  ModelKind kind = 2;
  Device device = 3;
  ModelLoadState state = 4;
}

message InstanceLimits {
  uint32 max_instances = 1;
  uint32 max_name_len = 2;
  uint32 max_prompt_bytes = 3;
}

// The error detail that a failed CreateInstance carries. NEW (07).
message ModelSelectionError {
  string requested_model = 1;
  ModelKind requested_kind = 2;
  Device requested_device = 3;
  // What this host supports, so the client can pick again.
  repeated ModelKindSupport supported = 4;
}

// ---------------------------------------------------------------------------
// Health and instances.

message HealthRequest {}

message HealthResponse {
  string status = 1;
  repeated ModelInfo models = 2;
  uint32 instances = 3;
}

message ModelInfo {
  string name = 1;
  ModelKind kind = 2;   // NEW (07)
  Device device = 3;    // NEW (07)
}

message CreateInstanceRequest {
  string name = 1;
  // Empty = pick by kind and device (see "Model selection" in proto.md).
  string model = 2;
  optional string system_prompt = 3;
  optional uint32 max_tokens = 4;
  optional float temperature = 5;
  // NEW (07): UNSPECIFIED = any kind.
  ModelKind kind = 6;
  // NEW (07): not set, or DEVICE_KIND_UNSPECIFIED = any device.
  Device device = 7;
}

message InstanceInfo {
  string name = 1;
  string model = 2;
  uint64 created_unix_ms = 3;
  uint64 turns = 4;
  ModelKind kind = 5;   // NEW (07)
  Device device = 6;    // NEW (07)
}

message ListInstancesRequest {
  // NEW (07): 0 = the server default (RegistryConfig::default_page_size).
  uint32 page_size = 1;
  // NEW (07): empty = the first page.
  string page_token = 2;
}

message ListInstancesResponse {
  repeated InstanceInfo instances = 1;
  // NEW (07): empty = no more pages.
  string next_page_token = 2;
}

message DestroyInstanceRequest {
  string name = 1;
}

message DestroyInstanceResponse {}

// ---------------------------------------------------------------------------
// Chat. No change in 07.

message ChatRequest {
  string instance = 1;
  string prompt = 2;
}

message ChatResponse {
  string text = 1;
  Usage usage = 2;
  uint64 message_count = 3;
}

// Cumulative session token usage.
message Usage {
  uint64 input_tokens = 1;
  uint64 output_tokens = 2;
  uint64 total_tokens = 3;
}

message TurnEvent {
  oneof event {
    // Assistant text (a delta for streaming backends).
    string text = 1;
    string thinking = 2;
    ToolCall tool_call = 3;
    ToolResult tool_result = 4;
    // Drop the text shown so far for this turn; a retry follows.
    Retract retract = 5;
    // Terminal: the turn failed after the events before it.
    TurnFailed failed = 6;
    // Terminal: the turn completed.
    TurnDone done = 7;
  }
}

message ToolCall {
  string id = 1;
  string name = 2;
  string arguments_json = 3;
}

message ToolResult {
  string tool_call_id = 1;
  string name = 2;
  string content = 3;
}

message Retract {
  string reason = 1;
}

message TurnFailed {
  string message = 1;
}

message TurnDone {
  uint64 message_count = 1;
  Usage usage = 2;
}

// ---------------------------------------------------------------------------
// Bootstrap.

message EnrollRequest {
  // A signed bootstrap secret (base64url JSON, see auth.md), as carried by
  // BootstrapPayload.token. Its purpose must be "enroll" and its aud must be
  // this host's url.
  string token = 1;
  // A display name for the client, recorded with it.
  string client_name = 2;
  // The credential the client wants. Exactly one is set.
  oneof credential {
    // PKCS#10 certificate signing request; the private key stays on the
    // client. The client then uses mTLS. Native hosts only.
    string csr_pem = 3;
    // NEW (07): a client public key; the private key stays on the client.
    // The client then uses access secrets. Every host that enrolls.
    ClientKey client_key = 4;
  }
  // NEW (07): 32 random bytes from the client. The server signs them in
  // server_proof.
  bytes nonce = 5;
}

// NEW (07): a client public key.
message ClientKey {
  // "ed25519" (raw 32 bytes) or "p256" (uncompressed SEC1, 65 bytes).
  string alg = 1;
  bytes public_key = 2;
}

message EnrollResponse {
  // Set for the csr_pem credential.
  string client_cert_pem = 1;
  // The CA that signs client and server certificates. Set for csr_pem.
  string ca_cert_pem = 2;
  // NEW (07): the ID of the enrolled client.
  string client_id = 3;
  // NEW (07): set for the client_key credential. A SignedSecret v2 with
  // purpose access, aud = url, sub = client_id. Send it as a bearer credential.
  string access_secret = 4;
  // NEW (07): the server's signature, with the payload private key, over the
  // nonce, the client credential and client_id (auth.md section 9.1.1).
  bytes server_proof = 5;
}

// NEW (07). Signed with the client private key over
// "ewe.agent_rpc.refresh.v1" 0x00 client_id unix_ms nonce.
message RefreshAccessRequest {
  string client_id = 1;
  uint64 unix_ms = 2;
  bytes nonce = 3;
  bytes signature = 4;
}

message RefreshAccessResponse {
  string access_secret = 1;
}

// What an operator gives a client, as base64url text or a QR code.
// CHANGED (07). There is no version field: protobuf field numbers already let
// an old reader skip a field that it does not know, and let a new reader see
// that an old field is absent. Section 6 of proto.md explains the choice.
message BootstrapPayload {
  // Removed in 07. Never reuse these numbers or names.
  reserved 1, 2, 3;
  reserved "version", "addr", "bootstrap_addr";
  // Lowercase hex SHA-256 of the server certificate (DER).
  // Set: the client pins this certificate (a host with its own CA, such as
  // the native host). Empty: the client checks the certificate against the
  // system root certificates (a host with a public certificate, such as a
  // Cloudflare Worker).
  string tls_fingerprint_sha256 = 4;
  // The signed, time-boxed secret (see auth.md). It also carries the server
  // public key. Its purpose says what the client does with it: Enroll = call
  // Enroll at enroll_url (any client, a browser included); Access = send it
  // as a bearer credential on each call to url.
  string token = 5;
  Capabilities capabilities = 6;
  // NEW (07): the base URL of AgentService. One form for every host, for
  // example "https://203.0.113.7:7443" or "https://agents.example.com/rpc".
  string url = 7;
  // NEW (07): the base URL of BootstrapService. Set only when the token
  // purpose is Enroll. It can equal url (one listener, for example a Worker).
  // The same tls_fingerprint_sha256 applies to it.
  string enroll_url = 8;
}

message Capabilities {
  repeated string protocols = 1;
  repeated string models = 2;
  // Removed in 07: the single auth_mode string. Use auth_modes.
  reserved 3;
  reserved "auth_mode";
  repeated ModelKind model_kinds = 4;   // NEW (07)
  repeated string auth_modes = 5;       // NEW (07)
}
```

## 4. Field table

| Message.field | Number | Change | Effect on old peers |
|---|---|---|---|
| `AgentService.GetCapabilities` | — | new RPC | An old server answers `UNIMPLEMENTED`. A new client then falls back to `Health`. |
| `BootstrapService.RefreshAccess` | — | new RPC | Only clients that enrolled with a client key call it. |
| `EnrollRequest.credential` (`csr_pem` in a oneof, new `client_key`), `.nonce` | 3, 4, 5 | changed, new | `csr_pem` keeps number 3, so a feature 01 request still decodes. Without a `nonce`, the server refuses (`INVALID_ARGUMENT`): feature 01 clients enroll again with a new build. |
| `EnrollResponse.client_id`, `.access_secret`, `.server_proof` | 3, 4, 5 | new | An old client ignores them. |
| `CreateInstanceRequest.kind` | 6 | new | An old server ignores it and uses `model` as today. |
| `CreateInstanceRequest.device` | 7 | new | Same as above. |
| `InstanceInfo.kind`, `.device` | 5, 6 | new | An old client ignores them. |
| `ModelInfo.kind`, `.device` | 2, 3 | new | An old client ignores them. |
| `ListInstancesRequest.page_size`, `.page_token` | 1, 2 | new | An old client sends zero and empty: it gets the first page. |
| `ListInstancesResponse.next_page_token` | 2 | new | An old client reads only the first page. |
| `Capabilities.model_kinds`, `.auth_modes` | 4, 5 | new | An old client ignores them. |
| `Capabilities.auth_mode` | 3 | removed, reserved | Replaced by `auth_modes`. |
| `BootstrapPayload.version`, `.addr`, `.bootstrap_addr` | 1, 2, 3 | removed, reserved | A feature 01 build cannot read a new payload, and a new build refuses a feature 01 payload (it has no `url`). Feature 01 was never released, so no deployed client is affected. The operator issues a new payload. |
| `BootstrapPayload.url`, `.enroll_url` | 7, 8 | new | See above. |

## 5. Model selection for `CreateInstance`

`CreateInstance` now carries `model`, `kind` and `device`. The server picks one configured
model with these rules. The rules live in `core::model::catalog::select` (see
[registry-and-models.md](./registry-and-models.md)).

This diagram shows how the server picks a model and which error code each failure gives.

```mermaid
flowchart TD
    start["CreateInstance arrives"] --> unknown{"kind or device is an unknown enum value?"}
    unknown -- yes --> e1["INVALID_ARGUMENT"]
    unknown -- no --> future{"kind is VIDEO or AUDIO?"}
    future -- yes --> e2["UNIMPLEMENTED"]
    future -- no --> named{"model name is set?"}
    named -- yes --> found{"a model with this name exists?"}
    found -- no --> e3["NOT_FOUND"]
    found -- yes --> kindok{"kind is UNSPECIFIED or equal to the model kind?"}
    kindok -- no --> e4["INVALID_ARGUMENT"]
    kindok -- yes --> devok{"device is UNSPECIFIED or equal to the model device?"}
    devok -- no --> e5["FAILED_PRECONDITION"]
    devok -- yes --> ok["use this model"]
    named -- no --> filter["keep the models that match kind and device"]
    filter --> count{"how many models match?"}
    count -- one --> ok
    count -- more than one --> e6["INVALID_ARGUMENT: name a model"]
    count -- none --> why{"does this host support the kind at all?"}
    why -- no --> e7["UNIMPLEMENTED"]
    why -- yes --> dev{"does this host support the kind on that device?"}
    dev -- no --> e8["FAILED_PRECONDITION"]
    dev -- yes --> e9["NOT_FOUND"]
```

The same rules as a table:

| Case | Connect code | Error detail |
|---|---|---|
| `kind` or `device.kind` is an unknown number | `INVALID_ARGUMENT` | `ModelSelectionError` |
| `kind` is `MODEL_KIND_VIDEO` or `MODEL_KIND_AUDIO` | `UNIMPLEMENTED` | `ModelSelectionError` |
| `model` names no configured model | `NOT_FOUND` | `ModelSelectionError` |
| `model` is set, `kind` is set and different | `INVALID_ARGUMENT` | `ModelSelectionError` |
| `model` is set, `device` is set and different | `FAILED_PRECONDITION` | `ModelSelectionError` |
| `model` is empty, the host never runs this `kind` (for example `GGUF` on a Worker) | `UNIMPLEMENTED` | `ModelSelectionError` |
| `model` is empty, the host runs `kind` but not on this `device` (for example `CUDA` on a host without CUDA) | `FAILED_PRECONDITION` | `ModelSelectionError` |
| `model` is empty, the host runs `kind` on `device`, but no model is configured | `NOT_FOUND` | `ModelSelectionError` |
| `model` is empty and more than one model matches | `INVALID_ARGUMENT` | `ModelSelectionError` |
| `DEVICE_KIND_REMOTE` with a local kind, or a local device with `MODEL_KIND_API` | `INVALID_ARGUMENT` | `ModelSelectionError` |
| All of `model`, `kind` and `device` are empty and the host has one model | none: use it | — |

The last row is the behavior of today. So old clients see no change.

"Connect code" is the status code of the Connect protocol. The same number goes into
`grpc-status` for gRPC and gRPC-Web.

## 6. `BootstrapPayload`: one address form, no version

A bootstrap payload is one string. An operator gives it to a client. The client needs nothing
else to connect. The string is base64url (unpadded) of the protobuf bytes.

### 6.1 Why there is no version field and no second address type

The owner asked (PR #126 review, 2026-10-25): why versions, and why not one URL type for hosted
and one for serverless, when a Cloudflare Worker also has an address?

- **No version field.** Protobuf already handles change. Each field has a number. An old reader
  skips numbers that it does not know. A new reader sees that a field is absent. A version
  field adds a second rule that can disagree with the fields. The earlier design used
  `version = 2` only to say "use `base_url`, not `addr`". With one address form, nothing is left
  for it to say.
- **The address is signed.** An enrollment token carries `aud` = `url` inside its signature, and
  the server proves that it holds the payload private key (auth.md section 9.1.1). So the payload
  is the owner's tamper-proof package: address, secret, signature and server public key.
- **One address type.** Every host has an address, and a URL can hold all of them:
  `https://203.0.113.7:7443` for a native host, `https://agents.example.com/rpc` for a Worker.
  The host kinds differ in two other facts, and the payload already carries each one:

  | Fact | Native host | Worker | Where the payload says it |
  |---|---|---|---|
  | How the client checks the server certificate | pin the host's own certificate | system root certificates | `tls_fingerprint_sha256` set or empty |
  | How the client gets access | enroll (client key or, for a native client, a CSR) | enroll with a client key, or a bearer token from the operator | the token's `purpose` (`Enroll` or `Access`), and `enroll_url` |

  Two URL types would repeat these facts, and could disagree with them (for example a
  "serverless" URL with a pinned fingerprint). So the payload has one `url`, and the client
  derives the rest.

### 6.2 What the client derives (`core::bootstrap::decode`)

`decode` checks the wire message and returns a Rust value in which each case is a type:

```rust
// foundation_agent_rpc/src/core/bootstrap.rs (task T13)

/// A checked bootstrap payload. Made by `decode`; read by the native client
/// and the web client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bootstrap {
    /// The AgentService base URL.
    pub url: Url,
    pub trust: TlsTrust,
    pub access: BootstrapAccess,
    pub capabilities: Capabilities,
}

/// How the client checks the server certificate. Derived, not on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsTrust {
    /// `tls_fingerprint_sha256` was set: pin this SHA-256.
    PinnedSha256([u8; 32]),
    /// The fingerprint was empty and the URL is `https`: system roots.
    SystemRoots,
    /// The URL is `http` with a loopback host. Development only.
    Plaintext,
}

/// How the client gets access. Derived from the token purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapAccess {
    /// Purpose `Enroll`: call `Enroll` at `enroll_url` with a client key (any
    /// client) or a CSR (native, then mTLS). Check the server proof with
    /// `token.public_key` (auth.md section 9.1).
    Enroll { token: SignedSecret, enroll_url: Url },
    /// Purpose `Access`: send the token as a bearer credential on each call.
    Bearer { token: SignedSecret },
}
```

### 6.3 Validation rules

| Field | Rule |
|---|---|
| `url` | Required. `https://<host>[:port][/path]`, or `http://` only with a loopback host. An empty `url` gives the message "this payload has no url; payloads made before feature 07 are not accepted, issue a new one". |
| `tls_fingerprint_sha256` | Empty, or 64 lowercase hex characters. Must be empty with an `http` URL. |
| `token` | Required. Decodes as a `SignedSecret` with purpose `Enroll` or `Access`. Its signature is checked with its own `public_key`. For `Enroll`, its `aud` must equal `url`, so a changed address fails. |
| `enroll_url` | Required when the purpose is `Enroll`. Must be empty when the purpose is `Access`. Same scheme rules as `url`. |
| `capabilities` | Optional. |

A decode error is `AgentRpcError::Codec`. It maps to `INVALID_ARGUMENT`.

### 6.4 What each host writes

- The native host writes `url` and `enroll_url` from `NativeHostConfig::public_url` and
  `public_enroll_url`. When they are not set, it uses `https://` and the bound address of each
  listener, as today. It sets the fingerprint and issues an `Enroll` token. With
  `NativeSecurity::Mtls { accept_access_secrets: true }`, the operator can also issue a payload
  with an `Access` token and no `enroll_url`.
- The Worker host writes its public URL and no fingerprint. With
  `WorkerHostConfig::enrollment` set, it issues an `Enroll` token and sets `enroll_url` to the
  same URL (the Worker serves `BootstrapService` on the same origin). Without it, the operator's
  tool issues an `Access` token and `enroll_url` stays empty.

A Worker fills the payload like this:

```rust
// host::worker::bootstrap — a verify-only Worker (no `enrollment` config).
// `public_url` comes from the Worker configuration (var AGENT_RPC_PUBLIC_URL).
// `token` is a SignedSecret with purpose = Access, issued by the operator's
// tool that holds the private key. A Worker with `enrollment` set writes an
// Enroll token instead, with enroll_url = url, and clients enroll with it.
let payload = BootstrapPayload {
    url: "https://agents.example.com/rpc".into(),
    enroll_url: String::new(),                        // no enrollment on a Worker
    tls_fingerprint_sha256: String::new(),            // public certificate: system roots
    token: token.encode().to_string(),
    capabilities: Capabilities {
        protocols: vec!["connect".into(), "grpc-web".into()],
        models: vec!["llama-3.1-8b".into()],
        model_kinds: vec![ModelKind::MODEL_KIND_API.into()],
        auth_modes: vec!["signed-secret".into()],
        ..Default::default()
    }
    .into(),
    ..Default::default()
};
```

What the code does: it gives a URL, no pin and a bearer token. The client derives
`TlsTrust::SystemRoots` and `BootstrapAccess::Bearer`. With an `Enroll` token, the client derives
`BootstrapAccess::Enroll` and enrolls with a client key (auth.md section 9.1).

A native host fills the same message like this:

```rust
// host::native::server::RunningServer::payload_with
let payload = BootstrapPayload {
    url: "https://203.0.113.7:7443".into(),
    enroll_url: "https://203.0.113.7:7444".into(),
    tls_fingerprint_sha256: identity.fingerprint_sha256().into(),
    token: token.encode().to_string(),               // SignedSecret, purpose = Enroll
    capabilities: Capabilities {
        protocols: vec!["connect".into(), "grpc".into(), "grpc-web".into()],
        models: vec!["tiny-llama".into()],
        model_kinds: vec![ModelKind::MODEL_KIND_CANDLE.into()],
        auth_modes: vec!["mtls".into()],
        ..Default::default()
    }
    .into(),
    ..Default::default()
};
```

What the code does: it gives two URLs, a pin and an enrollment token. The client derives
`TlsTrust::PinnedSha256` and `BootstrapAccess::Enroll`.

## 7. Use of `AgentService` between a Worker and a Durable Object

A Durable Object (DO) is a Cloudflare object with one name, one thread and its own storage.
The Worker host runs one DO for each agent instance (see [hosts.md](./hosts.md)).

The Worker does not need a second service definition to talk to the DO:

- The DO serves the same `AgentService`. It holds exactly one instance.
- The Worker calls the DO with the generated `AgentServiceClient` over a `DoStubTransport`.
- The Worker adds the header `ewe-forwarded-principal`. It holds the caller identity that the Worker has already checked. The value is JSON: `{"subject":"…","method":"signed-secret","roles":["agent-rpc:client"]}`.
- The Worker deletes every incoming header that starts with `ewe-forwarded-` before it checks the caller. So a client cannot send a false identity.
- A DO is not reachable from the internet. Only code in the same Cloudflare account reaches it, through the DO namespace binding. So the DO trusts the header.

## 8. Tests for this contract

All tests go in `backends/foundation_agent_rpc/tests/` (CLAUDE.md rule 7).

| Test file | What it proves |
|---|---|
| `tests/proto_compat_tests.rs` | A native payload and a Worker payload round-trip and derive the expected `TlsTrust` and `BootstrapAccess`. A feature 01 payload (a golden string kept in `tests/fixtures/payload_f01.txt`) is refused with the "issue a new one" message. Each validation rule in section 6.3 has one failing case. |
| `tests/capabilities_tests.rs` | `GetCapabilities` lists exactly the kinds and devices that the catalog reports. |
| `tests/model_selection_tests.rs` | One test for each row of the table in section 5. Each test checks the code and the `ModelSelectionError` detail. |
| `tests/list_paging_tests.rs` | Paging returns every instance once, in name order, with stable tokens. |
