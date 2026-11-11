---
feature: "Registry pull"
status: "pending"
priority: "low"
model: "sonnet"
phase: "3"
depends_on: ["13-container-image-source"]
crates: ["foundation_platform_sandbox"]
---

# Feature 15: Pull a container image from a registry with no Docker daemon

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); Open Container
Initiative (OCI); Hypertext Transfer Protocol (HTTP); Secure Hash Algorithm 256 (SHA-256);
JavaScript Object Notation (JSON); behaviour-driven development (BDD); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

## 1. Goal

Add the image source `Oci(reference)`: read a container image from a registry with the OCI
distribution API, and fill a sandbox image from its layers. After this feature, a local
sandbox image needs Docker in no path:

```
agent-sandbox image import oci:docker.io/library/python:3.12-slim --tag python
```

and `FROM oci:...` works in a `Sandboxfile`.

## 2. Story

- **S1**: "A user imports an image by its registry reference; no Docker daemon runs."
- **S2**: "The image has several layers; a later layer removes or replaces files of an
  earlier one; the result equals what Docker gives."
- **S3**: "The registry asks for a token; the image is public, or the user gave a name and
  a password."
- **S4**: "The reference names many platforms; the importer picks the guest architecture."
- **S5**: "A blob does not match its digest, a layer uses a format that this feature does
  not read, or the registry is not reachable."
- **S6**: "The same layer is in two images; it downloads one time."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Resolve | importer | a reference | The manifest for the guest architecture, with its digest. |
| U2 | Authorise | importer | a 401 answer | A bearer token for the repository. |
| U3 | Fetch blobs | importer | the manifest | The configuration and each layer in the blob cache, verified. |
| U4 | Apply layers | importer | the fill sandbox | The merged file tree in the image. |

## 3. Bounded context

**Inside this feature:** reference parsing, the registry client, token authentication,
platform choice, the blob cache, applying layers in order.

**Outside:** `fetch` and its limits (feature 03); unpack with whiteouts (feature 12); the
import story and the manifest mapping (feature 13), which this feature extends with one
source.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Reference | Registry host, repository, and a tag or a digest. | the caller; parsed here |
| Index | A list of manifests for platforms. | the registry |
| Manifest | The configuration digest and the layer list of one platform. | the registry |
| Blob | A configuration or a layer, named by its SHA-256 digest. | the registry; cached in `<cache_dir>/oci/blobs/sha256/<hex>` |
| Token | A bearer token for one repository and one scope. Held in memory for one import. | the registry's token service |
| Credentials | A user name and a password for one registry host. | the caller |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| parse reference | entity | import | Text to a reference, with the Docker Hub defaults. |
| request | entity | each registry call | One HTTP call, with the token when the client has one. |
| authenticate | entity | a 401 with a challenge | Gets a token; the call is tried again one time. |
| choose platform | entity | an index | The manifest whose platform is `linux` and the guest architecture. |
| fetch blob | entity | a digest | Streams to the cache, verifies the digest. |
| apply | story | fill | Unpacks each layer in order with whiteouts. |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Blob
Conserved value:         the bytes of a cached blob hash to its digest
Bound:                   size <= the size in the manifest, and <= max_download_bytes
Inverse pairs:           none
Idempotent verbs:        fetch blob (a cached blob is not downloaded again)
Ordering pairs:          none between blobs (they download one after the other in this feature)
Monotone values:         none
Freeze points:           content
Links to other entities: the manifest names the digest and the size of each blob

Entity: Pull (story)
Conserved value:         the file tree = the layers applied in manifest order with the OCI whiteout rules
Bound:                   layers <= max_layers; unpacked bytes <= max_unpack_bytes
Inverse pairs:           none
Idempotent verbs:        pull of a manifest digest that is in the image store
Ordering pairs:          resolve -> config -> layers in order
Monotone values:         layer index
Freeze points:           the manifest digest is the source identity, also when the caller gave a tag
Links to other entities: image key = from "oci:<registry>/<repository>@<manifest digest>"

