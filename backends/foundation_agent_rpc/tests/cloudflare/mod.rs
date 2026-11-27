//! The tests of the Cloudflare host (spec 83 feature 06).
//!
//! The container and the plan tests run on a native host with test doubles.
//! The orchestrator naming test runs on every host. The end-to-end cases need
//! a deployed Worker and read their URL and token from the environment.

/// The agent-rpc container over the spec 77 handle (test cases 4 and 5).
#[cfg(feature = "sandbox")]
mod container;

/// The agent-rpc deployment plan (test case 3). The plan needs the native
/// spec 77 deployers.
#[cfg(all(not(target_family = "wasm"), feature = "deployment"))]
mod plan;

/// The thin orchestrator and the scope-in-name rule (test cases 1, 2 and 6,
/// and issue #987).
mod orchestrator;
