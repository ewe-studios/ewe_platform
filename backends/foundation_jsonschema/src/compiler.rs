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
use crate::keywords::*;
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
pub fn compile(
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

/// Compile a schema value into a SchemaNode.
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

        if let Some(result) = compile_keyword(key, value, &keyword_ctx)? {
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
) -> Result<Option<BoxedValidator>, ValidationError> {
    match keyword {
        "type" => compile_type(value, ctx),
        "const" => compile_const(value, ctx),
        "enum" => compile_enum(value, ctx),
        // String
        "minLength" => compile_min_length(value, ctx),
        "maxLength" => compile_max_length(value, ctx),
        "pattern" => compile_pattern(value, ctx),
        "format" => compile_format(value, ctx),
        // Number
        "minimum" => compile_minimum(value, ctx),
        "maximum" => compile_maximum(value, ctx),
        "exclusiveMinimum" => compile_exclusive_minimum(value, ctx),
        "exclusiveMaximum" => compile_exclusive_maximum(value, ctx),
        "multipleOf" => compile_multiple_of(value, ctx),
        // Object
        "required" => compile_required(value, ctx),
        "minProperties" => compile_min_properties(value, ctx),
        "maxProperties" => compile_max_properties(value, ctx),
        "propertyNames" => compile_property_names(value, ctx),
        "dependentRequired" => compile_dependent_required(value, ctx),
        "properties" => compile_properties_stub(value, ctx),
        "additionalProperties" => compile_additional_properties_stub(value, ctx),
        "patternProperties" => compile_pattern_properties_stub(value, ctx),
        "dependentSchemas" => compile_dependent_schemas_stub(value, ctx),
        "unevaluatedProperties" => compile_unevaluated_properties_stub(value, ctx),
        // Array
        "minItems" => compile_min_items(value, ctx),
        "maxItems" => compile_max_items(value, ctx),
        "uniqueItems" => compile_unique_items(value, ctx),
        "items" => compile_items_stub(value, ctx),
        "prefixItems" => compile_prefix_items_stub(value, ctx),
        "contains" => compile_contains_stub(value, ctx),
        "unevaluatedItems" => compile_unevaluated_items_stub(value, ctx),
        // Composition
        "allOf" => compile_all_of_stub(value, ctx),
        "anyOf" => compile_any_of_stub(value, ctx),
        "oneOf" => compile_one_of_stub(value, ctx),
        "not" => compile_not_stub(value, ctx),
        "if" | "then" | "else" => compile_if_then_else_stub(value, ctx),
        // Reference
        "$ref" => compile_ref(value, ctx),
        "$dynamicRef" => compile_dynamic_ref_stub(value, ctx),
        "$recursiveRef" => compile_recursive_ref_stub(value, ctx),
        // Content
        "contentEncoding" => compile_content_encoding(value, ctx),
        "contentMediaType" => compile_content_media_type(value, ctx),
        // Unknown — skip
        _ => Ok(None),
    }
}

// ── Type ───────────────────────────────────────────────────────────────

fn compile_type(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
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
        _ => return Ok(None),
    };
    Ok(Some(Box::new(TypeValidator::new(
        types,
        ctx.schema_path.clone(),
    ))))
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

fn compile_const(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(ConstValidator::new(
        value.clone(),
        ctx.schema_path.clone(),
    ))))
}

fn compile_enum(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let options = value
        .as_array()
        .map(|arr| arr.clone())
        .unwrap_or_default();
    Ok(Some(Box::new(EnumValidator::new(
        options,
        ctx.schema_path.clone(),
    ))))
}

// ── String ─────────────────────────────────────────────────────────────

fn compile_min_length(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let min = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MinLengthValidator::new(
        min,
        ctx.schema_path.clone(),
    ))))
}

fn compile_max_length(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let max = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MaxLengthValidator::new(
        max,
        ctx.schema_path.clone(),
    ))))
}

fn compile_pattern(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let pattern = value.as_str().unwrap_or("").to_string();
    Ok(Some(Box::new(PatternValidator::new(
        pattern,
        ctx.schema_path.clone(),
    ))))
}

fn compile_format(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let format_name = value.as_str().unwrap_or("").to_string();
    Ok(Some(Box::new(FormatValidator::new(
        format_name,
        ctx.schema_path.clone(),
    ))))
}

// ── Number ─────────────────────────────────────────────────────────────

fn compile_minimum(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let limit = value.as_f64().unwrap_or(0.0);
    Ok(Some(Box::new(MinimumValidator::new(
        limit,
        false,
        ctx.schema_path.clone(),
    ))))
}

fn compile_maximum(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let limit = value.as_f64().unwrap_or(0.0);
    Ok(Some(Box::new(MaximumValidator::new(
        limit,
        false,
        ctx.schema_path.clone(),
    ))))
}

