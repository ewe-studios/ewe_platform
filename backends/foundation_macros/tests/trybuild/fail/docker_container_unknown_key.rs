//! Compile-fail: unknown attribute key is rejected, spanned on the key itself.
//!
//! The parse error is raised before any code is generated, so the annotated fn
//! is replaced wholesale by the error — its parameter type is never resolved,
//! and this file needs no `foundation_deployment_platform` dependency (which
//! could not be one anyway: that crate depends on this one).
use foundation_macros::docker_container;

#[docker_container(image = "redis:7-alpine", bogus_key = "x")]
fn unknown_key(_: ()) {}

fn main() {}
