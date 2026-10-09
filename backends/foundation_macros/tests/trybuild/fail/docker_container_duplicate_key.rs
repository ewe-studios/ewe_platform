//! Compile-fail: two containers sharing one `as` key would make lookup
//! ambiguous. The macro sees the whole set, so this is caught at compile time
//! rather than panicking once the containers are already running.
use foundation_macros::docker_container;

#[docker_container(
    { image = "redis:7-alpine", as = "cache" },
    { image = "redis:7-alpine", as = "cache" }
)]
fn duplicate_key(_: ()) {}

fn main() {}
