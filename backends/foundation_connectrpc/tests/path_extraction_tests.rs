//! The router path of a request target (issue #948).
//!
//! A native connection names a path; a wasm bridge names the whole URL. Both
//! must reach the router as `/service/Method`.

use foundation_connectrpc::extract_path;

#[test]
fn a_path_with_a_query_keeps_the_path() {
    assert_eq!(
        extract_path("/ewe.agent_rpc.v1.AgentService/Health?x=1"),
        "/ewe.agent_rpc.v1.AgentService/Health"
    );
}

#[test]
fn an_absolute_url_gives_its_path() {
    assert_eq!(
        extract_path("https://agents.example.com/ewe.agent_rpc.v1.AgentService/Health"),
        "/ewe.agent_rpc.v1.AgentService/Health"
    );
    assert_eq!(
        extract_path("http://127.0.0.1:8787/ewe.agent_rpc.v1.AgentService/ChatStream?x=1"),
        "/ewe.agent_rpc.v1.AgentService/ChatStream"
    );
}

#[test]
fn an_url_without_a_path_is_the_root() {
    assert_eq!(extract_path("https://agents.example.com"), "/");
    assert_eq!(extract_path("https://agents.example.com?x=1"), "/");
}

#[test]
fn a_root_path_stays_the_root() {
    assert_eq!(extract_path("/"), "/");
}
