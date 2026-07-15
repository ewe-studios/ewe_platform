//! Compile-fail: unknown SSL provider `bad_provider`.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        ssl: bad_provider {
            key: "value",
        },
        services: {},
    };
}