Entity: Token
Conserved value:         none
Bound:                   used only for the registry host and repository that it was asked for
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          401 challenge -> token -> retry
Monotone values:         none
Freeze points:           none
Links to other entities: never sent to another host (a blob redirect to a storage host gets no Authorization header)
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | Every manifest and blob is verified against its SHA-256 digest before it is used. A mismatch fails with `ArtifactHashMismatch`. |
| I2 | The file tree of the result equals the tree that `docker export` gives for the same image digest (compared by path, type, mode, owner, link target and content hash). |
| I3 | A layer is streamed from the cache file through the decompressor to the guest. No layer is held whole in memory or unpacked on the host. |
| I4 | An `Authorization` header goes only to the registry host and its token service. A redirect to another host is followed without it. |
| I5 | Credentials and tokens never appear in a log line, an error message or a `Debug` output. |
| I6 | The source identity is the manifest digest. Two tags that name one digest give one image. |
| I7 | A reference with no architecture match fails with `ImageArchMismatch` and lists the platforms that exist. |
| I8 | A layer media type that this feature does not read fails with `InvalidSpec` and names the type. Nothing is guessed. |
| I9 | A failed pull leaves no image, no work file and no partial blob in the cache. |

### 4.3 Must have

- `ImageSource::Oci(String)`, `OciReference`, `RegistryClient`, `RegistryAuth`.
- gzip and uncompressed layers.

### 4.4 Must do

- Retry one time after a 401 with a challenge; never loop.
- Check the status before the body on every call.

### 4.5 Must not

- Must not read `~/.docker/config.json` in library code. The command line boundary can
  read it and pass credentials in.
- Must not accept `http://` for a registry unless the caller sets `allow_insecure` for that
  host (for a local test registry).

## 5. Acceptance scenarios

```gherkin
Feature: Registry pull

  Background:
    Given a local OCI registry (the "registry:2" image) with test images, reached with allow_insecure

  Scenario: An image is pulled with no Docker daemon in the path
    Given an image "test/alpine:1" in the registry
    When the user imports "oci:localhost:5000/test/alpine:1" and runs ["cat", "/etc/alpine-release"]
    Then stdout is the release line of that image

  Scenario: Layers merge like Docker
    Given an image with three layers: add /a/b and /c/d; delete /a/b; make /c opaque and add /c/e
    When it is imported
    Then /a/b does not exist, /c/d does not exist, /c/e exists
    And the file tree equals the tree of "docker export" for the same digest

  Scenario: The platform is chosen from an index
    Given a reference to an index with amd64 and arm64 manifests
    When it is imported on an x86_64 host
    Then the amd64 manifest was fetched

  Scenario: No matching platform is a clear error
    Given an index with only arm64 on an x86_64 host
    When it is imported
    Then the result is ImageArchMismatch and the message lists "linux/arm64"

  Scenario: A token is fetched when the registry asks
    Given a registry that answers 401 with a Bearer challenge and a token service
    When a public image is imported
    Then the token service got one request with the repository pull scope
    And the manifest request was sent again with the token, one time

  Scenario: Credentials go only to their registry
    Given credentials for the registry host and a blob that redirects to another host
    When the image is imported
    Then the other host got no Authorization header

  Scenario: A corrupted blob is refused
    Given a registry that serves a layer whose bytes do not match its digest
    When the image is imported
    Then the result is ArtifactHashMismatch
    And the blob cache has no file for that digest

  Scenario: A shared layer downloads one time
    Given two images that share their first layer
    When both are imported
    Then the registry served that layer's blob one time

  Scenario: Two tags, one image
    Given two tags that name the same manifest digest
    When both are imported
    Then both imports return the same image key, and the second is a cache hit

  Scenario: An unknown layer format is named
    Given an image whose layer has the media type "application/vnd.oci.image.layer.v1.tar+zstd"
    When it is imported
    Then the result is InvalidSpec and the message has that media type
```

## 6. Technical breakdown

### 6.1 Reference

```rust
pub struct OciReference { pub registry: String, pub repository: String, pub reference: TagOrDigest }
pub enum TagOrDigest { Tag(String), Digest(String) }       // digest: "sha256:<64 hex>"
impl OciReference { pub fn parse(text: &str) -> SandboxResult<Self>; }
```

| Input | Result |
|---|---|
| `alpine` | `registry-1.docker.io`, `library/alpine`, tag `latest` |
| `alpine:3.22` | tag `3.22` |
| `user/app` | `registry-1.docker.io`, `user/app`, `latest` |
| `docker.io/library/alpine` | `docker.io` is mapped to `registry-1.docker.io` |
| `ghcr.io/org/app:1` | `ghcr.io`, `org/app`, `1` |
| `localhost:5000/test/a@sha256:<hex>` | host with a port; a digest |

The first path part is a registry host when it has a `.` or a `:` or is `localhost`.
Repository parts match `[a-z0-9]+([._-][a-z0-9]+)*`. A tag matches
`[A-Za-z0-9_][A-Za-z0-9._-]{0,127}`. Anything else: `InvalidSpec { field: "reference" }`.

