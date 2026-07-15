//! Compile-fail: unknown health_check key `bad`.
use foundation_macros::proxy;

fn main() {
    let _config = proxy! {
        domain: "example.com",
        public_ip: "1.2.3.4",
        services: {
            app: {
                host: "app.example.com",
                backends: ["http://localhost:3000"],
                health_check: {
                    path: "/up",
                    interval: 5,
                    timeout: 2,
                    bad: 1,
                },
            },
        },
    };
}
