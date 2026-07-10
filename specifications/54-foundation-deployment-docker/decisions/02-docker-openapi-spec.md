# 02 — Docker OpenAPI Spec Source

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Source the Docker Engine API spec from the **moby/moby** repository's official OpenAPI YAML,
matching the version that bollard 0.21 targets: **API v1.53** (Docker Engine v29.4.1).

**URL:** `https://raw.githubusercontent.com/moby/moby/docker-v29.4.1/api/docs/v1.53.yaml`

## Why this version

Bollard 0.21 targets Docker API v1.53, which corresponds to the `bollard-stubs` package version
`1.53.1-rc.29.3.1`. This is the most recent stable Docker Engine API schema. Using the same
version ensures our replicated types match bollard's behavior and that any type we copy from
bollard's source is valid for our target spec.

## Spec format

The moby OpenAPI spec is in **OpenAPI 3.0** format (not Swagger 2.0). The spec contains:
- `paths` — all API endpoints with their methods, parameters, and request/response schemas
- `components/schemas` — all data model types (ContainerConfig, ImageSummary, etc.)
- `components/parameters` — shared query/path parameters
- `components/responses` — shared response schemas

`foundation_openapi::SpecProcessor` already handles OpenAPI 3.0 specs, including `$ref` resolution,
`allOf`/`oneOf`/`anyOf` composition, and type normalization.

## Alternative spec sources considered

| Source | Format | Version | Notes |
|--------|--------|---------|-------|
| **moby/moby api/docs** | OpenAPI 3.0 YAML | v1.53 | Official source, what bollard uses — **chosen** |
| Docker Engine docs | Human-readable HTML | v1.52 | Not machine-parseable |
| Podman API | OpenAPI 3.0 | v5.x | Different API (libpod), incompatible with Docker |
| Third-party swagger | Various | Various | May be outdated or incomplete |

## BuildKit protobuf specs

Separate from the HTTP API spec, BuildKit uses gRPC with protobuf definitions. We source these
from bollard's `codegen/proto/resources/` directory:

| Proto file | Purpose |
|------------|---------|
| `moby/buildkit/v1/control.proto` | BuildKit control API (build, solve, status) |
| `moby/buildkit/v1/secrets.proto` | BuildKit secrets management |
| `moby/buildkit/v1/sourcepolicy/policy.proto` | Source policies |
| `moby/buildkit/v1/ssh.proto` | SSH agent forwarding |
| `moby/buildkit/v1/types/worker.proto` | Worker types |
| `moby/filesync/v1/auth.proto` | Registry authentication for builds |
| `moby/filesync/v1/filesync.proto` | File sync (COPY context) |
| `moby/filesync/v1/upload.proto` | Upload provider |
| `grpc/health/v1/health.proto` | Health checks |
| `pb/ops.proto` | BuildKit operations |

These are referenced in decision **[06 — BuildKit via foundation_connectrpc](06-buildkit-connectrpc.md)**.

## Spec storage

The spec YAML should be vendored into the `foundation_deployment_docker` crate under a `specs/`
directory, not fetched at runtime. This ensures:
- Builds are reproducible (no network dependency)
- The spec version is pinned to our implementation
- Code generation can run offline

```
foundation_deployment_docker/
  specs/
    docker-engine-v1.53.yaml    # Vendored OpenAPI spec
    buildkit/                   # Vendored protobuf files
      moby/buildkit/v1/control.proto
      ...
```

## Spec update strategy

When Docker releases a new API version:
1. Download the new YAML to `specs/`
2. Run `foundation_openapi::process_spec()` to get the diff vs current types
3. Update/add types as needed
4. Bump version constant in `foundation_deployment_docker::DOCKER_API_VERSION`