### 6.2 Registry client

`src/native/images/registry.rs`. HTTP through `foundation_netio`, with TLS.

```rust
pub enum RegistryAuth { Anonymous, Basic { username: String, password: SecretString } }
pub struct RegistryOptions {
    pub auth: Vec<(String, RegistryAuth)>,     // by registry host
    pub allow_insecure: Vec<String>,           // hosts reached with http://
    pub limits: RegistryLimits,
}
pub struct RegistryLimits {
    pub max_manifest_bytes: usize,   // default 4 MiB: the manifest limit of the Docker registry
    pub max_layers: usize,           // default 127: the layer limit of Docker's overlay2 driver
    pub max_redirects: u8,           // default 5
}
```

Calls (base `https://<registry>`):

| Call | Request | Notes |
|---|---|---|
| Manifest | `GET /v2/<repository>/manifests/<tag or digest>` with `Accept:` the four types below | The body is read whole, bounded by `max_manifest_bytes` (a small JSON document whose content decides the next step). The digest is `sha256` of the exact body bytes. When the reference is a digest, the body must hash to it. The answer header `Docker-Content-Digest`, when present, must equal the computed digest. |
| Blob | `GET /v2/<repository>/blobs/<digest>` | Streamed to `<cache_dir>/oci/blobs/sha256/<hex>.tmp-<random>` while it is hashed; renamed when the digest and the size match. A 307 or 302 is followed (up to `max_redirects`); a redirect to another host is requested with no `Authorization` header. |

Accepted manifest media types:

| Media type | Kind |
|---|---|
| `application/vnd.oci.image.index.v1+json`, `application/vnd.docker.distribution.manifest.list.v2+json` | index |
| `application/vnd.oci.image.manifest.v1+json`, `application/vnd.docker.distribution.manifest.v2+json` | manifest |

An index: pick the entry with `platform.os == "linux"` and `platform.architecture` equal to
`amd64` (x86_64) or `arm64` (aarch64); for `arm64` accept `variant` `v8` or none. Then
fetch that manifest by its digest. No match: `ImageArchMismatch`, with the list of
`os/architecture` values. A manifest (no index): after the configuration is read, its
`architecture` must match, with the same error.

Accepted layer media types:

| Media type | Reader |
|---|---|
| `application/vnd.oci.image.layer.v1.tar`, `application/vnd.docker.image.rootfs.diff.tar` | none (plain tar) |
| `application/vnd.oci.image.layer.v1.tar+gzip`, `application/vnd.docker.image.rootfs.diff.tar.gzip` | `flate2` |
| every other (zstd, foreign layers, encrypted) | `InvalidSpec` with the type |

zstd layers are rare on public base images; `foundation_connectrpc` has `zstd` as an
optional C library, and this feature does not add a C dependency. An issue records zstd
layers as later work.

### 6.3 Authentication

1. Send the request with no `Authorization` header (or with `Basic` when the caller gave
   credentials for the host and the registry is not Docker Hub style; see step 2).
2. A 401 answer with `WWW-Authenticate: Bearer realm="<url>",service="<s>",scope="<sc>"`:
   `GET <realm>?service=<s>&scope=repository:<repository>:pull`, with `Authorization:
   Basic` when the caller gave credentials for the registry host. The answer is JSON with
   `token` or `access_token`. Send the first request again with `Authorization: Bearer
   <token>`, one time. A second 401: `Unauthorized`.
3. A 401 with `WWW-Authenticate: Basic`: send again with the credentials when the caller
   gave them; if not, `Unauthorized`.
4. The token is kept for the life of the `RegistryClient` value (one import) and used for
   the later manifest and blob calls of the same repository.

The challenge header is parsed by a small function with tests (quoted values, any order of
the three keys, extra keys ignored). `realm` must be `https://` unless its host is in
`allow_insecure`.

### 6.4 The pull in the import story

Feature 13's import, with these steps for `ImageSource::Oci(text)`:

