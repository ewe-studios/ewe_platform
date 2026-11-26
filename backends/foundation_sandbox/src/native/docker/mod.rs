//! The Docker driver, the container control and the Engine calls (feature 10).
//!
//! - [`api`] is the trait over the Docker Engine calls and its real
//!   implementation on `DockerClient`.
//! - [`driver`] is `DockerDriver`: the launch state machine, the create body
//!   and the substrate handle.
//! - [`control`] is `DockerContainerControl`: a container with no sandbox
//!   record (feature 00, section 6.13).
//!
//! The module is native only: a container needs a Docker Engine and a host.

pub mod api;
pub mod control;
pub mod driver;

pub use api::{DockerApi, LABEL_CONTAINER_CONTROL, LABEL_SANDBOX_DRIVER, LABEL_SANDBOX_ID};
pub use control::DockerContainerControl;
pub use driver::{DockerDriver, DockerDriverConfig, DockerLink, SlaveMode};
