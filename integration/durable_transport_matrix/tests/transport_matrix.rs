//! The transport conformance matrix as a behaviour-driven development (BDD)
//! suite.
//!
//! One test function per Given/When/Then scenario. Each function runs the same
//! steps on the HTTP, WebSocket and `ConnectRPC` drivers. It compares every driver
//! with the model oracle first, then with the other drivers.

#![cfg(unix)]

use durable_transport_matrix::{
    model_scenario, run_scenario, Environment, HttpDriver, RpcDriver, Scenario, WsDriver,
};

use foundation_core::valtron::{block_on_future_scoped, initialize_pool};
use serial_test::serial;

/// Run one scenario on the model and on every driver, then compare.
fn assert_matrix(scenario: Scenario) {
    let environment = Environment::start();

    let expected = block_on_future_scoped(model_scenario(scenario));

    let mut http: HttpDriver = environment.http_driver();
    let http_log = block_on_future_scoped(run_scenario(&mut http, scenario));
    let mut ws: WsDriver = environment.ws_driver();
    let ws_log = block_on_future_scoped(run_scenario(&mut ws, scenario));
    let mut rpc: RpcDriver = environment.rpc_driver();
    let rpc_log = block_on_future_scoped(run_scenario(&mut rpc, scenario));

    drop(http);
    drop(ws);
    drop(rpc);
    environment.stop();

    assert_eq!(
        http_log,
        expected,
        "{}: the HTTP driver must match the model",
        scenario.name()
    );
    assert_eq!(
        ws_log,
        expected,
        "{}: the WebSocket driver must match the model",
        scenario.name()
    );
    assert_eq!(
        rpc_log,
        expected,
        "{}: the ConnectRPC driver must match the model",
        scenario.name()
    );
    assert_eq!(
        http_log,
        ws_log,
        "{}: HTTP and WebSocket must agree",
        scenario.name()
    );
    assert_eq!(
        ws_log,
        rpc_log,
        "{}: WebSocket and ConnectRPC must agree",
        scenario.name()
    );
}

/// The scenario list is the specification. A new transport adds a driver only.
#[test]
fn the_scenario_list_is_the_bdd_specification() {
    let scenarios = Scenario::all();
    assert_eq!(scenarios.len(), 6, "the suite has six scenarios");
    for scenario in scenarios {
        assert!(
            scenario.name().starts_with("given_"),
            "{} must be a Given/When/Then name",
            scenario.name()
        );
        assert!(
            scenario.name().contains("_when_") && scenario.name().contains("_then_"),
            "{} must carry Given, When and Then",
            scenario.name()
        );
    }
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_a_stream_when_it_is_created_and_appended_then_a_catch_up_read_returns_every_byte() {
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::CreateAppendRead);
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_two_appends_when_it_reads_from_the_first_next_offset_then_only_the_second_chunk_arrives() {
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::ReadFromFirstNextOffset);
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_a_closed_stream_when_it_reads_and_appends_then_the_read_reports_closed_and_the_append_is_refused(
) {
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::CloseReadAndRefusedAppend);
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_a_source_at_its_tail_when_a_fork_appends_then_the_fork_reads_the_prefix_and_its_own_chunk()
{
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::ForkAtTail);
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_a_missing_stream_when_it_reads_then_the_read_reports_not_found() {
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::ReadMissing);
}

#[test]
#[foundation_macros::timeout(120_000)]
#[serial(durable_transport_matrix)]
fn given_a_producer_when_it_retries_the_same_sequence_then_the_stream_holds_one_chunk_and_reports_a_duplicate(
) {
    let _guard = initialize_pool(42, Some(8));
    assert_matrix(Scenario::ProducerRetry);
}