| Step | Work |
|---|---|
| Identify | Parse; resolve the manifest (through an index when there is one). Identity: `oci:<registry>/<repository>@<manifest digest>`. `layers.len() <= max_layers`. |
| Cache | As feature 13. |
| Fetch | The configuration blob (read from the cache file, bounded by `max_manifest_bytes`), then each layer blob in order. A blob that is in the cache is not fetched. Progress: `Pulling` with bytes. |
| Fill | `create_blank`; one `Writable` sandbox; for each layer in order: `sandbox.unpack("/", <cache file through its reader>, UnpackOptions { oci_whiteouts: true, skip: [] })`; `stop()`. |
| Commit | The manifest mapping of feature 13, section 6.3, from the configuration blob's `config` object (`Entrypoint`, `Cmd`, `Env`, `WorkingDir`, `User`) and `architecture`. |

Fault table (the rows of feature 13, plus):

| Step that fails | What the caller sees | What is left behind |
|---|---|---|
| Reference parse | `InvalidSpec { field: "reference" }` | nothing |
| Registry not reachable, or a status that is not 200 | `Io { op: "registry" }` with the status and at most `max_error_message` bytes of the body | nothing |
| Authentication | `Unauthorized` | nothing |
| Digest or size mismatch | `ArtifactHashMismatch` | no partial blob (the temporary file is removed) |
| Unknown media type | `InvalidSpec` | cached blobs of that image stay (they are verified content) |
| A layer fails to unpack | the unpack error with the layer index | no work file |

The blob cache has a size limit, `ImageLimits.blob_cache_max_bytes`
(default 20 GiB), with removal by oldest use, never a blob that a running import named.

### 6.5 Command line

`agent-sandbox image import oci:<reference>` with `--username`, `--password-stdin` and
`--insecure-registry <host>`. With no credentials given, the command line boundary reads
`~/.docker/config.json` (`auths.<host>.auth`, base64 of `user:password`) when the file
exists; a credential helper entry (`credsStore`, `credHelpers`) is not read, and the
command says so when a 401 follows. This file read is in `cli/`, not in the library.

## 7. Tests

Unit and simulation tests use a small in-process HTTP server that plays a registry
(fixtures in `tests/fixtures/oci/`, made one time with `skopeo` or `docker save`; the
command is in a README there). End-to-end tests use the `registry:2` image through Docker
on the test machine to host the fixtures; the importer under test uses no Docker.

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| reference | `reference_table` (each row of section 6.1 and bad inputs); `reference_roundtrip` (property) | `tests/oci_reference.rs` | example, identity |
| challenge parse | `www_authenticate_cases` | `tests/oci_auth.rs` | example |
| "A token is fetched ...", step 2 retry rule | `bearer_flow`, `second_401_is_unauthorized`, `basic_flow` | the same | expected outcome |
| I4, "Credentials go only to their registry" | `redirect_drops_authorization` | the same | expected outcome |
| I5 | `secrets_not_in_debug_or_errors` | the same | expected outcome |
| I1, "A corrupted blob is refused" | `manifest_digest_checked`, `blob_digest_checked`, `blob_size_checked` | `tests/oci_registry.rs` | fault |
| "The platform is chosen ...", I7 | `index_platform_choice`, `no_platform_match`, `manifest_arch_checked` | the same | expected outcome |
| I8 | `unknown_layer_type` | the same | expected outcome |
| "A shared layer downloads one time", I6 | `blob_cache_reuse`, `two_tags_one_image` | the same | expected outcome |
| I9, the fault table | `pull_fault_matrix` | `tests/images_import_*.rs` | fault |
| I3 | `layer_streams` (a 200 MiB gzip layer; peak buffer under 1 MiB) | the same | bound |
| I2, "Layers merge like Docker" | `e2e_layers_equal_docker_export`: build a three-layer image with Docker, push it to the local registry, import it with `oci:` and with `docker:`, boot both, and compare a listing of every path with type, mode, owner, link target and SHA-256 | `tests/e2e_images.rs` | differential (Docker is the second implementation) |
| the first scenario | `e2e_oci_alpine` | the same | example |
| real registry | `e2e_oci_docker_hub` pulls `alpine:3.22` from Docker Hub; returns with a message when the network is not available | the same | example |
| `FROM oci:` | `build_from_oci` | `tests/build_sim_*.rs` | example |
| CLI | `cli_image_import_oci_parse`; `docker_config_auth` (a fixture file) | CLI test files | example |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "images testing slave" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "images testing slave" --profile uat oci_ images_import build_sim
cargo test -p foundation_platform_sandbox --features "images docker testing slave" --profile uat e2e_images -- --nocapture
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes.
- The differential test against `docker export` passes for the three-layer fixture.
- `agent-sandbox image import oci:alpine:3.22` works with no Docker daemon running.
- No credential or token appears in logs, errors or `Debug` output.
