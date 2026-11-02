use std::str::FromStr;

use cedar_policy::PolicySet;

use super::super::errors::CedarError;

/// Parse a policy set from text.
///
/// # Errors
/// Returns an error if the policy text does not parse.
pub fn parse_policies(text: &str) -> Result<PolicySet, CedarError> {
    PolicySet::from_str(text).map_err(|e| CedarError::PolicyParse(e.to_string()))
}
