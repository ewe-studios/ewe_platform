//! Compile-fail: unknown top-level key `foo`.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        foo: "bar",
        services: {},
    };
}
