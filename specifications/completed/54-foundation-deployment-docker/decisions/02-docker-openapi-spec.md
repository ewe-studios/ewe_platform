# 02 — Docker Engine API Spec Source

**Date:** 2026-07-10
**Updated:** 2026-07-12 (local moby checkout)
**Status:** Resolved

## Decision

Source the Docker Engine API spec from our **local moby checkout** at
`/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/moby/api/docs/`.

This is a first-party source tree — the actual moby/moby repository — containing
every API version from v1.25 through v1.55 (latest). We do not fetch specs from
GitHub; we use the local checkout pinned at the version we target.

## Spec location

```
/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/
├── moby/
│   └── api/
│       ├── swagger.yaml            # Entry point (Swagger 2.0)
│       ├── swagger-gen.yaml        # Codegen entry point
│       └── docs/
│           ├── v1.25.yaml
│           ├── ...
│           ├── v1.53.yaml          # Our initial target
│           ├── v1.54.yaml
│           └── v1.55.yaml          # Latest
├── buildkit/                       # BuildKit protos (see decision 06)
├── containerd/                     # containerd source (for image-spec refs)
├── swarmkit/                       # Swarm orchestration
└── ...
```

## Target version: v1.53

We target **API v1.53** (Docker Engine v29.4.1). This matches bollard 0.21's target
version, which is the last release before API v1.54 introduced breaking changes in
the container networking model. Targeting v1.53 gives us:

- Full container lifecycle (create, start, stop, remove, inspect, logs, exec, wait)
- Full image management (pull, push, inspect, list, remove, build)
- Networks, volumes, system endpoints
- BuildKit integration (decision 06)

## Spec format

The moby specs are **Swagger 2.0** (not OpenAPI 3.0). The specs contain:
- `paths` — all API endpoints with methods, parameters, and response schemas
- `definitions` — all data model types (ContainerConfig, ImageSummary, etc.)
- `parameters` — shared query/path parameters
- `responses` — shared response schemas

`foundation_openapi::SpecProcessor` handles both Swagger 2.0 and OpenAPI 3.0.

## Alternative spec sources considered

| Source | Format | Version | Notes |
|--------|--------|---------|-------|
| **Local moby checkout** | Swagger 2.0 YAML | v1.25–v1.55 | First-party source of truth — **chosen** |
| GitHub moby/moby | Swagger 2.0 YAML | v1.55 | Same files, but fetches over network |
| bollard stubs | Rust types | 1.53.1-rc.29.3.1 | Derived from moby specs — good reference, not authoritative |
| Docker Engine docs | HTML | v1.52 | Not machine-parseable |
| Podman API | OpenAPI 3.0 | v5.x | Different API (libpod), incompatible |

## Why local checkout instead of fetching from GitHub

1. **Offline builds** — no network dependency during code generation
2. **Version pinning** — the spec version is pinned to the local checkout ref
3. **Full history** — all versions v1.25–v1.55 available for diff/comparison
4. **Same repo as BuildKit protos** — decision 06 sources proto files from the same tree

## Spec storage

The spec YAML should be **vendored** into the `foundation_deployment_docker` crate under
`specs/`, copied from the local moby checkout at codegen time. This ensures:
- Builds are reproducible (no dependency on the formulas directory)
- The spec version is pinned to our implementation
- Code generation can run offline against the vendored copy

```
foundation_deployment_docker/
  specs/
    docker-engine-v1.53.yaml    # Vendored from local moby checkout
```

## Spec update strategy

When Docker releases a new API version and the local moby checkout is updated:

1. Identify the target version from `moby/api/docs/`
2. Copy the YAML to `foundation_deployment_docker/specs/`
3. Run the generator: `gen_api generate --provider docker --spec specs/docker-engine-v{N}.yaml`
4. Review the generated diff against existing types
5. Update/add hand-written refinements as needed
6. Bump `foundation_deployment_docker::DOCKER_API_VERSION`

## Related decisions

- **[06 — BuildKit via foundation_connectrpc](06-buildkit-connectrpc.md)** — Proto files from same local checkout
- **[03 — Type replication strategy](03-type-replication-strategy.md)** — How we generate from the spec
- **[04 — HTTP via DynNetClient + PreparedRequestBuilder](04-http-via-simple-http-client.md)** — HTTP transport
