//! Type resolution logic for OpenAPI schemas.
//!
//! WHY: Clear rules for what constitutes a "generatable" type vs what should be
//! `serde_json::Value`.
//!
//! WHAT: TypeResolver struct with schema lookup and resolution logic.
//!
//! HOW: Analyzes schema structure to determine if a type should be generated
//! as a struct or handled as raw JSON.

use crate::spec::{Response, Schema};
use crate::endpoint::ResponseType;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Resolver for OpenAPI schema types.
pub struct TypeResolver {
    /// Known object schemas (for $ref validation)
    object_schemas: BTreeSet<String>,
    /// All schemas by name
    schemas: Arc<BTreeMap<String, Schema>>,
}

impl TypeResolver {
    /// Create resolver with schema map.
    pub fn new(schemas: Arc<BTreeMap<String, Schema>>) -> Self {
        let object_schemas = Self::collect_object_schema_names(&schemas);
        Self {
            object_schemas,
            schemas,
        }
    }

    /// Collect schema names that are object types with properties.
    ///
    /// Used to validate `$ref` targets: if a ref points to a non-object schema,
    /// the field should fall back to `serde_json::Value`.
    fn collect_object_schema_names(schemas: &Arc<BTreeMap<String, Schema>>) -> BTreeSet<String> {
        let mut names = BTreeSet::new();

        for (name, value) in schemas.iter() {
            // Include all object types and composition types
            let has_composition = value.all_of.is_some()
                || value.one_of.is_some()
                || value.any_of.is_some();
            let is_object = value.schema_type.as_deref() == Some("object");

            // Also include response types (they may be allOf-refs-only but are still needed)
            let is_response = name.to_lowercase().contains("response");

            // Also include request types
            let is_request = name.to_lowercase().contains("request");

            if is_object || has_composition || is_response || is_request {
                names.insert(name.clone());
            }
        }

        names
    }

    /// Resolve a `$ref` to a type name.
    ///
    /// Handles both OpenAPI (`#/components/schemas/`) and GCP (`#/schemas/`) formats.
    pub fn resolve_ref(&self, ref_path: &str) -> Option<String> {
        let ref_name = ref_path
            .trim_start_matches("#/components/schemas/")
            .trim_start_matches("#/schemas/");

        if self.object_schemas.contains(ref_name) {
            let ty = Self::to_pascal_case(ref_name);
            Some(Self::rename_if_keyword(ty))
        } else {
            None
        }
    }

    /// Check if a schema is "generatable" (has properties or simple structure).
    ///
    /// Returns false for composition types (oneOf/anyOf) and empty objects.
    pub fn is_generatable(&self, schema: &Schema) -> bool {
        // Has explicit properties - always generatable
        if schema.properties.is_some() {
            return true;
        }

        // Check composition types
        if let Some(all_of) = &schema.all_of {
            // allOf with single $ref is a wrapper - generatable
            if all_of.len() == 1 && all_of[0].ref_path.is_some() {
                return true;
            }
            // allOf with multiple members - not generatable (use JsonValue)
            return false;
        }

        // Union types are not generatable
        if schema.one_of.is_some() || schema.any_of.is_some() {
            return false;
        }

        // Plain object with no properties - not generatable (use JsonValue)
        if schema.schema_type.as_deref() == Some("object") {
            return false;
        }

        // Has a $ref - check if target is generatable
        if let Some(ref_path) = &schema.ref_path {
            return self.resolve_ref(ref_path).is_some();
        }

        // Unknown/inline schema - not generatable
        false
    }

    /// Get the response type for a response object.
    ///
    /// Handles composition types, $refs, and inline schemas.
    pub fn get_response_type(&self, response: &Response) -> Option<ResponseType> {
        let content = response.content.as_ref()?;
        let media_type = content.get("application/json")?;
        let schema = media_type.schema.as_ref()?;

        // Check if this is a generatable type
        let schema_to_check = if schema.ref_path.is_some()
            && schema.properties.is_none()
            && schema.any_of.is_none()
            && schema.one_of.is_none()
        {
            // It's a $ref, look up the actual schema
            schema.ref_path.as_ref()
                .and_then(|ref_path| {
                    let type_name = ref_path.trim_start_matches("#/components/schemas/");
                    let type_name = type_name.trim_start_matches("#/schemas/");
                    self.schemas.get(type_name)
                })
                .unwrap_or(schema)
        } else {
            schema
        };

        // Check if the type is generatable
        let is_generatable = schema_to_check.properties.is_some()
            || (schema_to_check.all_of.is_none()
                && schema_to_check.any_of.is_none()
                && schema_to_check.one_of.is_none());

        if is_generatable {
            // Extract type name from $ref
            if let Some(ref_path) = &schema.ref_path {
                let type_name = ref_path
                    .trim_start_matches("#/components/schemas/")
                    .trim_start_matches("#/schemas/");
                let normalized = Self::to_pascal_case(type_name);
                Some(ResponseType::Generated(Self::rename_if_keyword(normalized)))
            } else {
                // Inline schema with properties - needs a name
                None
            }
        } else {
            Some(ResponseType::JsonValue)
        }
    }

