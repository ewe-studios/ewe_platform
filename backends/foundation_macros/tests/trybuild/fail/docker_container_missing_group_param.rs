//! Compile-fail: the function declares no parameter to receive the containers.
use foundation_macros::docker_container;

#[docker_container(image = "redis:7-alpine", port = 6379)]
fn no_group_param() {}

fn main() {}
