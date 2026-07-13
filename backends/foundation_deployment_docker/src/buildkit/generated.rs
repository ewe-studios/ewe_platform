//! Generated BuildKit gRPC message types — module tree glue.
//!
//! WHY: `buffa-build` (run from `build.rs` under the `buildkit` feature) emits
//! one file per proto package into `OUT_DIR`. The generated code cross-references
//! sibling packages with relative paths (e.g. `super::super::super::pb::Op`), so
//! the packages must be nested to mirror the proto package hierarchy.
//!
//! WHAT: This module *is* the root of that tree. From `moby::buildkit::v1`,
//! three `super`s land back here where `pb`, `google`, and `moby` are siblings —
//! exactly what the generated code expects.
//!
//! HOW: Each `include!` pulls a package's `<pkg>.mod.rs` entry file, which itself
//! `include!`s the concrete type / view / oneof files from the same `OUT_DIR`.
//! DO NOT rearrange the nesting — the generated relative paths depend on it.
#![allow(
    clippy::all,
    clippy::pedantic,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    unused_imports,
    unused_qualifications,
    rustdoc::all
)]

/// `pb` — BuildKit LLB solver operations (`solver/pb/ops.proto`).
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/pb.mod.rs"));
}

/// `google.rpc` — the `Status` well-known error type (`google/rpc/status.proto`).
pub mod google {
    pub mod rpc {
        include!(concat!(env!("OUT_DIR"), "/google.rpc.mod.rs"));
    }
}

/// `moby.buildkit.v1` — the Control service messages plus its `types` and
/// `sourcepolicy` sub-packages.
pub mod moby {
    pub mod buildkit {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/moby.buildkit.v1.mod.rs"));

            /// `moby.buildkit.v1.types` — worker records (`api/types/worker.proto`).
            pub mod types {
                include!(concat!(env!("OUT_DIR"), "/moby.buildkit.v1.types.mod.rs"));
            }

            /// `moby.buildkit.v1.sourcepolicy` — source policy
            /// (`sourcepolicy/pb/policy.proto`).
            pub mod sourcepolicy {
                include!(concat!(env!("OUT_DIR"), "/moby.buildkit.v1.sourcepolicy.mod.rs"));
            }
        }
    }
}

// ── Ergonomic re-exports of the Control-service RPC message types ────────────
pub use moby::buildkit::v1::{
    InfoRequest, InfoResponse, SolveRequest, SolveResponse, StatusRequest, StatusResponse,
};
