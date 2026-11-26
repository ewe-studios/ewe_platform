//! Architecture oracles for the transport-neutral core.

/// The core has no valtron, HTTP or socket type. The source proves it.
#[test]
fn given_the_durable_core_when_read_then_it_names_no_valtron_or_http_type() {
    let sources = [
        include_str!("../../src/durable/shared/mod.rs"),
        include_str!("../../src/durable/shared/service.rs"),
        include_str!("../../src/durable/shared/memory.rs"),
        include_str!("../../src/durable/shared/cache.rs"),
        include_str!("../../src/durable/shared/wire.rs"),
    ];
    for source in sources {
        let lower = source.to_ascii_lowercase();
        // Prose may name the layers. Code must not reach into them.
        for banned in [
            "::valtron",
            "use crate::valtron",
            "foundation_http",
            "hyper::",
            "tokio::",
        ] {
            assert!(
                !lower.contains(banned),
                "the durable core must not name {banned}"
            );
        }
    }
}
