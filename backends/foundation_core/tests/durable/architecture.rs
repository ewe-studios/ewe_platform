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

/// The core reader and producer tests exercise the core API only.
///
/// The crate has no feature that removes the valtron module. This guard proves
/// the closest available rule: the core tests never name the valtron wrapper.
#[test]
fn core_reader_and_producer_tests_use_no_valtron_wrapper() {
    for (name, source) in [
        ("reader.rs", include_str!("reader.rs")),
        ("producer.rs", include_str!("producer.rs")),
    ] {
        for banned in [
            "durable::valtron",
            "DurableStreamReader",
            "DurableStreamWriter",
            "producer_task",
            "WakeableStream",
        ] {
            assert!(!source.contains(banned), "{name} must not name {banned}");
        }
    }
}
