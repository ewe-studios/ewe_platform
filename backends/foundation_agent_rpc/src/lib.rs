//! `foundation_agent_rpc` — the per-machine agent RPC model server.
//!
//! WHY: launching one binary on a host should start an RPC server that serves
//! many agent sessions against a model loaded **once** for the whole machine.
//!
//! WHAT: crate root. Modules land task by task (see
//! `specifications/65-agent-rpc-serving-system/features/01-agent-rpc`).
