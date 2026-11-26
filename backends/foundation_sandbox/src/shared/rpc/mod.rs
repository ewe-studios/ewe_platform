//! The two RPC service definitions.
//!
//! `SlaveService` runs in the sandbox. `SandboxService` runs in the control
//! plane, local or remote. Every call is unary or server-streaming; no call
//! streams its request.
//!
//! Each service lives in its own module because the `#[service]` macro emits a
//! `procedure` module at the same scope. The names are re-exported here, so a
//! caller uses one path.

pub mod messages;

mod sandbox;
/// The slave service. The module is public so the server reads the generated
/// procedure paths of [`slave::procedure`].
pub mod slave;

pub use sandbox::{
    register_sandbox_service, SandboxService, SandboxServiceClient, SandboxServiceClientExt,
    UnimplementedSandboxServiceHandler,
};
pub use slave::{
    register_slave_service, SlaveService, SlaveServiceClient, SlaveServiceClientExt,
    UnimplementedSlaveServiceHandler,
};
