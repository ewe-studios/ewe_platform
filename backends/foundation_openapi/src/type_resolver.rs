//! Type resolution logic for `OpenAPI` schemas.
//!
//! WHY: Clear rules for what constitutes a "generatable" type vs what should be
//! `serde_json::Value`.
//!
//! WHAT: `TypeResolver` struct with schema lookup and resolution logic.
//!
//! HOW: Analyzes schema structure to determine if a type should be generated
//! as a struct or handled as raw JSON.

use crate::endpoint::ResponseType;
use crate::spec::{Response, Schema};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Resolver for `OpenAPI` schema types.
pub struct TypeResolver {
    /// Known object schemas (for $ref validation)
    object_schemas: BTreeSet<String>,
    /// All schemas by name
    schemas: Arc<BTreeMap<String, Schema>>,
}

impl TypeResolver {
    /// Create resolver with schema map.
    #[must_use]
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
            let has_composition =
                value.all_of.is_some() || value.one_of.is_some() || value.any_of.is_some();
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
    /// Handles both `OpenAPI` (`#/components/schemas/`) and GCP (`#/schemas/`) formats.
    #[must_use]
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

    /// Resolve a request body type, handling arrays by wrapping in Vec<>.
    ///
    /// For array types, returns `Vec<ItemType>` where ItemType is resolved from the items schema.
    /// For object types, returns the type name directly.
    /// For non-generatable types, returns None.
    #[must_use]
    pub fn resolve_request_body_type(&self, schema: &Schema) -> Option<String> {
        // If schema has a $ref, first try to resolve it directly
        if let Some(ref_path) = &schema.ref_path {
            let ref_name = ref_path
                .trim_start_matches("#/components/schemas/")
                .trim_start_matches("#/schemas/");

            // Check if this is an array type
            if let Some(ref_schema) = self.schemas.get(ref_name) {
                if ref_schema.schema_type.as_deref() == Some("array") {
                    // Handle array type - wrap item type in Vec<>
                    return self.resolve_array_items_type(&ref_schema.items);
                }
                // Not an array, use normal resolution
                return self.resolve_ref(ref_path);
            }
        }

        // No $ref or ref didn't resolve - check schema type directly
        if schema.schema_type.as_deref() == Some("array") {
            return self.resolve_array_items_type(&schema.items);
        }

        // Object type - resolve normally
        if schema.ref_path.is_some() {
            return self.resolve_ref(schema.ref_path.as_ref().unwrap());
        }

        // Inline schema with properties
        if schema.properties.as_ref().is_some_and(|p| !p.is_empty()) {
            // Inline schemas don't have a name, so we can't generate a type
            None
        } else {
            None
        }
    }

    /// Resolve the item type for an array schema, wrapping in Vec<>.
    fn resolve_array_items_type(&self, items: &Option<Box<Schema>>) -> Option<String> {
        let items_schema = items.as_ref()?;

        // Items is a $ref to a named type
        if let Some(ref_path) = &items_schema.ref_path {
            let item_type = self.resolve_ref(ref_path)?;
            Some(format!("Vec<{item_type}>"))
        }
        // Items is an inline object with properties - generate Vec<serde_json::Value>
        else if items_schema
            .properties
            .as_ref()
            .is_some_and(|p| !p.is_empty())
        {
            Some("Vec<serde_json::Value>".to_string())
        }
        // Items is a primitive type (e.g., string)
        else if let Some(type_name) = &items_schema.schema_type {
            let rust_type = match type_name.as_str() {
                "string" => "String",
                "integer" => "i64",
                "number" => "f64",
                "boolean" => "bool",
                _ => "serde_json::Value",
            };
            Some(format!("Vec<{rust_type}>"))
        } else {
            Some("Vec<serde_json::Value>".to_string())
        }
    }

    /// Check if a schema is "generatable" (has properties or simple structure).
    ///
    /// Returns false for composition types (oneOf/anyOf) and empty objects.
    #[must_use]
    pub fn is_generatable(&self, schema: &Schema) -> bool {
        Self::check_schema_generatable(schema, self.schemas.as_ref())
    }

