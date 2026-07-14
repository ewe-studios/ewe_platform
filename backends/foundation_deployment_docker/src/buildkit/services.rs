//! ConnectRPC service definitions for BuildKit — generated from the `.proto`
//! service declarations by `foundation_connectrpc_codegen` in `build.rs`.
//!
//! WHY: Rather than hand-wire each `Client`/`Router` registration, we generate
//! the service trait + `register_*` fn + typed client straight from the BuildKit
//! protos (the connectrpc build.rs codegen pattern). The generator refers to
//! message types by bare name, so each service is included in a module that
//! brings the matching buffa message types (from [`super::generated`]) into
//! scope.
//!
//! WHAT:
//! - [`control`] — the `Control` service client (Solve/Status/Info/…).
//! - [`filesync`], [`auth`], [`secrets`], [`ssh`] — the session sidecar service
//!   traits + registration fns the client serves back to buildkitd during a
//!   Solve.
#![allow(
    clippy::all,
    clippy::pedantic,
    dead_code,
    unused_variables,
    unused_imports,
    non_snake_case,
    async_fn_in_trait
)]

/// `moby.buildkit.v1.Control` — the main build-control service (client side).
pub mod control {
    use crate::buildkit::generated::moby::buildkit::v1::*;
    include!(concat!(env!("OUT_DIR"), "/control_service.rs"));
}

/// `moby.filesync.v1.FileSync` — streams the build context to buildkitd.
pub mod filesync {
    use crate::buildkit::generated::fsutil::types::*;
    include!(concat!(env!("OUT_DIR"), "/filesync_service.rs"));
}

/// `moby.filesync.v1.Auth` — registry credential callbacks.
pub mod auth {
    use crate::buildkit::generated::moby::filesync::v1::*;
    include!(concat!(env!("OUT_DIR"), "/auth_service.rs"));
}

/// `moby.buildkit.secrets.v1.Secrets` — build secret callbacks.
pub mod secrets {
    use crate::buildkit::generated::moby::buildkit::secrets::v1::*;
    include!(concat!(env!("OUT_DIR"), "/secrets_service.rs"));
}

/// `moby.sshforward.v1.SSH` — SSH agent-forwarding callbacks.
pub mod ssh {
    use crate::buildkit::generated::moby::sshforward::v1::*;
    include!(concat!(env!("OUT_DIR"), "/ssh_service.rs"));
}
