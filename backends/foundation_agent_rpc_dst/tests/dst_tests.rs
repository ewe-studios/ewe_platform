//! Deterministic simulation tests: the flow, 32 fixed seeds and an injected
//! storage failure.
//!
//! Every test prints its seed on failure, so the same run can be replayed.

use std::time::Duration;

use foundation_agent_rpc_dst::{text_of, TestWorld};
use foundation_core::valtron::sim::SimRuntime;
use foundation_db::testing::FaultStep;

/// The start of the simulated clock.
const START: Duration = Duration::from_secs(1_700_000_000);

/// One run's observable trace. It holds the seeded client identity and the
/// words the scripted model said through the real session turn.
fn trace(seed: u64) -> String {
    SimRuntime::run(seed, START, |sim| {
        let world = TestWorld::in_memory(&sim);
        let enrolled = world.enroll("laptop").expect("the client enrolls");
        let client = world.client_with(&enrolled.access_secret);

        let health = client.health().expect("health").status;
        let info = client.create("alpha").expect("create");
        let events = client.chat_stream("alpha", "hello").expect("the turn runs");
        let text = text_of(&events);
        client.destroy("alpha").expect("destroy");
        let list = client.list().expect("list the instances");

        format!(
            "client_id={} access={} health={} instance={} text={:?} events={} left={}",
            enrolled.client_id,
            enrolled.access_secret,
            health,
            info.name,
            text,
            events.len(),
            list.instances.len()
        )
    })
}

#[test]
fn dst_create_enroll_one_turn_destroy_from_one_seed() {
    SimRuntime::run(42, START, |sim| {
        let world = TestWorld::in_memory(&sim);

        // Enroll: an enrollment secret and a client key become an access secret.
        let enrolled = world.enroll("laptop").expect("the client enrolls");
        assert!(!enrolled.client_id.is_empty(), "seed 42");
        assert!(!enrolled.access_secret.is_empty(), "seed 42");

        let client = world.client_with(&enrolled.access_secret);
        assert_eq!(
            client.health().expect("health").status,
            "serving",
            "seed 42"
        );

        let info = client.create("alpha").expect("create");
        assert_eq!(info.name, "alpha", "seed 42");

        let events = client.chat_stream("alpha", "hello").expect("the turn runs");
        assert_eq!(text_of(&events), "Hi there", "seed 42");
        assert!(
            events.iter().any(|event| matches!(
                event.event,
                Some(foundation_agent_rpc::proto::v1::__buffa::oneof::turn_event::Event::Done(_))
            )),
            "the turn ends with Done (seed 42)"
        );

        client.destroy("alpha").expect("destroy");
        assert!(
            client.list().expect("list").instances.is_empty(),
            "no instance remains (seed 42)"
        );
    });
}

#[test]
fn dst_same_seed_gives_identical_traces_for_32_seeds() {
    for seed in 1..=32_u64 {
        let first = trace(seed);
        let second = trace(seed);
        assert_eq!(first, second, "seed {seed}");
        assert!(first.contains("text=\"Hi there\""), "seed {seed}: {first}");
    }
}

#[test]
fn dst_can_enroll_many_clients_from_one_seed() {
    SimRuntime::run(7, START, |sim| {
        let world = TestWorld::in_memory(&sim);
        let laptop = world.enroll("laptop").expect("laptop enrolls");
        let phone = world.enroll("phone").expect("phone enrolls");
        assert_ne!(
            laptop.client_id, phone.client_id,
            "each enrollment gets its own seeded id"
        );
        assert!(
            world.client_with(&phone.access_secret).health().is_ok(),
            "the second client also calls"
        );
    });
}

#[test]
fn dst_injected_storage_failure_gives_a_typed_error() {
    SimRuntime::run(3, START, |sim| {
        let world = TestWorld::with_faulty_storage(&sim, FaultStep::Append);
        let client = world.client();
        let error = client
            .create("alpha")
            .expect_err("the injected store failure ends the create");
        assert_eq!(
            error.current_context().code(),
            foundation_connectrpc::Code::Internal,
            "seed 3: {error:?}"
        );
        assert!(
            error
                .current_context()
                .to_string()
                .contains("injected storage failure at append call 1"),
            "seed 3: {error:?}"
        );
        assert!(
            client.list().expect("list").instances.is_empty(),
            "no instance remains (seed 3)"
        );
    });
}
