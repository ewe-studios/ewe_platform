//! Schema compiler — transforms a JSON Schema into a validator tree.
//!
//! WHY: Interpreting the schema on every validation is expensive. By compiling
//! once into a tree of keyword validators, we pay the schema analysis cost once
//! and validate instances with minimal per-invocation overhead.

use alloc::string::String;
use alloc::vec::Vec;

use crate::compiler_context::CompilerContext;
use crate::draft::Draft;
use crate::error::{ValidationError, ValidationErrorKind};
use crate::keywords::BoxedValidator;
use crate::keywords::type_::TypeValidator;
use crate::keywords::const_::ConstValidator;
use crate::keywords::enum_::EnumValidator;
use crate::keywords::min_length::MinLengthValidator;
use crate::keywords::max_length::MaxLengthValidator;
use crate::keywords::pattern::PatternValidator;
use crate::keywords::format::FormatValidator;
use crate::keywords::minimum::MinimumValidator;
use crate::keywords::maximum::MaximumValidator;
use crate::keywords::exclusive_minimum::ExclusiveMinimumValidator;
use crate::keywords::exclusive_maximum::ExclusiveMaximumValidator;
use crate::keywords::multiple_of::MultipleOfValidator;
use crate::keywords::required::RequiredValidator;
use crate::keywords::min_properties::MinPropertiesValidator;
use crate::keywords::max_properties::MaxPropertiesValidator;
use crate::keywords::property_names::PropertyNamesValidator;
use crate::keywords::dependent_required::DependentRequiredValidator;
use crate::keywords::properties::PropertiesValidator;
use crate::keywords::additional_properties::AdditionalPropertiesValidator;
use crate::keywords::pattern_properties::PatternPropertiesValidator;
use crate::keywords::dependent_schemas::DependentSchemasValidator;
use crate::keywords::unevaluated_properties::UnevaluatedPropertiesValidator;
use crate::keywords::min_items::MinItemsValidator;
use crate::keywords::max_items::MaxItemsValidator;
use crate::keywords::unique_items::UniqueItemsValidator;
use crate::keywords::items::ItemsValidator;
use crate::keywords::prefix_items::PrefixItemsValidator;
use crate::keywords::contains::ContainsValidator;
use crate::keywords::unevaluated_items::UnevaluatedItemsValidator;
use crate::keywords::all_of::AllOfValidator;
use crate::keywords::any_of::AnyOfValidator;
use crate::keywords::one_of::OneOfValidator;
use crate::keywords::not_::NotValidator;
use crate::keywords::if_::IfThenElseValidator;
use crate::keywords::ref_::{RefValidator, DynamicRefValidator, RecursiveRefValidator};
use crate::keywords::content::{ContentEncodingValidator, ContentMediaTypeValidator};
use crate::node::SchemaNode;
use crate::paths::Location;
use crate::referencing::{Registry, VocabularySet};
use foundation_errstacks::IntoErrorTrace;
use serde_json::Value;

