//! Compile-fail: missing `public_ip` field.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        domain: "example.com",
        services: {},
    };
}
