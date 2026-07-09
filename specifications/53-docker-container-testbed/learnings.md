# Learnings — Spec 53

## Bollard 0.21 API

- **Builder pattern everywhere.** Options types (`CreateContainerOptions`, `StopContainerOptions`, etc.) are built via `XxxBuilder::default().field(val).build()`, not struct literals.
- **Types moved.** Options live in `bollard::query_parameters`, models in `bollard::models`. Not in `bollard::container`.
- **ContainerCreateBody** replaces the old `Config` struct. Fields: `image`, `cmd`, `env`, `exposed_ports`, `host_config`, `labels`.
- **Generic options.** `start_container` takes `Option<StartContainerOptions>` (no type param), `create_container` takes `Option<&CreateContainerOptions>` (by ref).

## Error handling with foundation_errstacks

- **derive_more::From conflicts.** Multiple `String` variants in an enum cause conflicting `From<String>` impls. Solution: remove `derive_more::From`, use explicit `map_err` calls.
- **derive_more::Error requires inner Error impls.** `String` doesn't implement `std::error::Error`, so `Error` derive fails. Solution: `ErrorTrace<DockerError>` provides the `Error` impl; `DockerError` only needs `Display` + `Debug`.
- **`?` doesn't auto-convert to `ErrorTrace`.** Unlike `anyhow`, `ErrorTrace<C>` requires explicit wrapping. Use `docker_err(DockerError::Variant)?.` or `.map_err(|e| docker_err(...))?`.