    /// Normalize type name to Rust PascalCase.
    ///
    /// Handles various naming conventions:
    /// - "treasury.transaction" → "TreasuryTransaction"
    /// - "Custom-pages" → "CustomPages"
    /// - "@cf_ai4bharat.translation" → "CfAi4bharatTranslation"
    pub fn to_pascal_case(name: &str) -> String {
        name.split(|c| c == '.' || c == '-' || c == '@' || c == '_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>())
                    .unwrap_or_default()
                    + chars.as_str()
            })
            .collect()
    }

    /// Rename if the type name conflicts with Rust keywords/builtins.
    pub fn rename_if_keyword(name: String) -> String {
        match name.as_str() {
            "Option" => "ApiOption".to_string(),
            "Value" => "ApiValue".to_string(),
            "Result" => "ApiResult".to_string(),
            "Ok" => "ApiOk".to_string(),
            "Err" => "ApiErr".to_string(),
            "Some" => "ApiSome".to_string(),
            "None" => "ApiNone".to_string(),
            "Box" => "ApiBox".to_string(),
            "Vec" => "ApiVec".to_string(),
            "String" => "ApiString".to_string(),
            _ => name,
        }
    }

    /// Convert identifier to snake_case.
    pub fn to_snake_case(name: &str) -> String {
        let mut parts = Vec::new();
        let mut current = String::new();

        let chars: Vec<char> = name.chars().collect();
        for i in 0..chars.len() {
            let c = chars[i];
            if !c.is_alphanumeric() {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            } else if c.is_uppercase() {
                if !current.is_empty() {
                    let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
                    if next_is_lower || current.chars().last().map_or(false, |p| p.is_lowercase()) {
                        parts.push(current.clone());
                        current.clear();
                    }
                }
                current.push(c.to_ascii_lowercase());
            } else {
                current.push(c);
            }
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts.join("_")
    }

    /// Get all generatable type names.
    pub fn generatable_types(&self) -> Vec<String> {
        self.schemas
            .keys()
            .filter(|name| {
                if let Some(schema) = self.schemas.get(*name) {
                    self.is_generatable(schema)
                } else {
                    false
                }
            })
            .map(|name| Self::rename_if_keyword(Self::to_pascal_case(name)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema(properties: Option<BTreeMap<String, Schema>>) -> Schema {
        Schema {
            schema_type: Some("object".to_string()),
            properties,
            ..Default::default()
        }
    }

    #[test]
    fn normalizes_dotted_names() {
        assert_eq!(TypeResolver::to_pascal_case("treasury.transaction"), "TreasuryTransaction");
    }

    #[test]
    fn normalizes_hyphenated_names() {
        assert_eq!(TypeResolver::to_pascal_case("Custom-pages"), "CustomPages");
    }

    #[test]
    fn normalizes_snake_case_names() {
        assert_eq!(TypeResolver::to_pascal_case("iam_response_collection"), "IamResponseCollection");
    }

    #[test]
    fn normalizes_at_prefixed_names() {
        assert_eq!(TypeResolver::to_pascal_case("@cf_ai4bharat.translation"), "CfAi4bharatTranslation");
    }

    #[test]
    fn renames_rust_keywords() {
        assert_eq!(TypeResolver::rename_if_keyword("Option".to_string()), "ApiOption");
        assert_eq!(TypeResolver::rename_if_keyword("Value".to_string()), "ApiValue");
        assert_eq!(TypeResolver::rename_if_keyword("Result".to_string()), "ApiResult");
        assert_eq!(TypeResolver::rename_if_keyword("MyType".to_string()), "MyType");
    }

    #[test]
    fn converts_to_snake_case() {
        assert_eq!(TypeResolver::to_snake_case("getV1Projects"), "get_v1_projects");
        assert_eq!(TypeResolver::to_snake_case("GetV1Projects"), "get_v1_projects");
    }

    #[test]
    fn is_generatable_with_properties() {
        let schemas = Arc::new(BTreeMap::new());
        let resolver = TypeResolver::new(schemas);

        let mut props = BTreeMap::new();
        props.insert("name".to_string(), Schema {
            schema_type: Some("string".to_string()),
            ..Default::default()
        });

        let schema = make_schema(Some(props));
        assert!(resolver.is_generatable(&schema));
    }

    #[test]
    fn is_generatable_with_single_allof_ref() {
        let schemas = Arc::new(BTreeMap::new());
        let resolver = TypeResolver::new(schemas);

        let schema = Schema {
            all_of: Some(vec![Schema {
                ref_path: Some("#/components/schemas/BaseType".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        assert!(resolver.is_generatable(&schema));
    }

    #[test]
    fn is_not_generatable_with_oneof() {
        let schemas = Arc::new(BTreeMap::new());
        let resolver = TypeResolver::new(schemas);

        let schema = Schema {
            one_of: Some(vec![
                Schema { ref_path: Some("#/components/schemas/A".to_string()), ..Default::default() },
                Schema { ref_path: Some("#/components/schemas/B".to_string()), ..Default::default() },
            ]),
            ..Default::default()
        };

        assert!(!resolver.is_generatable(&schema));
    }
}
