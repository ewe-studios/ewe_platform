//! The common API types are one type, not a copy.
//!
//! `foundation_deployment::providers::common` re-exports
//! `foundation_api_common` (spec 77 feature 11). A value of one path is a value
//! of the other, so a generated client and a handler that name different paths
//! still compile together.

use foundation_deployment::providers::common as deployment_common;

/// The two paths name the same type.
#[test]
fn common_reexport_is_same_type() {
    fn limits_of(client: foundation_api_common::ApiClient) -> foundation_api_common::BodyLimits {
        client.limits()
    }

    let http = foundation_netio::HttpClientBuilder::new().build();
    let client: deployment_common::ApiClient = http.into();
    let limits: foundation_api_common::BodyLimits = limits_of(client);
    assert_eq!(limits.max_error_body, 64 * 1024);

    let from_deployment: deployment_common::BodyLimits = deployment_common::BodyLimits::default();
    let _same: foundation_api_common::BodyLimits = from_deployment;

    let _empty: foundation_api_common::Empty = deployment_common::Empty {
        data: std::collections::HashMap::new(),
    };
}