/// Compile a JSON Schema into a validator tree.
///
/// WHY: This is the main entry point that transforms a raw JSON Schema
/// into an immutable `SchemaNode` ready for validating instances.
#[allow(dead_code)]
pub(crate) fn compile(
    schema: &Value,
    registry: &Registry,
    draft: Draft,
) -> Result<SchemaNode, ValidationError> {
    let base_uri = schema
        .as_object()
        .and_then(|o| o.get("$id"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let resolver = registry.resolver(base_uri);
    let schema_path = Location::new();
    let vocabulary = VocabularySet::for_draft(draft);
    let ctx = CompilerContext::new(resolver, schema_path, vocabulary, draft);

    compile_node(schema, &ctx)
}

/// Compile a schema value into a `SchemaNode`.
fn compile_node(schema: &Value, ctx: &CompilerContext) -> Result<SchemaNode, ValidationError> {
    // Boolean schemas
    if let Value::Bool(b) = schema {
        return if *b {
            Ok(SchemaNode::AlwaysValid)
        } else {
            Ok(SchemaNode::AlwaysInvalid {
                schema_path: ctx.schema_path.clone(),
            })
        };
    }

    let schema_obj = schema.as_object().ok_or_else(|| {
        ValidationErrorKind::Schema {
            reason: "schema must be an object or boolean".into(),
        }
        .into_error_trace()
    })?;

    let mut validators: Vec<BoxedValidator> = Vec::new();

    // Process each keyword
    for (key, value) in schema_obj {
        let keyword_ctx = ctx.push_keyword(key);

        if let Some(result) = compile_keyword(key, value, &keyword_ctx) {
            validators.push(result);
        }
    }

    Ok(SchemaNode::Validators {
        validators,
        schema_path: ctx.schema_path.clone(),
    })
}

/// Compile a single keyword into a validator.
fn compile_keyword(
    keyword: &str,
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Option<BoxedValidator> {
    match keyword {
        "type" => compile_type(value, ctx),
        "const" => Some(compile_const(value, ctx)),
        "enum" => Some(compile_enum(value, ctx)),
        // String
        "minLength" => Some(compile_min_length(value, ctx)),
        "maxLength" => Some(compile_max_length(value, ctx)),
        "pattern" => Some(compile_pattern(value, ctx)),
        "format" => Some(compile_format(value, ctx)),
        // Number
        "minimum" => Some(compile_minimum(value, ctx)),
        "maximum" => Some(compile_maximum(value, ctx)),
        "exclusiveMinimum" => Some(compile_exclusive_minimum(value, ctx)),
        "exclusiveMaximum" => Some(compile_exclusive_maximum(value, ctx)),
        "multipleOf" => compile_multiple_of(value, ctx),
        // Object
        "required" => Some(compile_required(value, ctx)),
        "minProperties" => Some(compile_min_properties(value, ctx)),
        "maxProperties" => Some(compile_max_properties(value, ctx)),
        "propertyNames" => Some(compile_property_names(value, ctx)),
        "dependentRequired" => Some(compile_dependent_required(value, ctx)),
        "properties" => Some(compile_properties_stub()),
        "additionalProperties" => Some(compile_additional_properties_stub()),
        "patternProperties" => Some(compile_pattern_properties_stub()),
        "dependentSchemas" => Some(compile_dependent_schemas_stub()),
        "unevaluatedProperties" => Some(compile_unevaluated_properties_stub()),
        // Array
        "minItems" => Some(compile_min_items(value, ctx)),
        "maxItems" => Some(compile_max_items(value, ctx)),
        "uniqueItems" => compile_unique_items(value, ctx),
        "items" => Some(compile_items_stub()),
        "prefixItems" => Some(compile_prefix_items_stub()),
        "contains" => Some(compile_contains_stub()),
        "unevaluatedItems" => Some(compile_unevaluated_items_stub()),
        // Composition
        "allOf" => Some(compile_all_of_stub()),
        "anyOf" => Some(compile_any_of_stub()),
        "oneOf" => Some(compile_one_of_stub()),
        "not" => Some(compile_not_stub()),
        "if" | "then" | "else" => Some(compile_if_then_else_stub()),
        // Reference
        "$ref" => Some(compile_ref(value)),
        "$dynamicRef" => Some(compile_dynamic_ref_stub()),
        "$recursiveRef" => Some(compile_recursive_ref_stub()),
        // Content
        "contentEncoding" => Some(compile_content_encoding(value, ctx)),
        "contentMediaType" => Some(compile_content_media_type(value, ctx)),
        // Unknown — skip
        _ => None,
    }
}

// ── Type ───────────────────────────────────────────────────────────────

fn compile_type(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Option<BoxedValidator> {
    use crate::types::JsonTypeSet;
    let types = match value {
        Value::String(s) => {
            let mut set = JsonTypeSet::new();
            if let Some(t) = parse_type_name(s) {
                set.insert(t);
            }
            set
        }
        Value::Array(arr) => {
            let mut set = JsonTypeSet::new();
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Some(t) = parse_type_name(s) {
                        set.insert(t);
                    }
                }
            }
            set
        }
        _ => return None,
    };
    Some(Box::new(TypeValidator::new(
        types,
        ctx.schema_path.clone(),
    )))
}

fn parse_type_name(name: &str) -> Option<crate::types::JsonType> {
    use crate::types::JsonType;
    match name {
        "null" => Some(JsonType::Null),
        "boolean" => Some(JsonType::Boolean),
        "object" => Some(JsonType::Object),
        "array" => Some(JsonType::Array),
        "string" => Some(JsonType::String),
        "number" => Some(JsonType::Number),
        "integer" => Some(JsonType::Integer),
        _ => None,
    }
}

// ── Const / Enum ───────────────────────────────────────────────────────

fn compile_const(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    Box::new(ConstValidator::new(value.clone(), ctx.schema_path.clone()))
}

fn compile_enum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let options = value.as_array().cloned().unwrap_or_default();
    Box::new(EnumValidator::new(options, ctx.schema_path.clone()))
}

// ── String ─────────────────────────────────────────────────────────────

fn compile_min_length(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let min = value.as_u64().unwrap_or(0);
    Box::new(MinLengthValidator::new(min, ctx.schema_path.clone()))
}

fn compile_max_length(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let max = value.as_u64().unwrap_or(0);
    Box::new(MaxLengthValidator::new(max, ctx.schema_path.clone()))
}

fn compile_pattern(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let pattern = value.as_str().unwrap_or("").to_string();
    Box::new(PatternValidator::new(pattern, ctx.schema_path.clone()))
}

fn compile_format(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let format_name = value.as_str().unwrap_or("").to_string();
    Box::new(FormatValidator::new(format_name, ctx.schema_path.clone()))
}

// ── Number ─────────────────────────────────────────────────────────────

fn compile_minimum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);
    Box::new(MinimumValidator::new(limit, false, ctx.schema_path.clone()))
}

