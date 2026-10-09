use foundation_openapi::spec::{MediaType, Response};
use foundation_openapi::{ResponseType, Schema, TypeResolver};
use std::collections::BTreeMap;
use std::sync::Arc;

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