    /// Check if a schema is generatable (can be converted to a Rust struct).
    ///
    /// Handles allOf by merging properties from all members.
    fn check_schema_generatable(schema: &Schema, schemas: &BTreeMap<String, Schema>) -> bool {
        // Has explicit properties at top level - generatable
        if schema.properties.as_ref().is_some_and(|p| !p.is_empty()) {
            return true;
        }

        // Handle allOf - merge properties from all members
        if let Some(all_of) = &schema.all_of {
            // Check if any member has properties
            for member in all_of {
                if member.properties.as_ref().is_some_and(|p| !p.is_empty()) {
                    return true;
                }
                // Recursively check nested allOf
                if member.all_of.is_some() && Self::check_schema_generatable(member, schemas) {
                    return true;
                }
            }
            // allOf with only $refs (no properties) - still generatable as it extends a base type
            if all_of
                .iter()
                .all(|m| m.ref_path.is_some() && m.properties.is_none())
            {
                return true;
            }
            return false;
        }

        // oneOf/anyOf are not generatable (union types)
        if schema.one_of.is_some() || schema.any_of.is_some() {
            return false;
        }

        // Simple $ref is generatable if the referenced type exists
        if let Some(ref_path) = &schema.ref_path {
            let type_name = ref_path
                .trim_start_matches("#/components/schemas/")
                .trim_start_matches("#/schemas/");
            return schemas.contains_key(type_name);
        }

        // Unknown/inline schema without properties - not generatable
        false
    }

    /// Get the response type for a response object.
    ///
    /// Handles composition types, $refs, and inline schemas.
    #[must_use]
    pub fn get_response_type(&self, response: &Response) -> Option<ResponseType> {
        let content = response.content.as_ref()?;
        let media_type = content.get("application/json")?;
        let schema = media_type.schema.as_ref()?;

        // Check if this is a generatable type
        // For allOf types, we need to check if any member has properties or if it's a simple ref wrapper
        let is_generatable = Self::check_schema_generatable(schema, self.schemas.as_ref());

        if is_generatable {
            // Extract type name from $ref or use the schema directly
            if let Some(ref_path) = &schema.ref_path {
                let type_name = ref_path
                    .trim_start_matches("#/components/schemas/")
                    .trim_start_matches("#/schemas/");
                let normalized = Self::to_pascal_case(type_name);
                Some(ResponseType::Generated(Self::rename_if_keyword(normalized)))
            } else if schema.all_of.is_some() && schema.ref_path.is_none() {
                // allOf type without top-level $ref - use the schema name from context
                // This case shouldn't happen here as we need a name, return None
                None
            } else if schema.properties.is_some() {
                // Inline schema with properties - needs a name (not available here)
                None
            } else {
                // Simple type without ref - shouldn't happen in well-formed specs
                None
            }
        } else {
            Some(ResponseType::JsonValue)
        }
    }

