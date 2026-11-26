//! A Cloudflare route on a generated procedure path matches the dispatch.
//!
//! The agent-rpc Worker registers one route per `AgentService` procedure. The
//! service segment has dots, so this case pins that a dotted static segment
//! matches.

use foundation_http::shared::router::Router;
use foundation_netio::shared::http::SimpleMethod;

#[test]
fn an_absolute_url_keeps_the_procedure_path() {
    let uri = foundation_core::url::Uri::parse(
        "http://127.0.0.1:63794/ewe.agent_rpc.v1.AgentService/Health",
    )
    .expect("parse the URL");
    assert_eq!(uri.path(), "/ewe.agent_rpc.v1.AgentService/Health");
}

#[test]
fn every_procedure_route_matches_after_a_shared_prefix_merge() {
    const PATHS: &[&str] = &[
        "/ewe.agent_rpc.v1.AgentService/Health",
        "/ewe.agent_rpc.v1.AgentService/GetCapabilities",
        "/ewe.agent_rpc.v1.AgentService/CreateInstance",
        "/ewe.agent_rpc.v1.AgentService/ListInstances",
        "/ewe.agent_rpc.v1.AgentService/DestroyInstance",
        "/ewe.agent_rpc.v1.AgentService/Chat",
        "/ewe.agent_rpc.v1.AgentService/ChatStream",
    ];
    let mut router: Router<()> = Router::new();
    for path in PATHS {
        router.add_route_any_cf(path, &());
    }
    for path in PATHS {
        assert!(
            router.dispatch_result(&SimpleMethod::POST, path).is_ok(),
            "the procedure {path} must match after the shared prefix"
        );
    }
}

#[test]
fn a_dotted_procedure_path_matches() {
    let mut router: Router<()> = Router::new();
    router.add_route_any_cf("/ewe.agent_rpc.v1.AgentService/Health", &());
    assert!(
        router
            .dispatch_result(&SimpleMethod::POST, "/ewe.agent_rpc.v1.AgentService/Health")
            .is_ok(),
        "the generated procedure path must match"
    );
    assert!(
        router
            .dispatch_result(
                &SimpleMethod::POST,
                "/ewe.agent_rpc.v1.AgentService/Unknown"
            )
            .is_err(),
        "another procedure must not match"
    );
}
