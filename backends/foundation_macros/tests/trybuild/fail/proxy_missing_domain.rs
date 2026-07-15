//! Compile-fail: missing `domain` field.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        public_ip: "1.2.3.4",
        services: {},
    };
}