    /// Normalize type name to Rust `PascalCase`.
    ///
    /// Handles various naming conventions:
    /// - "treasury.transaction" → "`TreasuryTransaction`"
    /// - "Custom-pages" → "`CustomPages`"
    /// - "@`cf_ai4bharat.translation`" → "`CfAi4bharatTranslation`"
    #[must_use]
    pub fn to_pascal_case(name: &str) -> String {
        name.split(['.', '-', '@', '_'])
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
    #[must_use]
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

    /// Convert identifier to `snake_case`.
    #[must_use]
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
                    if next_is_lower || current.chars().last().is_some_and(char::is_lowercase) {
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
    #[must_use]
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
    use crate::spec::{MediaType, Response};
    use std::collections::BTreeMap;

    fn make_schema(properties: Option<BTreeMap<String, Schema>>) -> Schema {
        Schema {
            schema_type: Some("object".to_string()),
            properties,
            ..Default::default()
        }
    }

    #[test]
    fn normalizes_dotted_names() {
        assert_eq!(
            TypeResolver::to_pascal_case("treasury.transaction"),
            "TreasuryTransaction"
        );
    }

    #[test]
    fn normalizes_hyphenated_names() {
        assert_eq!(TypeResolver::to_pascal_case("Custom-pages"), "CustomPages");
    }

    #[test]
    fn normalizes_snake_case_names() {
        assert_eq!(
            TypeResolver::to_pascal_case("iam_response_collection"),
            "IamResponseCollection"
        );
    }

    #[test]
    fn normalizes_at_prefixed_names() {
        assert_eq!(
            TypeResolver::to_pascal_case("@cf_ai4bharat.translation"),
            "CfAi4bharatTranslation"
        );
    }

    #[test]
    fn renames_rust_keywords() {
        assert_eq!(
            TypeResolver::rename_if_keyword("Option".to_string()),
            "ApiOption"
        );
        assert_eq!(
            TypeResolver::rename_if_keyword("Value".to_string()),
            "ApiValue"
        );
        assert_eq!(
            TypeResolver::rename_if_keyword("Result".to_string()),
            "ApiResult"
        );
        assert_eq!(
            TypeResolver::rename_if_keyword("MyType".to_string()),
            "MyType"
        );
    }

    #[test]
    fn converts_to_snake_case() {
        assert_eq!(
            TypeResolver::to_snake_case("getV1Projects"),
            "get_v1_projects"
        );
        assert_eq!(
            TypeResolver::to_snake_case("GetV1Projects"),
            "get_v1_projects"
        );
    }

    #[test]
    fn is_generatable_with_properties() {
        let schemas = Arc::new(BTreeMap::new());
        let resolver = TypeResolver::new(schemas);

        let mut props = BTreeMap::new();
        props.insert(
            "name".to_string(),
            Schema {
                schema_type: Some("string".to_string()),
                ..Default::default()
            },
        );

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
                Schema {
                    ref_path: Some("#/components/schemas/A".to_string()),
                    ..Default::default()
                },
                Schema {
                    ref_path: Some("#/components/schemas/B".to_string()),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        assert!(!resolver.is_generatable(&schema));
    }

    #[test]
    fn is_generatable_with_allof_multiple_refs() {
        // allOf with multiple refs (extending base type) should be generatable
        let mut schemas = BTreeMap::new();
        schemas.insert(
            "BaseResponse".to_string(),
            Schema {
                schema_type: Some("object".to_string()),
                ..Default::default()
            },
        );
        let schemas = Arc::new(schemas);
        let resolver = TypeResolver::new(schemas);

        let schema = Schema {
            all_of: Some(vec![
                Schema {
                    ref_path: Some("#/components/schemas/BaseResponse".to_string()),
                    ..Default::default()
                },
                Schema {
                    properties: Some(BTreeMap::from([(
                        "result".to_string(),
                        Schema {
                            schema_type: Some("object".to_string()),
                            ..Default::default()
                        },
                    )])),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        assert!(resolver.is_generatable(&schema));
    }

    #[test]
    fn get_response_type_with_allof_schema() {
        // Test that get_response_type correctly handles allOf response schemas
        let mut schemas = BTreeMap::new();
        schemas.insert(
            "IamApiResponseCollection".to_string(),
            Schema {
                schema_type: Some("object".to_string()),
                ..Default::default()
            },
        );
        schemas.insert(
            "IamAccount".to_string(),
            Schema {
                schema_type: Some("object".to_string()),
                ..Default::default()
            },
        );
        let schemas = Arc::new(schemas);
        let resolver = TypeResolver::new(schemas);

        // Create an allOf schema like iam_response_collection_accounts
        let response_schema = Schema {
            all_of: Some(vec![
                Schema {
                    ref_path: Some("#/components/schemas/IamApiResponseCollection".to_string()),
                    ..Default::default()
                },
                Schema {
                    properties: Some(BTreeMap::from([(
                        "result".to_string(),
                        Schema {
                            schema_type: Some("array".to_string()),
                            items: Some(Box::new(Schema {
                                ref_path: Some("#/components/schemas/IamAccount".to_string()),
                                ..Default::default()
                            })),
                            ..Default::default()
                        },
                    )])),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };

        // Create a response with the allOf schema
        let response = Response {
            description: Some("Success".to_string()),
            content: Some(BTreeMap::from([(
                "application/json".to_string(),
                MediaType {
                    schema: Some(response_schema),
                },
            )])),
        };

        // For inline allOf schemas (no $ref), get_response_type returns None
        // because there's no type name to use. The name comes from the endpoint's
        // response $ref, not from the inline schema itself.
        let result = resolver.get_response_type(&response);
        assert!(result.is_none()); // Inline allOf without $ref returns None
    }

    #[test]
    fn get_response_type_with_ref_to_allof_schema() {
        // Test that get_response_type correctly resolves $ref to allOf schema
        let mut schemas = BTreeMap::new();

        // Base response type
        schemas.insert(
            "IamApiResponseCollection".to_string(),
            Schema {
                schema_type: Some("object".to_string()),
                ..Default::default()
            },
        );

        // allOf type that extends base (like iam_response_collection_accounts)
        schemas.insert(
            "IamResponseCollectionAccounts".to_string(),
            Schema {
                all_of: Some(vec![
                    Schema {
                        ref_path: Some("#/components/schemas/IamApiResponseCollection".to_string()),
                        ..Default::default()
                    },
                    Schema {
                        properties: Some(BTreeMap::from([(
                            "result".to_string(),
                            Schema {
                                schema_type: Some("array".to_string()),
                                ..Default::default()
                            },
                        )])),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
        );

        let schemas = Arc::new(schemas);
        let resolver = TypeResolver::new(schemas);

        // Create a response that $refs to the allOf type
        let response = Response {
            description: Some("Success".to_string()),
            content: Some(BTreeMap::from([(
                "application/json".to_string(),
                MediaType {
                    schema: Some(Schema {
                        ref_path: Some(
                            "#/components/schemas/IamResponseCollectionAccounts".to_string(),
                        ),
                        ..Default::default()
                    }),
                },
            )])),
        };

        let result = resolver.get_response_type(&response);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            ResponseType::Generated("IamResponseCollectionAccounts".to_string())
        );
    }
}
