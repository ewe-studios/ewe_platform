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
use crate::keywords::additional_properties::{AdditionalPropertiesValidator, AdditionalSchema};
use crate::keywords::all_of::AllOfValidator;
use crate::keywords::any_of::AnyOfValidator;
use crate::keywords::const_::ConstValidator;
use crate::keywords::contains::ContainsValidator;
use crate::keywords::content::{
    ContentCombinedValidator, ContentEncoding, ContentEncodingValidator, ContentMediaTypeValidator,
};
use crate::keywords::content_schema::ContentSchemaValidator;
use crate::keywords::dependent_required::DependentRequiredValidator;
use crate::keywords::dependent_schemas::DependentSchemasValidator;
use crate::keywords::enum_::EnumValidator;
use crate::keywords::exclusive_maximum::ExclusiveMaximumValidator;
use crate::keywords::exclusive_minimum::ExclusiveMinimumValidator;
use crate::keywords::format::FormatValidator;
use crate::keywords::if_::IfThenElseValidator;
use crate::keywords::items::ItemsValidator;
use crate::keywords::legacy::{DependenciesValidator, Dependency};
use crate::keywords::max_items::MaxItemsValidator;
use crate::keywords::max_length::MaxLengthValidator;
use crate::keywords::max_properties::MaxPropertiesValidator;
use crate::keywords::maximum::MaximumValidator;
use crate::keywords::min_items::MinItemsValidator;
use crate::keywords::min_length::MinLengthValidator;
use crate::keywords::min_properties::MinPropertiesValidator;
use crate::keywords::minimum::MinimumValidator;
use crate::keywords::multiple_of::MultipleOfValidator;
use crate::keywords::not_::NotValidator;
use crate::keywords::one_of::OneOfValidator;
use crate::keywords::pattern::PatternValidator;
use crate::keywords::pattern_properties::PatternPropertiesValidator;
use crate::keywords::prefix_items::PrefixItemsValidator;
use crate::keywords::properties::PropertiesValidator;
use crate::keywords::property_names::PropertyNamesValidator;
use crate::keywords::ref_::{DynamicRefValidator, RecursiveRefValidator, RefValidator};
use crate::keywords::required::RequiredValidator;
use crate::keywords::tuple_items::{AdditionalItemsPolicy, TupleItemsValidator};
use crate::keywords::type_::TypeValidator;
use crate::keywords::unevaluated_items::UnevaluatedItemsValidator;
use crate::keywords::unevaluated_properties::UnevaluatedPropertiesValidator;
use crate::keywords::unique_items::UniqueItemsValidator;
use crate::keywords::BoxedValidator;
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
    assert_format: bool,
    custom_formats: alloc::collections::BTreeMap<
        alloc::string::String,
        Box<dyn crate::formats::FormatChecker>,
    >,
    custom_keywords: alloc::collections::BTreeMap<
        alloc::string::String,
        Box<dyn crate::keywords::custom::KeywordFactory>,
    >,
) -> Result<SchemaNode, ValidationError> {
    let base_uri = schema
        .as_object()
        .and_then(|o| o.get("$id"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let resolver = registry.resolver(base_uri);
    let schema_path = Location::new();
    let vocabulary = VocabularySet::for_draft(draft);
    let ctx = CompilerContext::new(
        resolver,
        schema_path,
        vocabulary,
        draft,
        assert_format,
        custom_formats,
        custom_keywords,
    );

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

    // Pre-compute which properties are defined (for additionalProperties)
    let defined_properties: alloc::collections::BTreeSet<alloc::string::String> = schema_obj
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let pattern_regexes: Vec<(regex::Regex, alloc::string::String)> = schema_obj
        .get("patternProperties")
        .and_then(|p| p.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, _)| regex::Regex::new(k).ok().map(|r| (r, k.clone())))
                .collect()
        })
        .unwrap_or_default();

    let mut validators: Vec<BoxedValidator> = Vec::new();

    // Process each keyword
    for (key, value) in schema_obj {
        let keyword_ctx = ctx.push_keyword(key);

        if let Some(result) = compile_keyword(
            key,
            value,
            schema_obj,
            &keyword_ctx,
            &defined_properties,
            &pattern_regexes,
        ) {
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
    schema_obj: &serde_json::Map<String, Value>,
    ctx: &CompilerContext<'_>,
    defined_properties: &alloc::collections::BTreeSet<alloc::string::String>,
    pattern_regexes: &[(regex::Regex, alloc::string::String)],
) -> Option<BoxedValidator> {
    // Check custom keywords first
    if let Some(factory) = ctx.custom_keywords.get(keyword) {
        match factory.compile(value, ctx.schema_path.clone()) {
            Ok(validator) => return Some(validator),
            Err(_) => return None,
        }
    }

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
        "properties" => compile_properties(value, ctx),
        "additionalProperties" => {
            compile_additional_properties(value, ctx, defined_properties, pattern_regexes)
        }
        "patternProperties" => compile_pattern_properties(value, ctx),
        "dependentSchemas" => compile_dependent_schemas(value, ctx),
        "unevaluatedProperties" => compile_unevaluated_properties(value, ctx),
        // Array
        "minItems" => Some(compile_min_items(value, ctx)),
        "maxItems" => Some(compile_max_items(value, ctx)),
        "uniqueItems" => compile_unique_items(value, ctx),
        "items" => compile_items(value, ctx, schema_obj),
        "prefixItems" => compile_prefix_items(value, ctx),
        "contains" => compile_contains(value, ctx),
        "unevaluatedItems" => compile_unevaluated_items(value, ctx),
        "additionalItems" => compile_additional_items(value, ctx, schema_obj),
        // Composition
        "allOf" => compile_all_of(value, ctx),
        "anyOf" => compile_any_of(value, ctx),
        "oneOf" => compile_one_of(value, ctx),
        "not" => compile_not(value, ctx),
        "if" | "then" | "else" => compile_if_then_else(schema_obj, ctx),
        // Reference
        "$ref" => compile_ref(value, ctx),
        "$dynamicRef" => compile_dynamic_ref(value, ctx),
        "$recursiveRef" => compile_recursive_ref(value, ctx),
        // Content
        "contentEncoding" => compile_content_encoding(value, ctx, schema_obj),
        "contentMediaType" => Some(compile_content_media_type(value, ctx, schema_obj)),
        "contentSchema" => compile_content_schema(value, ctx, schema_obj),
        // Legacy
        "dependencies" => compile_dependencies(value, ctx),
        // Unknown — skip
        _ => None,
    }
}

