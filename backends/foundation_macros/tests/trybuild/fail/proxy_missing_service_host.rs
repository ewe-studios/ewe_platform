//! Compile-fail: missing required field `host` in a service block.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        services: {
            app: {
                backends: ["http://localhost:3000"],
            },
        },
    };
}
