//! T2 — the crate's single error type and its wire mapping.
//!
//! WHY: every RPC failure must reach the client as a machine-readable status
//! code, never a bare string. `AgentRpcError` is the one error enum; these
//! tests pin each variant's Connect/gRPC code.

use foundation_agent_rpc::AgentRpcError;
use foundation_connectrpc::{Code, ConnectError};

#[test]
fn every_variant_maps_to_a_machine_readable_code() {
    let cases = [
        (AgentRpcError::ModelLoad("x".into()), Code::Internal),
        (AgentRpcError::ModelNotFound("m".into()), Code::NotFound),
        (
            AgentRpcError::Auth("no token".into()),
            Code::Unauthenticated,
        ),
        (AgentRpcError::InstanceNotFound("a".into()), Code::NotFound),
        (
            AgentRpcError::InstanceExists("a".into()),
            Code::AlreadyExists,
        ),
        (
            AgentRpcError::InvalidName("../x".into()),
            Code::InvalidArgument,
        ),
        (AgentRpcError::InstanceBusy("a".into()), Code::Aborted),
        (AgentRpcError::InstanceLimit(4), Code::ResourceExhausted),
        (AgentRpcError::Inference("boom".into()), Code::Internal),
        (AgentRpcError::Codec("bad".into()), Code::InvalidArgument),
        (
            AgentRpcError::Config("bad".into()),
            Code::FailedPrecondition,
        ),
        (AgentRpcError::Transport("down".into()), Code::Unavailable),
        (AgentRpcError::Shutdown, Code::Unavailable),
    ];
    for (err, code) in cases {
        assert_eq!(err.code(), code, "{err}");
        let wire = ConnectError::from(err);
        assert_eq!(wire.code(), code);
        assert!(!wire.message().is_empty(), "wire errors carry a message");
    }
}

#[test]
fn display_names_the_subject() {
    let err = AgentRpcError::InstanceNotFound("alpha".into());
    assert_eq!(err.to_string(), "instance not found: alpha");
}
