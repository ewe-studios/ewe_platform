use std::str::FromStr;

use cedar_policy::PolicySet;

use super::super::errors::CedarError;

pub fn parse_policies(text: &str) -> Result<PolicySet, CedarError> {
    PolicySet::from_str(text).map_err(|e| CedarError::PolicyParse(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_policy() {
        let text = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
);
"#;
        assert!(parse_policies(text).is_ok());
    }

    #[test]
    fn test_parse_invalid_policy() {
        assert!(parse_policies("invalid{{{").is_err());
    }

    #[test]
    fn test_parse_multiple_policies() {
        let text = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource
);
forbid(
    principal == User::"eve",
    action,
    resource
);
"#;
        let ps = parse_policies(text).unwrap();
        assert_eq!(ps.policies().count(), 2);
    }
}