fn compile_exclusive_minimum(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let limit = value.as_f64().unwrap_or(0.0);
    Ok(Some(Box::new(ExclusiveMinimumValidator::new(
        limit,
        ctx.schema_path.clone(),
    ))))
}

fn compile_exclusive_maximum(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let limit = value.as_f64().unwrap_or(0.0);
    Ok(Some(Box::new(ExclusiveMaximumValidator::new(
        limit,
        ctx.schema_path.clone(),
    ))))
}

fn compile_multiple_of(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let multiple = value.as_f64().unwrap_or(0.0);
    if multiple == 0.0 {
        return Ok(None);
    }
    Ok(Some(Box::new(MultipleOfValidator::new(
        multiple,
        ctx.schema_path.clone(),
    ))))
}

// ── Object ─────────────────────────────────────────────────────────────

fn compile_required(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let props = value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(Box::new(RequiredValidator::new(
        props,
        ctx.schema_path.clone(),
    ))))
}

fn compile_min_properties(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let min = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MinPropertiesValidator::new(
        min,
        ctx.schema_path.clone(),
    ))))
}

fn compile_max_properties(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let max = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MaxPropertiesValidator::new(
        max,
        ctx.schema_path.clone(),
    ))))
}

fn compile_property_names(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(PropertyNamesValidator::new(
        Vec::new(),
        ctx.schema_path.clone(),
    ))))
}

fn compile_dependent_required(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
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
    Ok(Some(Box::new(DependentRequiredValidator::new(
        map,
        ctx.schema_path.clone(),
    ))))
}

// ── Stub keywords (sub-schema validators — need SchemaNode) ────────────

macro_rules! stub_kw {
    ($fn:ident, $validator:ty) => {
        fn $fn(
            _value: &Value,
            ctx: &CompilerContext<'_>,
        ) -> Result<Option<BoxedValidator>, ValidationError> {
            Ok(Some(Box::new(<$validator>::default())))
        }
    };
}

// For stub validators that implement Default
fn compile_properties_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(PropertiesValidator::default())))
}

fn compile_additional_properties_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(AdditionalPropertiesValidator::default())))
}

fn compile_pattern_properties_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(PatternPropertiesValidator::default())))
}

fn compile_dependent_schemas_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(DependentSchemasValidator::default())))
}

fn compile_unevaluated_properties_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(UnevaluatedPropertiesValidator::default())))
}

fn compile_items_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(ItemsValidator::default())))
}

fn compile_prefix_items_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(PrefixItemsValidator::default())))
}

fn compile_contains_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(ContainsValidator)))
}

fn compile_unevaluated_items_stub(
    _value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(UnevaluatedItemsValidator::default())))
}

fn compile_all_of_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(AllOfValidator)))
}

fn compile_any_of_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(AnyOfValidator)))
}

fn compile_one_of_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(OneOfValidator)))
}

fn compile_not_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(NotValidator)))
}

fn compile_if_then_else_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(IfThenElseValidator)))
}

fn compile_ref(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let reference = value.as_str().unwrap_or("").to_string();
    Ok(Some(Box::new(RefValidator::new(reference))))
}

fn compile_dynamic_ref_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(DynamicRefValidator)))
}

fn compile_recursive_ref_stub(
    _value: &Value,
    _ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    Ok(Some(Box::new(RecursiveRefValidator)))
}

// ── Content ────────────────────────────────────────────────────────────

fn compile_content_encoding(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let encoding = value.as_str().unwrap_or("").to_string();
    Ok(Some(Box::new(ContentEncodingValidator::new(
        encoding,
        ctx.schema_path.clone(),
    ))))
}

fn compile_content_media_type(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let media_type = value.as_str().unwrap_or("").to_string();
    Ok(Some(Box::new(ContentMediaTypeValidator::new(
        media_type,
        ctx.schema_path.clone(),
    ))))
}

// ── Array validators ───────────────────────────────────────────────────

fn compile_min_items(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let min = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MinItemsValidator::new(
        min,
        ctx.schema_path.clone(),
    ))))
}

fn compile_max_items(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    let max = value.as_u64().unwrap_or(0);
    Ok(Some(Box::new(MaxItemsValidator::new(
        max,
        ctx.schema_path.clone(),
    ))))
}

fn compile_unique_items(
    value: &Value,
    ctx: &CompilerContext<'_>,
) -> Result<Option<BoxedValidator>, ValidationError> {
    if value.as_bool().unwrap_or(false) {
        Ok(Some(Box::new(UniqueItemsValidator::new(
            ctx.schema_path.clone(),
        ))))
    } else {
        Ok(None)
    }
}