// ── Type ───────────────────────────────────────────────────────────────

fn compile_type(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
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
    Some(Box::new(TypeValidator::new(types, ctx.schema_path.clone())))
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
    // Check custom formats first, then built-in
    let checker = ctx
        .custom_formats
        .get(&format_name)
        .map(|c| c.clone_box())
        // Clippy suggests using method syntax here, but FormatChecker trait isn't in scope
        .or_else(|| {
            crate::formats::builtin_format(&format_name)
                .map(super::formats::FormatChecker::clone_box)
        });
    Box::new(FormatValidator::new(
        format_name,
        ctx.schema_path.clone(),
        checker,
        ctx.assert_format,
    ))
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
    Box::new(ExclusiveMinimumValidator::new(
        limit,
        ctx.schema_path.clone(),
    ))
}

fn compile_exclusive_maximum(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let limit = value.as_f64().unwrap_or(0.0);
    Box::new(ExclusiveMaximumValidator::new(
        limit,
        ctx.schema_path.clone(),
    ))
}

fn compile_multiple_of(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    let multiple = value.as_f64().unwrap_or(0.0);
    if multiple == 0.0 {
        return None;
    }
    Some(Box::new(MultipleOfValidator::new(
        multiple,
        ctx.schema_path.clone(),
    )))
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

fn compile_property_names(value: &Value, ctx: &CompilerContext<'_>) -> BoxedValidator {
    let sub_ctx = ctx.push_keyword("propertyNames");
    let schema = compile_node(value, &sub_ctx).unwrap_or(SchemaNode::AlwaysValid);
    Box::new(PropertyNamesValidator::new(schema))
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
    Box::new(DependentRequiredValidator::new(
        map,
        ctx.schema_path.clone(),
    ))
}

// ── Composition keywords (sub-schema validators) ─────────────────────

fn compile_all_of(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let arr = value.as_array()?;
    let mut schemas = Vec::new();
    for (i, sub) in arr.iter().enumerate() {
        let sub_ctx = ctx.push_keyword(&format!("allOf/{i}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => schemas.push(node),
            Err(_) => return None,
        }
    }
    Some(Box::new(AllOfValidator::new(schemas)))
}

fn compile_any_of(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let arr = value.as_array()?;
    let mut schemas = Vec::new();
    for (i, sub) in arr.iter().enumerate() {
        let sub_ctx = ctx.push_keyword(&format!("anyOf/{i}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => schemas.push(node),
            Err(_) => return None,
        }
    }
    Some(Box::new(AnyOfValidator::new(schemas)))
}

fn compile_one_of(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let arr = value.as_array()?;
    let mut schemas = Vec::new();
    for (i, sub) in arr.iter().enumerate() {
        let sub_ctx = ctx.push_keyword(&format!("oneOf/{i}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => schemas.push(node),
            Err(_) => return None,
        }
    }
    Some(Box::new(OneOfValidator::new(schemas)))
}

fn compile_not(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let sub_ctx = ctx.push_keyword("not");
    match compile_node(value, &sub_ctx) {
        Ok(node) => Some(Box::new(NotValidator::new(node))),
        Err(_) => None,
    }
}

fn compile_if_then_else(
    schema_obj: &serde_json::Map<String, Value>,
    ctx: &CompilerContext,
) -> Option<BoxedValidator> {
    let if_val = schema_obj.get("if")?;
    let if_ctx = ctx.push_keyword("if");
    let if_schema = compile_node(if_val, &if_ctx).ok()?;

    let then_schema = schema_obj
        .get("then")
        .map(|v| {
            let then_ctx = ctx.push_keyword("then");
            compile_node(v, &then_ctx)
        })
        .transpose()
        .ok()?;

    let else_schema = schema_obj
        .get("else")
        .map(|v| {
            let else_ctx = ctx.push_keyword("else");
            compile_node(v, &else_ctx)
        })
        .transpose()
        .ok()?;

    Some(Box::new(IfThenElseValidator::new(
        if_schema,
        then_schema,
        else_schema,
    )))
}

// ── Object keywords (sub-schema validators) ──────────────────────────

fn compile_properties(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let obj = value.as_object()?;
    let mut props = alloc::collections::BTreeMap::new();
    for (name, sub) in obj {
        let sub_ctx = ctx.push_keyword(&format!("properties/{name}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => {
                props.insert(name.clone(), node);
            }
            Err(_) => return None,
        }
    }
    Some(Box::new(PropertiesValidator::new(props)))
}

fn compile_additional_properties(
    value: &Value,
    ctx: &CompilerContext,
    defined_properties: &alloc::collections::BTreeSet<alloc::string::String>,
    pattern_regexes: &[(regex::Regex, alloc::string::String)],
) -> Option<BoxedValidator> {
    let schema = match value {
        Value::Bool(false) => Some(AdditionalSchema::False),
        Value::Object(_) | Value::Array(_) => {
            let sub_ctx = ctx.push_keyword("additionalProperties");
            match compile_node(value, &sub_ctx) {
                Ok(node) => Some(AdditionalSchema::Schema(node)),
                Err(_) => return None,
            }
        }
        _ => None, // true or other — allow all
    };
    Some(Box::new(AdditionalPropertiesValidator::new(
        schema,
        defined_properties.clone(),
        pattern_regexes
            .iter()
            .map(|(r, s)| (r.clone(), s.clone()))
            .collect(),
    )))
}

fn compile_pattern_properties(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let obj = value.as_object()?;
    let mut patterns = Vec::new();
    for (pattern, sub) in obj {
        let regex = regex::Regex::new(pattern).ok()?;
        let sub_ctx = ctx.push_keyword(&format!("patternProperties/{pattern}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => patterns.push((regex, node)),
            Err(_) => return None,
        }
    }
    Some(Box::new(PatternPropertiesValidator::new(patterns)))
}

fn compile_dependent_schemas(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let obj = value.as_object()?;
    let mut deps = alloc::collections::BTreeMap::new();
    for (name, sub) in obj {
        if sub.is_object() || sub.is_boolean() {
            let sub_ctx = ctx.push_keyword(&format!("dependentSchemas/{name}"));
            match compile_node(sub, &sub_ctx) {
                Ok(node) => {
                    deps.insert(name.clone(), node);
                }
                Err(_) => return None,
            }
        }
    }
    if deps.is_empty() {
        return None;
    }
    Some(Box::new(DependentSchemasValidator::new(deps)))
}

fn compile_unevaluated_properties(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let sub_ctx = ctx.push_keyword("unevaluatedProperties");
    match compile_node(value, &sub_ctx) {
        Ok(node) => Some(Box::new(UnevaluatedPropertiesValidator::new(node))),
        Err(_) => None,
    }
}

// ── Array keywords (sub-schema validators) ───────────────────────────

fn compile_items(
    value: &Value,
    ctx: &CompilerContext,
    schema_obj: &serde_json::Map<String, Value>,
) -> Option<BoxedValidator> {
    match value {
        Value::Bool(false) => {
            // items: false means no items allowed — tuple of length 0, additionalItems: false
            match ctx.draft {
                Draft::Draft202012 => {
                    let sub_ctx = ctx.push_keyword("items");
                    match compile_node(value, &sub_ctx) {
                        Ok(node) => Some(Box::new(ItemsValidator::new(node))),
                        Err(_) => None,
                    }
                }
                _ => Some(Box::new(TupleItemsValidator::new(
                    vec![],
                    AdditionalItemsPolicy::RejectAll,
                ))),
            }
        }
        Value::Object(_) => {
            let sub_ctx = ctx.push_keyword("items");
            match compile_node(value, &sub_ctx) {
                Ok(node) => {
                    // In Draft 2020-12, items only applies to items beyond prefixItems length.
                    let skip_first = if matches!(ctx.draft, Draft::Draft202012) {
                        schema_obj
                            .get("prefixItems")
                            .and_then(Value::as_array)
                            .map_or(0, std::vec::Vec::len)
                    } else {
                        0
                    };
                    if skip_first > 0 {
                        Some(Box::new(ItemsValidator::with_offset(node, skip_first)))
                    } else {
                        Some(Box::new(ItemsValidator::new(node)))
                    }
                }
                Err(_) => None,
            }
        }
        Value::Array(arr) => {
            // Tuple validation (Draft 4/6/7/2019-09)
            // Draft 2020-12: items as array is not valid, prefixItems handles tuples.
            if ctx.draft == Draft::Draft202012 {
                None
            } else {
                let mut schemas = Vec::new();
                for (i, sub) in arr.iter().enumerate() {
                    let sub_ctx = ctx.push_keyword(&format!("items/{i}"));
                    match compile_node(sub, &sub_ctx) {
                        Ok(node) => schemas.push(node),
                        Err(_) => return None,
                    }
                }
                // Check for additionalItems in the same schema object
                let additional = schema_obj
                    .get("additionalItems")
                    .map_or(AdditionalItemsPolicy::AllowAny, |v| {
                        compile_additional_schema(v, ctx)
                    });
                Some(Box::new(TupleItemsValidator::new(schemas, additional)))
            }
        }
        _ => None,
    }
}

/// Compile the `additionalItems` keyword (Draft 4/6/7/2019-09 only).
///
/// This is called from `compile_items` when `items` is an array, or as a
/// standalone keyword when `items` is a single schema (in which case it
/// should be ignored per spec, but we handle it gracefully).
fn compile_additional_items(
    _value: &Value,
    _ctx: &CompilerContext,
    schema_obj: &serde_json::Map<String, Value>,
) -> Option<BoxedValidator> {
    // additionalItems only applies when items is an array (tuple form).
    // If items is a single schema or not present, additionalItems is ignored.
    // We detect this by checking if items is an array.
    match schema_obj.get("items") {
        Some(Value::Array(_arr)) => {
            // Already handled by compile_items — skip to avoid double-compilation
            None
        }
        _ => {
            // items is not an array — additionalItems is ignored per spec
            None
        }
    }
}

/// Compile the `additionalItems` schema value into an `AdditionalItemsPolicy`.
fn compile_additional_schema(value: &Value, ctx: &CompilerContext) -> AdditionalItemsPolicy {
    match value {
        Value::Bool(false) => AdditionalItemsPolicy::RejectAll,
        Value::Object(_) => {
            let sub_ctx = ctx.push_keyword("additionalItems");
            match compile_node(value, &sub_ctx) {
                Ok(node) => AdditionalItemsPolicy::Validate(node),
                Err(_) => AdditionalItemsPolicy::AllowAny,
            }
        }
        _ => AdditionalItemsPolicy::AllowAny,
    }
}

fn compile_prefix_items(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let arr = value.as_array()?;
    let mut items = Vec::new();
    for (i, sub) in arr.iter().enumerate() {
        let sub_ctx = ctx.push_keyword(&format!("prefixItems/{i}"));
        match compile_node(sub, &sub_ctx) {
            Ok(node) => items.push(node),
            Err(_) => return None,
        }
    }
    Some(Box::new(PrefixItemsValidator::new(items)))
}

fn compile_contains(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let sub_ctx = ctx.push_keyword("contains");
    match compile_node(value, &sub_ctx) {
        Ok(node) => Some(Box::new(ContainsValidator::new(node))),
        Err(_) => None,
    }
}

fn compile_unevaluated_items(value: &Value, ctx: &CompilerContext) -> Option<BoxedValidator> {
    let sub_ctx = ctx.push_keyword("unevaluatedItems");
    match compile_node(value, &sub_ctx) {
        Ok(node) => Some(Box::new(UnevaluatedItemsValidator::new(node))),
        Err(_) => None,
    }
}

// ── Reference keywords ─────────────────────────────────────────────────

fn compile_ref(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    let reference = value.as_str()?.to_string();
    let resolved = ctx.resolver.lookup(&reference).ok()?;
    let target_uri = resolved.resolver().base_uri().to_string();

    // Circular reference — return AlwaysValid to break the cycle
    if ctx.is_in_progress(&target_uri) {
        return Some(Box::new(RefValidator::new(
            reference,
            ctx.schema_path.clone(),
            SchemaNode::AlwaysValid,
        )));
    }

    ctx.mark_in_progress(&target_uri);
    let sub_ctx = ctx.push_keyword("$ref");
    let resolved_schema = compile_node(resolved.contents(), &sub_ctx).ok()?;
    ctx.mark_done(&target_uri);

    Some(Box::new(RefValidator::new(
        reference,
        ctx.schema_path.clone(),
        resolved_schema,
    )))
}

fn compile_dynamic_ref(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    let reference = value.as_str()?.to_string();
    let resolved = ctx.resolver.lookup(&reference).ok()?;
    let target_uri = resolved.resolver().base_uri().to_string();

    if ctx.is_in_progress(&target_uri) {
        return Some(Box::new(DynamicRefValidator::new(
            reference,
            ctx.schema_path.clone(),
            SchemaNode::AlwaysValid,
        )));
    }

    ctx.mark_in_progress(&target_uri);
    let sub_ctx = ctx.push_keyword("$dynamicRef");
    let resolved_schema = compile_node(resolved.contents(), &sub_ctx).ok()?;
    ctx.mark_done(&target_uri);

    Some(Box::new(DynamicRefValidator::new(
        reference,
        ctx.schema_path.clone(),
        resolved_schema,
    )))
}

fn compile_recursive_ref(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    let reference = value.as_str()?.to_string();
    // For $recursiveRef, use the resolver's lookup_recursive_ref if the reference
    // is "#" (self-reference), otherwise use normal lookup
    let resolved = if reference == "#" || reference.is_empty() {
        ctx.resolver.lookup_recursive_ref().ok()?
    } else {
        ctx.resolver.lookup(&reference).ok()?
    };
    let target_uri = resolved.resolver().base_uri().to_string();

    if ctx.is_in_progress(&target_uri) {
        return Some(Box::new(RecursiveRefValidator::new(
            reference,
            ctx.schema_path.clone(),
            SchemaNode::AlwaysValid,
        )));
    }

    ctx.mark_in_progress(&target_uri);
    let sub_ctx = ctx.push_keyword("$recursiveRef");
    let resolved_schema = compile_node(resolved.contents(), &sub_ctx).ok()?;
    ctx.mark_done(&target_uri);

    Some(Box::new(RecursiveRefValidator::new(
        reference,
        ctx.schema_path.clone(),
        resolved_schema,
    )))
}

// ── Content ────────────────────────────────────────────────────────────

fn compile_content_encoding(
    value: &Value,
    ctx: &CompilerContext<'_>,
    schema_obj: &serde_json::Map<String, Value>,
) -> Option<BoxedValidator> {
    let encoding = value.as_str()?.to_string();
    // If contentMediaType is also present, use the combined validator instead.
    // The combined validator decodes first, then checks media type.
    if schema_obj.contains_key("contentMediaType") {
        return None; // Handled by compile_content_media_type
    }
    Some(Box::new(ContentEncodingValidator::new(
        &encoding,
        ctx.schema_path.clone(),
    )))
}

fn compile_content_media_type(
    value: &Value,
    ctx: &CompilerContext<'_>,
    schema_obj: &serde_json::Map<String, Value>,
) -> BoxedValidator {
    let media_type = value.as_str().unwrap_or("").to_string();
    // If contentSchema is also present, contentSchema handles the media type check internally.
    if schema_obj.contains_key("contentSchema") {
        return Box::new(ContentMediaTypeValidator::new(
            media_type,
            ctx.schema_path.clone(),
        ));
    }
    // If contentEncoding is also present, create a combined validator that
    // decodes first, then checks media type on the decoded result.
    if let Some(Value::String(encoding)) = schema_obj.get("contentEncoding") {
        let mut enc_path = ctx.schema_path.clone();
        enc_path.push_property("contentEncoding");
        let mut mt_path = ctx.schema_path.clone();
        mt_path.push_property("contentMediaType");
        return Box::new(ContentCombinedValidator::new(
            ContentEncoding::from_name(encoding),
            media_type,
            enc_path,
            mt_path,
        ));
    }
    Box::new(ContentMediaTypeValidator::new(
        media_type,
        ctx.schema_path.clone(),
    ))
}

/// Compile the `contentSchema` keyword (Draft 2019-09+).
///
/// When combined with `contentEncoding` and/or `contentMediaType`, the
/// `ContentSchemaValidator` handles decoding and JSON parsing before validating
/// the resulting value against the schema.
fn compile_content_schema(
    value: &Value,
    ctx: &CompilerContext<'_>,
    schema_obj: &serde_json::Map<String, Value>,
) -> Option<BoxedValidator> {
    let sub_ctx = ctx.push_keyword("contentSchema");
    let schema = compile_node(value, &sub_ctx).ok()?;

    // Extract encoding if present
    let encoding = schema_obj
        .get("contentEncoding")
        .and_then(Value::as_str)
        .map(ContentEncoding::from_name);

    // Extract media type if present
    let media_type = schema_obj
        .get("contentMediaType")
        .and_then(Value::as_str)
        .map(String::from);

    Some(Box::new(ContentSchemaValidator::new(
        schema,
        encoding,
        media_type,
        ctx.schema_path.clone(),
    )))
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

fn compile_unique_items(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    if value.as_bool().unwrap_or(false) {
        Some(Box::new(UniqueItemsValidator::new(ctx.schema_path.clone())))
    } else {
        None
    }
}

// ── Legacy keywords ────────────────────────────────────────────────────

fn compile_dependencies(value: &Value, ctx: &CompilerContext<'_>) -> Option<BoxedValidator> {
    let obj = value.as_object()?;
    let mut deps = alloc::collections::BTreeMap::new();
    for (name, dep_value) in obj {
        if let Some(arr) = dep_value.as_array() {
            let props: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if !props.is_empty() {
                deps.insert(name.clone(), Dependency::Required(props));
            }
        } else if dep_value.is_object() || dep_value.is_boolean() {
            let sub_ctx = ctx.push_keyword(&format!("dependencies/{name}"));
            if let Ok(schema) = compile_node(dep_value, &sub_ctx) {
                deps.insert(name.clone(), Dependency::Schema(schema));
            }
        }
    }
    if deps.is_empty() {
        return None;
    }
    Some(Box::new(DependenciesValidator::new(deps)))
}
