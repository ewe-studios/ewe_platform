//! Generated wire types (`proto/ewe/agent_rpc/v1/agent.proto`).
//!
//! WHY: the RPC contract is defined once in protobuf and generated — message
//! types by `buffa-build`, service traits/clients by
//! `foundation_connectrpc_codegen` (see `build.rs`).
//!
//! WHAT: [`v1`] holds the messages; [`v1::agent_service`] and
//! [`v1::bootstrap_service`] hold each service's trait, `register_*` function,
//! typed client and procedure paths.
//!
//! HOW: `include!` of the `OUT_DIR` files. The generated oneof code refers to
//! messages as `super::super::super::X`, so the package file must be included
//! directly inside `v1`.

/// `ewe.agent_rpc.v1`.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    rustdoc::all
)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/ewe.agent_rpc.v1.mod.rs"));

    /// `AgentService`: health, instance lifecycle, chat.
    pub mod agent_service {
        #[allow(unused_imports)]
        use super::*;
        include!(concat!(env!("OUT_DIR"), "/agent_service.rs"));
    }

    /// `BootstrapService`: bootstrap token → client certificate.
    pub mod bootstrap_service {
        #[allow(unused_imports)]
        use super::*;
        include!(concat!(env!("OUT_DIR"), "/bootstrap_service.rs"));
    }
}