fn compile_maximum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);
    Box::new(MaximumValidator::new(limit, false, ctx.schema_path.clone()))
}

fn compile_exclusive_minimum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);
    Box::new(ExclusiveMinimumValidator::new(limit, ctx.schema_path.clone()))
}

fn compile_exclusive_maximum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);
    Box::new(ExclusiveMaximumValidator::new(limit, ctx.schema_path.clone()))
}

fn compile_multiple_of(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Option<BoxedValidator> {
    let multiple = value.as_f64().unwrap_or(0.0);
    if multiple == 0.0 {
        return None;
    }
    Some(Box::new(MultipleOfValidator::new(multiple, ctx.schema_path.clone())))
}

// ── Object ─────────────────────────────────────────────────────────────

fn compile_required(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let props = value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Box::new(RequiredValidator::new(props, ctx.schema_path.clone()))
}

fn compile_min_properties(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let min = value.as_u64().unwrap_or(0);
    Box::new(MinPropertiesValidator::new(min, ctx.schema_path.clone()))
}

fn compile_max_properties(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let max = value.as_u64().unwrap_or(0);
    Box::new(MaxPropertiesValidator::new(max, ctx.schema_path.clone()))
}

fn compile_property_names(_value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    Box::new(PropertyNamesValidator::new(Vec::new(), ctx.schema_path.clone()))
}

fn compile_dependent_required(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    use alloc::collections::BTreeMap;
    let map = value
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    let props: Vec<String> = v
                        .as_array()?
                        .iter()
                        .filter_map(|p| p.as_str().map(String::from))
                        .collect();
                    if props.is_empty() {
                        None
                    } else {
                        Some((k.clone(), props))
                    }
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    Box::new(DependentRequiredValidator::new(map, ctx.schema_path.clone()))
}

// ── Stub keywords (sub-schema validators — need SchemaNode) ────────────

fn compile_properties_stub() -> BoxedValidator { Box::new(PropertiesValidator) }
fn compile_additional_properties_stub() -> BoxedValidator { Box::new(AdditionalPropertiesValidator) }
fn compile_pattern_properties_stub() -> BoxedValidator { Box::new(PatternPropertiesValidator) }
fn compile_dependent_schemas_stub() -> BoxedValidator { Box::new(DependentSchemasValidator) }
fn compile_unevaluated_properties_stub() -> BoxedValidator { Box::new(UnevaluatedPropertiesValidator) }
fn compile_items_stub() -> BoxedValidator { Box::new(ItemsValidator) }
fn compile_prefix_items_stub() -> BoxedValidator { Box::new(PrefixItemsValidator) }
fn compile_contains_stub() -> BoxedValidator { Box::new(ContainsValidator) }
fn compile_unevaluated_items_stub() -> BoxedValidator { Box::new(UnevaluatedItemsValidator) }
fn compile_all_of_stub() -> BoxedValidator { Box::new(AllOfValidator) }
fn compile_any_of_stub() -> BoxedValidator { Box::new(AnyOfValidator) }
fn compile_one_of_stub() -> BoxedValidator { Box::new(OneOfValidator) }
fn compile_not_stub() -> BoxedValidator { Box::new(NotValidator) }
fn compile_if_then_else_stub() -> BoxedValidator { Box::new(IfThenElseValidator) }

fn compile_ref(value: &Value) -> BoxedValidator {
    let reference = value.as_str().unwrap_or("").to_string();
    Box::new(RefValidator::new(reference))
}

fn compile_dynamic_ref_stub() -> BoxedValidator { Box::new(DynamicRefValidator) }
fn compile_recursive_ref_stub() -> BoxedValidator { Box::new(RecursiveRefValidator) }

// ── Content ────────────────────────────────────────────────────────────

fn compile_content_encoding(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let encoding = value.as_str().unwrap_or("").to_string();
    Box::new(ContentEncodingValidator::new(encoding, ctx.schema_path.clone()))
}

fn compile_content_media_type(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let media_type = value.as_str().unwrap_or("").to_string();
    Box::new(ContentMediaTypeValidator::new(media_type, ctx.schema_path.clone()))
}

// ── Array validators ───────────────────────────────────────────────────

fn compile_min_items(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let min = value.as_u64().unwrap_or(0);
    Box::new(MinItemsValidator::new(min, ctx.schema_path.clone()))
}

fn compile_max_items(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let max = value.as_u64().unwrap_or(0);
    Box::new(MaxItemsValidator::new(max, ctx.schema_path.clone()))
}

fn compile_unique_items(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Option<BoxedValidator> {
    if value.as_bool().unwrap_or(false) {
        Some(Box::new(UniqueItemsValidator::new(ctx.schema_path.clone())))
    } else {
        None
    }
}
