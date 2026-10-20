//! Unified code generator for OpenAPI specs.
//!
//! WHY: Generates cohesive per-endpoint units (types + clients + provider impl).
//!
//! WHAT: For each endpoint in a group, generates all related code together.
//!
//! HOW: String templates for each endpoint unit, grouped into modules.

use crate::{sanitize_identifier, to_pascal_case, to_snake_case, EndpointInfo, ResponseType};
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

use super::analyzer::ApiGroup;
use super::emit::{self, Field, ModuleFile, TypeDef, ARGS_DERIVES, SCHEMA_DERIVES};

/// How deep `allOf` members are followed through `$ref`s before giving up — a
/// guard against a reference cycle, far past any real spec's nesting.
const MAX_ALL_OF_DEPTH: usize = 16;

/// Merge one `allOf` member's properties (and required names) into the
/// composed type's.
///
/// A member is an inline object, a `$ref` to one, or itself an `allOf`. The
/// `$ref` case is the common one — Docker's `ContainerCreateRequest` is
/// `allOf: [$ref ContainerConfig, {HostConfig, NetworkingConfig}]` — and it was
/// skipped: only inline properties were merged, so the composed type lost every
/// field of the type it extends.
fn collect_all_of_member(
    member: &crate::spec::Schema,
    schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
    properties: &mut std::collections::BTreeMap<String, (String, crate::spec::Schema)>,
    required: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > MAX_ALL_OF_DEPTH {
        return;
    }
    if let Some(ref_path) = &member.ref_path {
        let name = ref_path
            .trim_start_matches("#/components/schemas/")
            .trim_start_matches("#/schemas/");
        if let Some(target) = schemas.get(name) {
            collect_all_of_member(target, schemas, properties, required, depth + 1);
        }
        return;
    }
    if let Some(nested) = &member.all_of {
        for inner in nested {
            collect_all_of_member(inner, schemas, properties, required, depth + 1);
        }
    }
    if let Some(props) = &member.properties {
        for (k, v) in props {
            // Deduplicate by snake_case field name to avoid duplicates like
            // "Version" and "version"; a later member's definition wins.
            properties.insert(field_ident(k), (k.clone(), v.clone()));
        }
    }
    for r in &member.required {
        required.insert(r.clone());
    }
}

/// Rust keywords that must be escaped when used as identifiers.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "typeof", "unsized", "virtual", "yield", "try", "union", "raw",
];

/// Escape a Rust keyword by prefixing with `r#`.
/// NOTE: This does NOT work for field names — use `escape_field_keyword` instead.
fn escape_rust_keyword(ident: &str) -> String {
    if RUST_KEYWORDS.contains(&ident) {
        format!("r#{}", ident)
    } else {
        ident.to_string()
    }
}

/// Escape a Rust keyword for use in a struct field name.
/// `r#self` and `r#Self` are NOT valid field names in Rust, so we rename instead.
/// A property key, turned into a field name Rust will accept.
///
/// **Why not `escape_field_keyword(to_snake_case(key))`, which is what this was:**
/// that handles keywords and casing but assumes the key is otherwise already an
/// identifier. Vendors do not cooperate — Cloudflare keys properties `13335` (an
/// ASN), `*` and `$metadata`; Linode uses `+and`/`+gt` for filter operators;
/// DigitalOcean has `pg_partman_bgw.interval`. Those went straight into the
/// output as `pub 13335: …`, which does not parse.
///
/// The wire name is preserved regardless: the caller emits `#[serde(rename)]`
/// whenever the field name differs from the property name, which is exactly when
/// this function changed something.
///
/// Names that are already valid pass through untouched, so this does not churn
/// any provider's committed output.
fn field_ident(prop_name: &str) -> String {
    let snake = to_snake_case(&sanitize_identifier(prop_name));
    // A key made entirely of punctuation sanitises to nothing — Cloudflare has a
    // property literally named `*`, which produced `pub : Type`.
    if snake.is_empty() {
        return "field".to_string();
    }
    // No identifier may start with a digit (Cloudflare keys properties by ASN).
    let snake = if snake.starts_with(|c: char| c.is_ascii_digit()) {
        format!("field_{snake}")
    } else {
        snake
    };
    escape_field_keyword(&snake)
}

/// Whether the vendor says this field's value may be `null`.
///
/// **Follows `$ref`, and must.** Nullability lives on the *schema*, and hoisting
/// moves the schema. Hetzner declares
/// `"deprecation": { "type": ["object","null"], "properties": {…} }` — a required,
/// nullable, inline object. `normalize_nullable_types` records `nullable: true` on
/// it, then `extract_inline_schemas` lifts the whole thing into
/// `components/schemas` and leaves the property as a bare `$ref` that says nothing
/// about null. Checking only the property generated
/// `pub deprecation: …IsoDeprecation`, and the first real `create_server` died with
/// `invalid type: null, expected struct …IsoDeprecation` — after Hetzner had
/// already built the machine.
///
/// Bounded rather than recursive: a `$ref` to a `$ref` is already unusual, and a
/// cycle here would hang the generator rather than fail it.
fn is_nullable(
    schema: &crate::spec::Schema,
    schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
) -> bool {
    let mut current = schema;
    for _ in 0..8 {
        if current.nullable == Some(true) {
            return true;
        }
        let Some(ref_path) = &current.ref_path else {
            return false;
        };
        let name = ref_path
            .trim_start_matches("#/components/schemas/")
            .trim_start_matches("#/schemas/");
        match UnifiedGenerator::resolve_schema_key(name, schemas) {
            Some(target) => current = target,
            None => return false,
        }
    }
    false
}

fn escape_field_keyword(ident: &str) -> String {
    if ident == "self" {
        "_self".to_string()
    } else if RUST_KEYWORDS.contains(&ident) {
        format!("r#{}", ident)
    } else {
        ident.to_string()
    }
}

/// Sanitize a module name so it can be used in `pub mod NAME;`.
/// `r#move` is invalid syntax, so we rename problematic keywords with a `_` suffix.
fn sanitize_module_name(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("{}_mod", name)
    } else {
        name.to_string()
    }
}

/// Rename a type that conflicts with Rust std types by adding a suffix.
fn rename_std_type_conflict(type_name: &str) -> String {
    // Types that conflict with std or common types
    let std_types = [
        "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "Cow", "HashMap", "BTreeMap",
    ];
    if std_types.contains(&type_name) {
        format!("{}Type", type_name)
    } else {
        type_name.to_string()
    }
}

/// Extract inner type names from a type string (handles Vec<T>, Option<T>, etc.).
/// Adds extracted type names to the all_types set.
fn extract_type_names_from_generic(
    type_str: &str,
    all_types: &mut std::collections::BTreeSet<String>,
) {
    // Skip invalid types
    if type_str == "()" || type_str == "serde_json::Value" {
        return;
    }
    // Skip if it's just a generic wrapper without a real type
    if [
        "Vec", "Option", "HashMap", "BTreeMap", "Box", "Rc", "Arc", "Cow",
    ]
    .contains(&type_str)
    {
        return;
    }
    // If it's a generic type like Vec<T>, extract T
    if type_str.contains('<') && type_str.contains('>') {
        // Extract content between < and >
        if let Some(start) = type_str.find('<') {
            if let Some(end) = type_str.rfind('>') {
                let inner = type_str[start + 1..end].trim();
                // Recursively extract (handles nested generics like Vec<Option<T>>)
                extract_type_names_from_generic(inner, all_types);
            }
        }
        return;
    }
    // Skip types with :: (path types)
    if type_str.contains("::") {
        return;
    }
    // Rename std type conflicts and add the type
    let safe_name = rename_std_type_conflict(type_str);
    all_types.insert(safe_name);
}

/// Collect all type names referenced via $ref in a schema, recursively.
/// Adds found type names (in PascalCase) to seen_types and types_to_process.
fn collect_referenced_type_names(
    schema: &crate::spec::Schema,
    seen_types: &mut std::collections::BTreeSet<String>,
    types_to_process: &mut Vec<String>,
) {
    // Check for direct $ref
    if let Some(ref_path) = &schema.ref_path {
        let ref_name = ref_path
            .trim_start_matches("#/components/schemas/")
            .trim_start_matches("#/schemas/");
        let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
        if seen_types.insert(safe_name) {
            types_to_process.push(ref_name.to_string());
        }
        return;
    }

    // Recursively collect from allOf
    if let Some(all_of) = &schema.all_of {
        for member in all_of {
            collect_referenced_type_names(member, seen_types, types_to_process);
        }
    }

    // Recursively collect from oneOf
    if let Some(one_of) = &schema.one_of {
        for member in one_of {
            collect_referenced_type_names(member, seen_types, types_to_process);
        }
    }

    // Recursively collect from anyOf
    if let Some(any_of) = &schema.any_of {
        for member in any_of {
            collect_referenced_type_names(member, seen_types, types_to_process);
        }
    }

    // Collect from properties
    if let Some(properties) = &schema.properties {
        for (_prop_name, prop_schema) in properties {
            // Check if property is a $ref
            if let Some(ref_path) = &prop_schema.ref_path {
                let ref_name = ref_path
                    .trim_start_matches("#/components/schemas/")
                    .trim_start_matches("#/schemas/");
                let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
                if seen_types.insert(safe_name) {
                    types_to_process.push(ref_name.to_string());
                }
            }
            // Check if property is an array with $ref items
            else if prop_schema.schema_type.as_deref() == Some("array") {
                if let Some(items) = &prop_schema.items {
                    if let Some(ref_path) = &items.ref_path {
                        let ref_name = ref_path
                            .trim_start_matches("#/components/schemas/")
                            .trim_start_matches("#/schemas/");
                        let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
                        if seen_types.insert(safe_name) {
                            types_to_process.push(ref_name.to_string());
                        }
                    } else {
                        // Recursively process array items
                        collect_referenced_type_names(items, seen_types, types_to_process);
                    }
                }
            }
            // Recursively process object properties
            else if prop_schema.schema_type.as_deref() == Some("object") {
                collect_referenced_type_names(prop_schema, seen_types, types_to_process);
            }
        }
    }

    // Top-level array items: when the schema is *itself* an array
    // (`{type: array, items: {...}}`), follow the element schema. Without this,
    // a `$ref` to a standalone array schema (e.g. an Access "exclude" list whose
    // items `$ref` `access_rule`) collects the array wrapper but never its
    // element type, so the element type is referenced in generated field
    // positions (`Vec<AccessRule>`) yet never emitted — an `E0425` at compile.
    if let Some(items) = &schema.items {
        if let Some(ref_path) = &items.ref_path {
            let ref_name = ref_path
                .trim_start_matches("#/components/schemas/")
                .trim_start_matches("#/schemas/");
            let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
            if seen_types.insert(safe_name) {
                types_to_process.push(ref_name.to_string());
            }
        } else {
            collect_referenced_type_names(items, seen_types, types_to_process);
        }
    }
}

/// Build a map of which types reference which other types (dependency graph).
/// Returns: Map<type_name, Set<types it references>>
fn build_type_dependencies(
    schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let mut deps: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();

    for (name, schema) in schemas {
        let safe_name = rename_std_type_conflict(&crate::to_pascal_case(name));
        let mut refs_set = std::collections::HashSet::new();
        collect_refs_from_schema(schema, &mut refs_set, schemas);
        deps.insert(safe_name, refs_set);
    }

    deps
}

fn collect_refs_from_schema(
    schema: &crate::spec::Schema,
    refs: &mut std::collections::HashSet<String>,
    schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
) {
    // Check for direct $ref
    if let Some(ref_path) = &schema.ref_path {
        let ref_name = ref_path
            .trim_start_matches("#/components/schemas/")
            .trim_start_matches("#/schemas/");
        let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
        refs.insert(safe_name);
        return;
    }

    // Recursively collect from allOf/oneOf/anyOf
    for member in schema
        .all_of
        .iter()
        .flatten()
        .chain(schema.one_of.iter().flatten())
        .chain(schema.any_of.iter().flatten())
    {
        collect_refs_from_schema(member, refs, schemas);
    }

    // Collect from properties
    if let Some(properties) = &schema.properties {
        for (_k, prop) in properties {
            if let Some(ref_path) = &prop.ref_path {
                let ref_name = ref_path
                    .trim_start_matches("#/components/schemas/")
                    .trim_start_matches("#/schemas/");
                let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
                refs.insert(safe_name);
            } else if prop.schema_type.as_deref() == Some("array") {
                if let Some(items) = &prop.items {
                    if let Some(ref_path) = &items.ref_path {
                        let ref_name = ref_path
                            .trim_start_matches("#/components/schemas/")
                            .trim_start_matches("#/schemas/");
                        let safe_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
                        refs.insert(safe_name);
                    }
                }
            }
        }
    }
}

/// Find all types that are part of a recursive cycle (self or mutual recursion).
fn find_recursive_types(
    deps: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashSet<String> {
    let mut recursive = std::collections::HashSet::new();

    for type_name in deps.keys() {
        // DFS from this type to see if it can reach itself
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<String> = vec![type_name.clone()];
        let mut found_cycle = false;

        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            if let Some(references) = deps.get(&current) {
                for referenced in references {
                    if referenced == type_name {
                        found_cycle = true;
                        break;
                    }
                    if !visited.contains(referenced) {
                        stack.push(referenced.clone());
                    }
                }
            }
            if found_cycle {
                break;
            }
        }

        if found_cycle {
            recursive.insert(type_name.clone());
        }
    }

    recursive
}

/// Given a field type name, wrap it in Box<> if it's recursive.
fn maybe_box_type(field_type: &str, recursive_types: &std::collections::HashSet<String>) -> String {
    // Handle Option<T> -> Option<Box<T>> if T is recursive
    if let Some(inner) = field_type
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix(">"))
    {
        if recursive_types.contains(inner) {
            return format!("Option<Box<{}>>", inner);
        }
    }
    // Handle Vec<T> -> Vec<Box<T>> if T is recursive
    if let Some(inner) = field_type
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix(">"))
    {
        if recursive_types.contains(inner) {
            return format!("Vec<Box<{}>>", inner);
        }
    }
    // Handle Option<Vec<T>> -> Option<Vec<Box<T>>> if T is recursive
    if let Some(inner) = field_type
        .strip_prefix("Option<Vec<")
        .and_then(|s| s.strip_suffix(">>"))
    {
        if recursive_types.contains(inner) {
            return format!("Option<Vec<Box<{}>>>", inner);
        }
    }
    // Direct type reference that is recursive
    if recursive_types.contains(field_type) {
        return format!("Box<{}>", field_type);
    }

    field_type.to_string()
}

/// Whether `ty` mentions the identifier `name` (as a whole word).
fn type_ident_in(ty: &str, name: &str) -> bool {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|ident| ident == name)
}

/// Transform an OpenAPI path into a Rust format! string.
/// Converts `{param}` placeholders to `{}` only for params in path_params.
/// Other braces are escaped as literal braces.
/// Returns the escaped path and the list of param names in URL order.
fn escape_url_for_format(path: &str, path_params: &[String]) -> (String, Vec<String>) {
    let mut result = String::new();
    let mut params_in_url_order = Vec::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                // Check if this is a path parameter like {param}
                let mut param_content = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '}' {
                        chars.next(); // consume the closing brace
                                      // Check if this param is in our path_params list.
                                      // Strip GCP resource expansion prefix (+) from param_content.
                        let normalized = param_content.strip_prefix('+').unwrap_or(&param_content);
                        if let Some(matching_param) = path_params.iter().find(|p| {
                            let sanitized = sanitize_identifier(p);
                            sanitized == normalized || **p == param_content || **p == normalized
                        }) {
                            // This is a known path parameter - replace with {}
                            result.push_str("{}");
                            params_in_url_order
                                .push(to_snake_case(&sanitize_identifier(matching_param)));
                        } else {
                            // Unknown param - escape as literal braces
                            result.push_str(&format!("{{{{{}}}}}", param_content));
                        }
                        break;
                    } else {
                        param_content.push(chars.next().unwrap());
                    }
                }
            }
            '}' => {
                // Standalone closing brace - escape it
                result.push_str("}}");
            }
            _ => result.push(c),
        }
    }

    (result, params_in_url_order)
}

/// Sanitize a group name for use as a directory/file name and Rust identifier.
/// Converts PascalCase to snake_case, removes redundant words, and normalizes identifiers.
fn sanitize_group_name(name: &str) -> String {
    // Step 1: Convert PascalCase/CamelCase to snake_case
    // Insert underscore before each uppercase letter that follows a lowercase letter or digit
    let mut snake_case = String::new();
    let mut prev_was_upper_or_digit = false;

    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            // Insert underscore before uppercase if previous char was lowercase or digit
            if i > 0 && prev_was_upper_or_digit == false {
                snake_case.push('_');
            }
            snake_case.push(c.to_ascii_lowercase());
            prev_was_upper_or_digit = true;
        } else if c.is_numeric() {
            snake_case.push(c);
            prev_was_upper_or_digit = true;
        } else {
            snake_case.push(c);
            prev_was_upper_or_digit = false;
        }
    }

    // Step 2: Replace any non-alphanumeric characters (except underscore) with underscore
    let sanitized: String = snake_case
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Step 3: Collapse multiple underscores into one
    let collapsed: String = sanitized
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    // Step 4: Remove common redundant words/patterns
    let normalized = collapse_redundant_words(&collapsed);

    // Step 5: Ensure starts with a letter (not digit or underscore)
    let mut result = normalized;
    while result.starts_with(|c: char| c.is_numeric() || c == '_') {
        result.remove(0);
    }

    // Step 6: Truncate long names while preserving uniqueness
    if result.len() > 50 {
        // Keep first 40 chars + underscore + hash of last 10
        let hash = result.len() % 1000;
        result.truncate(40);
        result = format!("{}_{}", result, hash);
    }

    if result.is_empty() {
        "group".to_string()
    } else {
        result
    }
}

/// Remove redundant words and normalize common patterns.
fn collapse_redundant_words(name: &str) -> String {
    let segments: Vec<&str> = name.split('_').collect();
    let mut result = Vec::new();
    let mut prev_segment: Option<&str> = None;

    for segment in segments {
        // Skip duplicate consecutive segments (e.g., "rules_rules" → "rules")
        if prev_segment == Some(segment) {
            continue;
        }

        // Skip common filler words that don't add meaning
        let skip_words = ["api", "v1", "v2", "v3", "the", "and", "for", "with"];
        if skip_words.contains(&segment.to_lowercase().as_str()) {
            prev_segment = Some(segment);
            continue;
        }

        // Normalize common abbreviations - just pass through as lowercase
        result.push(segment.to_lowercase());
        prev_segment = Some(segment);
    }

    result.join("_")
}

/// Rewrite the provider's feature flags in a crate's `Cargo.toml`.
///
/// WHY: the generated modules are gated on `<provider>_<group>` features, so the
/// manifest has to list exactly the groups that exist after a regeneration.
///
/// WHAT: replaces every `<provider>_*` feature with one empty feature per group,
/// `<provider>_all` and `<provider>` (both enabling every group), and — for a
/// sub-provider such as `gcp_admin` — refreshes the parent feature.
///
/// HOW: edits the document with `toml_edit`, so everything this function does
/// not own (comments, table order, formatting, other features) is left exactly
/// as written. It used to round-trip the file through `toml::Value`, which
/// threw away every comment and re-sorted the tables, and it deleted
/// `<provider>_all` without re-adding it.
fn update_cargo_toml(
    provider: &str,
    groups: &[ApiGroup],
    cargo_toml_path: &Path,
) -> Result<(), std::io::Error> {
    let content = fs::read_to_string(cargo_toml_path)?;
    let group_names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
    let updated = rewrite_features_for_groups(&content, provider, &group_names)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if updated != content {
        fs::write(cargo_toml_path, updated)?;
    }
    Ok(())
}

/// Errors from [`rewrite_features_for_groups`].
#[derive(Debug, derive_more::Display)]
pub enum FeatureRewriteError {
    /// The manifest is not valid TOML.
    #[display("Cargo.toml does not parse: {_0}")]
    Parse(toml_edit::TomlError),
    /// The manifest has no `[features]` table to add the group features to.
    #[display("Cargo.toml has no [features] table")]
    MissingFeatures,
}

impl std::error::Error for FeatureRewriteError {}

/// The pure half of [`update_cargo_toml`]: the manifest text with the
/// provider's features rewritten for `group_names` (as the analyzer names the
/// groups, before sanitising), everything else untouched.
///
/// # Errors
/// [`FeatureRewriteError`] when the manifest does not parse or has no
/// `[features]` table.
pub fn rewrite_features_for_groups(
    manifest: &str,
    provider: &str,
    group_names: &[String],
) -> Result<String, FeatureRewriteError> {
    let mut doc: toml_edit::DocumentMut =
        manifest.parse().map_err(FeatureRewriteError::Parse)?;

    let provider_feature = provider.replace(['-', '/'], "_");
    let all_feature = format!("{provider_feature}_all");
    let group_prefix = format!("{provider_feature}_");

    let features = doc
        .get_mut("features")
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or(FeatureRewriteError::MissingFeatures)?;

    // A sub-provider (`gcp_admin`) also refreshes its parent (`gcp`).
    let parent_feature = provider_feature
        .split_once('_')
        .map(|(parent, _)| parent.to_string());

    // Group features, deduplicated and in a stable order.
    let mut group_features: Vec<String> = group_names
        .iter()
        .map(|name| {
            let safe = sanitize_module_name(&sanitize_group_name(name));
            format!("{provider_feature}_{safe}")
        })
        .collect();
    group_features.sort();
    group_features.dedup();

    // Drop this provider's old group features (a group can disappear when the
    // spec or the selection changes). `<provider>_all` is rebuilt below.
    let stale: Vec<String> = features
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| {
            k.starts_with(&group_prefix) && *k != all_feature && !group_features.contains(k)
        })
        .collect();
    for key in stale {
        features.remove(&key);
    }

    // One entry per line, as the manifests write feature lists.
    let feature_list = |names: &[String]| -> toml_edit::Item {
        let mut array = toml_edit::Array::new();
        for name in names {
            let mut entry = toml_edit::Value::from(name.as_str());
            entry.decor_mut().set_prefix("\n    ");
            array.push_formatted(entry);
        }
        if !names.is_empty() {
            array.set_trailing("\n");
            array.set_trailing_comma(true);
        }
        toml_edit::value(array)
    };

    let mut added = false;
    for feature in &group_features {
        // A group feature that already exists keeps whatever it enables.
        if features.get(feature).is_none() {
            features.insert(feature, feature_list(&[]));
            added = true;
        }
    }
    // `<provider>` and `<provider>_all` enable every group, and keep whatever
    // else was written into them by hand (Docker's `docker` also enables
    // `dep:tar`).
    for umbrella in [&provider_feature, &all_feature] {
        let mut entries: Vec<String> = features
            .get(umbrella)
            .and_then(toml_edit::Item::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    // Group features are re-listed below; anything else stays.
                    .filter(|e| !e.starts_with(&group_prefix))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        entries.extend(group_features.iter().cloned());
        added |= set_feature(features, umbrella, feature_list(&entries));
    }

    if let Some(parent) = parent_feature {
        // Every sub-provider feature (`gcp_admin`, `gcp_cloudkms`, …): exactly
        // two segments, and not the parent's own `gcp_all`.
        let mut sub_providers: Vec<String> = features
            .iter()
            .map(|(k, _)| k.to_string())
            .filter(|k| {
                k.strip_prefix(&format!("{parent}_"))
                    .is_some_and(|rest| !rest.contains('_') && rest != "all")
            })
            .collect();
        sub_providers.sort();
        added |= set_feature(features, &parent, feature_list(&sub_providers));
    }

    // New keys were appended at the end; place them in order. A manifest that
    // gained nothing is not re-sorted (its hand-written order stays).
    if added {
        features.sort_values();
    }

    Ok(doc.to_string())
}

/// Set a feature, assigning in place when the key exists: `insert` would
/// replace the key too, and with it any comment written above the feature.
///
/// A feature that already lists exactly these entries is left as it is, so
/// regenerating an up-to-date crate does not reformat its manifest.
///
/// Returns whether the key is new.
fn set_feature(features: &mut dyn toml_edit::TableLike, key: &str, item: toml_edit::Item) -> bool {
    let entries = |item: &toml_edit::Item| -> Option<Vec<String>> {
        item.as_array().map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
    };
    let sorted = |v: Option<Vec<String>>| {
        v.map(|mut v| {
            v.sort();
            v
        })
    };
    match features.get_mut(key) {
        // Same entries in any order: leave the hand-written order alone.
        Some(existing) if sorted(entries(existing)) == sorted(entries(&item)) => false,
        Some(existing) => {
            *existing = item;
            false
        }
        None => {
            features.insert(key, item);
            true
        }
    }
}

/// Format generated files with `rustfmt`.
///
/// The edition matters: `rustfmt` run on a bare file assumes Rust 2015, where
/// `async fn` does not parse — so every run used to fail, and the failure was
/// discarded (`let _ = …output()`), leaving the code unformatted without anyone
/// knowing. A formatter that cannot run is now an error.
fn format_files(paths: &[PathBuf]) -> Result<(), GenError> {
    if paths.is_empty() {
        return Ok(());
    }
    let output = std::process::Command::new("rustfmt")
        .args(["--edition", "2021"])
        .args(paths)
        .output()
        .map_err(|e| GenError::Rustfmt(format!("could not run rustfmt: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GenError::Rustfmt(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    }
}

/// Unified generator that produces cohesive per-endpoint units.
pub struct UnifiedGenerator {
    output_dir: PathBuf,
    /// Optional override: write files directly to this directory instead of
    /// `output_dir.join(provider)`. Used for providers split into their own
    /// crate (e.g. cloudflare → foundation_deployment_cloudflare/src).
    provider_dir_override: Option<PathBuf>,
}

/// Error type for generation failures.
#[derive(Debug, derive_more::Display)]
pub enum GenError {
    #[display("failed to read {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
    #[display("analysis failed: {_0}")]
    AnalysisFailed(String),
    #[display("fmt error: {_0}")]
    FmtError(std::fmt::Error),
    #[display("rustfmt failed on the generated code: {_0}")]
    Rustfmt(String),
}

impl std::error::Error for GenError {}

impl From<std::io::Error> for GenError {
    fn from(e: std::io::Error) -> Self {
        GenError::ReadFile {
            path: String::new(),
            source: e,
        }
    }
}

impl From<std::fmt::Error> for GenError {
    fn from(e: std::fmt::Error) -> Self {
        GenError::FmtError(e)
    }
}

impl UnifiedGenerator {
    pub fn new(output_dir: PathBuf) -> Self {
        Self { output_dir, provider_dir_override: None }
    }

    /// Override the provider output directory. When set, `generate()` writes
    /// files to this path instead of `output_dir.join(provider)`.
    #[must_use]
    pub fn with_provider_dir(mut self, dir: PathBuf) -> Self {
        self.provider_dir_override = Some(dir);
        self
    }

    /// Generate all artifacts for a provider as cohesive per-endpoint units.
    pub fn generate(
        &self,
        provider: &str,
        spec_content: &str,
        options: &super::analyzer::AnalysisOptions,
    ) -> Result<(), GenError> {
        use super::analyzer::analyze_spec;

        // Analyze spec
        let analysis = analyze_spec(spec_content, provider, options)
            .map_err(|e| GenError::AnalysisFailed(e.to_string()))?;

        let provider_root = self
            .provider_dir_override
            .clone()
            .unwrap_or_else(|| self.output_dir.join(provider));
        // All generated files live under `generated/` so hand-written code
        // in neighbouring directories is never overwritten by regeneration.
        let generated_dir = provider_root.join("generated");
        fs::create_dir_all(&generated_dir)?;

        // Generate shared/ module (always needed for ApiError/ApiResponse types)
        let shared = self.generate_shared_module(&analysis)?;
        let shared_names = shared.type_names();

        // Generate one module per group.
        let mut modules = vec![shared];
        for group in &analysis.groups {
            let module = self.generate_group_module(
                provider,
                group,
                &analysis.shared_resources,
                &analysis.schemas,
                &shared_names,
            )?;
            modules.push(module);
        }

        // Collapse the duplicates (see `emit`), then write and format.
        emit::shrink(&mut modules, 0);
        let mut written = Vec::with_capacity(modules.len());
        for module in &modules {
            let path = generated_dir.join(&module.rel_path);
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&path, module.render()?)?;
            written.push(path);
        }

        // Generate generated/mod.rs with feature guards — this becomes the
        // single entry point for all generated code.
        self.generate_provider_mod(provider, &analysis.groups, &analysis.shared_resources, &generated_dir)?;
        written.push(generated_dir.join("mod.rs"));
        format_files(&written)?;

        // For monolith providers, write a thin parent mod.rs that just re-exports
        // `generated/`. Only create it if the file doesn't already exist (so
        // users can replace it with hand-written code without losing it on the
        // next regeneration).
        if self.provider_dir_override.is_none() {
            let parent_mod = provider_root.join("mod.rs");
            if !parent_mod.exists() {
                fs::write(&parent_mod, "//! Auto-generated thin re-export module.\n//! Replace with hand-written code — this file will not be overwritten.\npub mod generated;\n")?;
            }
        }

        // Update Cargo.toml with missing feature flags.
        // For split-out providers (provider_dir_override set), output_dir IS the
        // crate root. For monolith providers, output_dir is
        // foundation_deployment/src/providers and the crate root is 2 levels up.
        let cargo_toml_path = if self.provider_dir_override.is_some() {
            self.output_dir.join("Cargo.toml")
        } else {
            self.output_dir
                .ancestors()
                .nth(2)
                .map(|p| p.join("Cargo.toml"))
                .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/Cargo.toml"))
        };

        if cargo_toml_path.exists() {
            update_cargo_toml(provider, &analysis.groups, &cargo_toml_path).map_err(|e| {
                GenError::WriteFile {
                    path: cargo_toml_path.display().to_string(),
                    source: e,
                }
            })?;
        }

        Ok(())
    }

    /// Generate a single group module with per-endpoint cohesive units.
    fn generate_group_module(
        &self,
        provider: &str,
        group: &ApiGroup,
        shared_resources: &[String],
        schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
        actual_shared_types: &HashSet<String>,
    ) -> Result<ModuleFile, GenError> {
        // Sanitize group name for use as directory/file name
        let safe_name = sanitize_group_name(&group.name);
        let safe_name = sanitize_module_name(&safe_name);
        let mut module = ModuleFile::new(format!("{safe_name}/mod.rs"));

        let mut out = String::new();

        // File header
        let provider_safe = provider.replace('-', "_").replace('/', "_");
        writeln!(
            out,
            "//! Auto-generated API module for {} {}.",
            provider, group.name
        )?;
        writeln!(out, "//!")?;
        writeln!(
            out,
            "//! Generated by `cargo run --bin ewe_platform gen_api`."
        )?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Feature flag: `{}_{} `", provider_safe, safe_name)?;
        writeln!(out)?;
        writeln!(
            out,
            "#![cfg(feature = \"{}_{}\")]",
            provider_safe, safe_name
        )?;
        writeln!(
            out,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity)]"
        )?;
        writeln!(
            out,
            "#![allow(clippy::missing_errors_doc, clippy::doc_markdown, clippy::useless_format)]"
        )?;
        writeln!(out, "#![allow(unused_imports)]")?;
        writeln!(out)?;

        // Common imports — only what's actually used in the generated code
        writeln!(
            out,
            "use foundation_netio::{{DynNetClient, PreparedRequestBuilder}};"
        )?;
        writeln!(
            out,
            "use foundation_netio::shared::client::http_client::HttpClient;"
        )?;
        writeln!(out, "use serde::{{Deserialize, Serialize}};")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out)?;

        // Collect shared types actually used by this group's endpoints.
        let mut used_shared_types: Vec<(String, String)> = Vec::new(); // (original_name, renamed_name)
        for type_name in shared_resources {
            // Skip generic types and paths
            if type_name.contains('<') || type_name.contains('>') || type_name.contains("::") {
                continue;
            }
            if !type_name
                .chars()
                .next()
                .map(|c| c.is_alphabetic() || c == '_')
                .unwrap_or(false)
            {
                continue;
            }
            if [
                "Vec", "Option", "HashMap", "BTreeMap", "Box", "Rc", "Arc", "Cow",
            ]
            .contains(&type_name.as_str())
            {
                continue;
            }
            // Check if any endpoint in this group uses this type directly
            let is_used = group.endpoints.iter().any(|ep| {
                ep.response_type
                    .as_ref()
                    .is_some_and(|rt| rt.as_rust_type() == type_name)
                    || ep.request_type.as_ref().is_some_and(|rt| rt == type_name)
            });
            if is_used {
                let renamed = rename_std_type_conflict(type_name);
                used_shared_types.push((type_name.clone(), renamed));
            }
        }

        // Shared type imports are written after transitive type collection below

        // Track types we've already generated (for shared types)
        let mut generated_types: HashSet<String> = HashSet::new();

        // Collect all endpoint units for this group
        // We'll generate: types, then client functions, then provider impl
        // But organized per-endpoint for readability

        // First pass: collect all unique types from endpoint response/request types
        // BTreeSet, not HashSet: this set's iteration order IS the order types are
        // emitted into the file (below). Rust seeds a HashSet's hasher randomly
        // per process, so a HashSet here meant two runs of the same command
        // produced the same types in a different order — which made `check()`
        // report Stale forever, had every build regenerate, and churned 833 lines
        // of diff for nothing. Generated code that is committed must be a pure
        // function of its inputs.
        let mut all_types: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for ep in &group.endpoints {
            if let Some(rt) = &ep.response_type {
                let type_name = rt.as_rust_type();
                extract_type_names_from_generic(type_name, &mut all_types);
            }
            if let Some(rt) = &ep.request_type {
                extract_type_names_from_generic(rt, &mut all_types);
            }
        }

        // Second pass: collect all transitively referenced types from schemas.
        // This ensures nested types (e.g., types referenced via $ref in properties)
        // are also generated.
        //
        // all_types contains PascalCase names, but schemas are keyed by the spec's
        // original names. Use resolve_schema_key to bridge the gap.
        let mut types_to_process: Vec<String> = all_types.iter().cloned().collect();
        let mut already_traversed: std::collections::HashSet<String> = std::collections::HashSet::new();

        while let Some(type_name) = types_to_process.pop() {
            if already_traversed.contains(&type_name.to_lowercase()) {
                continue;
            }
            if let Some(schema) = Self::resolve_schema_key(&type_name, schemas) {
                already_traversed.insert(type_name.to_lowercase());
                collect_referenced_type_names(schema, &mut all_types, &mut types_to_process);
            }
        }
        // Converging pass: after traversing all $ref chains, scan every generated
        // type's schema for properties that reference other schemas via $ref. Any
        // target type not yet known gets generated too. Repeat until no new types
        // are discovered.
        loop {
            let before = all_types.len();
            let snapshot: Vec<String> = all_types.iter().cloned().collect();
            for type_name in &snapshot {
                if let Some(schema) = Self::resolve_schema_key(type_name, schemas) {
                    collect_referenced_type_names(schema, &mut all_types, &mut types_to_process);
                }
            }
            while let Some(type_name) = types_to_process.pop() {
                if already_traversed.contains(&type_name.to_lowercase()) {
                    continue;
                }
                if let Some(schema) = Self::resolve_schema_key(&type_name, schemas) {
                    already_traversed.insert(type_name.to_lowercase());
                    collect_referenced_type_names(schema, &mut all_types, &mut types_to_process);
                }
            }
            if all_types.len() == before {
                break;
            }
        }

        // Add shared types found during transitive collection that weren't caught by direct endpoint check
        for type_name in &all_types {
            if (shared_resources.contains(type_name) || actual_shared_types.contains(type_name))
                && !used_shared_types.iter().any(|(orig, _)| orig == type_name)
            {
                let renamed = rename_std_type_conflict(type_name);
                used_shared_types.push((type_name.clone(), renamed));
            }
        }

        // Ensure Empty and Operation are imported when used anywhere (endpoint responses, struct fields, etc.).
        // These are always provided by crate::providers::common via super::shared, but may
        // not appear in the group's own shared/mod.rs (they're re-exported from common).
        for special_type in &["Empty", "Operation"] {
            if all_types.contains(*special_type)
                && !used_shared_types
                    .iter()
                    .any(|(orig, _)| orig == special_type)
            {
                used_shared_types.push((special_type.to_string(), special_type.to_string()));
            }
        }

        // Emit imports for shared types (both direct and transitively referenced)
        if !used_shared_types.is_empty() {
            writeln!(out, "// Import shared types used by this module")?;
            for (original, renamed) in &used_shared_types {
                if original == renamed {
                    writeln!(out, "use super::shared::{renamed:};")?;
                } else {
                    writeln!(out, "use super::shared::{original:} as {renamed:};")?;
                }
            }
            writeln!(out)?;
        }

        // ApiResponse is always used by the generated _request functions
        if !group.endpoints.is_empty() {
            writeln!(out, "use super::shared::ApiResponse;")?;
            writeln!(out)?;
        }

        // Generate type stubs for forward references
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out, "// TYPE DECLARATIONS")?;
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out)?;

        // Everything so far is the module header and imports.
        module.raw(std::mem::take(&mut out));

        // Build dependency graph to detect recursive types (types that reference themselves)
        let type_deps = build_type_dependencies(schemas);
        let recursive_types = find_recursive_types(&type_deps);

        // Generate types from schemas - skip types that are already in shared module
        for type_name in &all_types {
            if generated_types.contains(type_name) {
                continue;
            }
            // Skip types that are defined in shared module
            if shared_resources.contains(type_name) || actual_shared_types.contains(type_name) {
                continue;
            }
            // Skip Empty and Operation — always provided by crate::providers::common via super::shared
            if type_name == "Empty" || type_name == "Operation" {
                continue;
            }

            // Rename types that conflict with std types
            let safe_type_name = rename_std_type_conflict(type_name);

            // Try to find the schema for this type (may be PascalCase while
            // schemas use the original spec key convention)
            let def = match Self::resolve_schema_key(type_name, schemas) {
                Some(schema) => {
                    self.type_def_from_schema(&safe_type_name, schema, schemas, &recursive_types)
                }
                // No schema to say what it is: any JSON value.
                None => TypeDef::alias(
                    &safe_type_name,
                    &format!("`{safe_type_name}` type (no schema in the spec)."),
                    "serde_json::Value",
                ),
            };
            module.push(def);

            generated_types.insert(safe_type_name.clone());
        }

        // Generate Args types per endpoint
        module.raw(
            "// =============================================================================\n\
             // ARGS TYPES (per-endpoint)\n\
             // =============================================================================\n\n"
                .to_string(),
        );

        for ep in &group.endpoints {
            let args_name = format!(
                "{}Args",
                to_pascal_case(&sanitize_identifier(&ep.operation_id))
            );

            // Always emit the Args struct — the generated `*_request` function
            // unconditionally takes `args: &{Op}Args`, so a param-less endpoint
            // still needs the (empty) struct to exist, otherwise its function
            // references an undefined type (`E0425`).
            let mut fields = Vec::new();
            for param in &ep.path_params {
                fields.push(Field {
                    doc: format!("Path parameter: `{param}`."),
                    attrs: Vec::new(),
                    name: escape_rust_keyword(&to_snake_case(&sanitize_identifier(param))),
                    ty: "String".to_string(),
                });
            }
            for param in &ep.query_params {
                let name = escape_rust_keyword(&to_snake_case(&sanitize_identifier(param)));
                // An array is sent as one `name=value` pair per value.
                let (doc, ty) = if ep.array_query_params.contains(param) {
                    (format!("Query parameter: `{param}` (one per value)."), "Vec<String>")
                } else {
                    (format!("Query parameter: `{param}`."), "Option<String>")
                };
                fields.push(Field {
                    doc,
                    attrs: Vec::new(),
                    name,
                    ty: ty.to_string(),
                });
            }
            if let Some(rt) = &ep.request_type {
                fields.push(Field {
                    doc: "Request body.".to_string(),
                    attrs: Vec::new(),
                    name: "body".to_string(),
                    ty: rt.clone(),
                });
            }
            module.push(TypeDef::structure(
                &args_name,
                &format!("Arguments for [`{}_request`].", ep.operation_id),
                ARGS_DERIVES,
                fields,
            ));
        }

        // Generate client functions per endpoint
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out, "// CLIENT FUNCTIONS (per-endpoint)")?;
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out)?;

        for ep in &group.endpoints {
            self.generate_endpoint_client_functions(&mut out, ep, group)?;
        }
        module.raw(out);

        Ok(module)
    }

    /// The type a schema generates: a struct for an object with properties
    /// (or an `allOf`), and an alias for everything else.
    ///
    /// **Why an alias, and why that alias:** a schema without properties is a
    /// string, a number, a list, a free-form map or a union — and its name is
    /// part of the API (fields and endpoints refer to it). It used to become
    /// `struct Name { #[serde(flatten)] data: HashMap<String, Value> }` whatever it
    /// actually was, so `latitude: Option<LoadBalancingLatitude>` could not
    /// deserialize the number Cloudflare sends, and an `AccessMessages` list
    /// could not deserialize a list. Now the name aliases what the schema says:
    /// `String`, `i64`, `f64`, `bool`, `Vec<Item>`, and `serde_json::Value` for a
    /// free-form map or a union (no wrapper type around a map).
    fn type_def_from_schema(
        &self,
        type_name: &str,
        schema: &crate::spec::Schema,
        schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
        recursive_types: &std::collections::HashSet<String>,
    ) -> TypeDef {
        use crate::spec::Schema as SpecSchema;
        use std::collections::BTreeMap;

        let doc = format!("`{type_name}` type.");
        if let Some(target) = self.plain_rust_type(schema, schemas) {
            // `type A = Vec<A>` is an infinitely sized alias; such a schema can
            // only be described as JSON.
            let target = if type_ident_in(&target, type_name) {
                "serde_json::Value".to_string()
            } else {
                target
            };
            return TypeDef::alias(type_name, &doc, &target);
        }

        // WHY: OpenAPI specs use mixed naming conventions (PascalCase, camelCase,
        // snake_case). We convert all property names to snake_case for Rust fields,
        // so we must emit per-field #[serde(rename)] when the original name differs
        // from the snake_cased field name. A single rename_all can't handle mixed
        // specs like Docker (ApiVersion + architecture in the same schema).
        let field = |field_name: &str, prop_name: &str, prop_schema: &SpecSchema, required: bool| {
            let rust_type = self.schema_to_rust_type(prop_schema, schemas);
            let rust_type = maybe_box_type(&rust_type, recursive_types);
            // `required` and `nullable` say different things: the first that
            // the key is always present, the second that its value may be
            // null. A field can be both — Hetzner's pagination `next_page`
            // is always there and is null on the last page. Only a field
            // that is required AND not nullable can be a bare T.
            let is_required = required && !is_nullable(prop_schema, schemas);
            Field {
                doc: format!("`{prop_name}` property."),
                attrs: if prop_name == field_name {
                    Vec::new()
                } else {
                    vec![format!("#[serde(rename = \"{prop_name}\")]")]
                },
                name: field_name.to_string(),
                ty: if is_required {
                    rust_type
                } else {
                    format!("Option<{rust_type}>")
                },
            }
        };

        let mut fields = Vec::new();
        // Handle allOf - merge properties from all members
        if let Some(all_of) = &schema.all_of {
            let mut all_properties: BTreeMap<String, (String, SpecSchema)> = BTreeMap::new();
            let mut required: std::collections::HashSet<String> = std::collections::HashSet::new();

            for member in all_of {
                collect_all_of_member(member, schemas, &mut all_properties, &mut required, 0);
            }

            for (field_name, (prop_name, prop_schema)) in &all_properties {
                fields.push(field(field_name, prop_name, prop_schema, required.contains(prop_name)));
            }
        }
        // Handle object with properties
        else if let Some(properties) = &schema.properties {
            let required: std::collections::HashSet<String> =
                schema.required.iter().cloned().collect();
            let mut generated_fields: HashSet<String> = HashSet::new();

            for (prop_name, prop_schema) in properties {
                let field_name = field_ident(prop_name);
                // Skip if we already generated this field (handles "Version" vs "version" duplicates)
                if !generated_fields.insert(field_name.clone()) {
                    continue;
                }
                fields.push(field(&field_name, prop_name, prop_schema, required.contains(prop_name)));
            }
        }

        TypeDef::structure(type_name, &doc, SCHEMA_DERIVES, fields)
    }

    /// The Rust type for a schema that does not generate a struct, or `None`
    /// for one that does (an object with properties, or an `allOf`).
    ///
    /// A free-form map (`type: object` with no properties, or with an empty
    /// `properties` plus `additionalProperties`), a `oneOf`/`anyOf`, and a schema
    /// with no type are all `serde_json::Value`.
    fn plain_rust_type(
        &self,
        schema: &crate::spec::Schema,
        schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
    ) -> Option<String> {
        if schema.all_of.is_some() {
            return None;
        }
        if let Some(properties) = &schema.properties {
            // `properties: {}` is an empty object (a struct with no fields)
            // unless further keys are allowed, in which case it is a map.
            return (properties.is_empty() && schema.additional_properties == Some(true))
                .then(|| "serde_json::Value".to_string());
        }
        if schema.ref_path.is_some() {
            return Some(self.schema_to_rust_type(schema, schemas));
        }
        Some(match schema.schema_type.as_deref() {
            Some("string" | "integer" | "number" | "boolean" | "array") => {
                self.schema_to_rust_type(schema, schemas)
            }
            _ => "serde_json::Value".to_string(),
        })
    }

    /// Convert an OpenAPI schema to a Rust type string.
    fn schema_to_rust_type(
        &self,
        schema: &crate::spec::Schema,
        schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
    ) -> String {
        // Check for $ref first
        if let Some(ref_path) = &schema.ref_path {
            let ref_name = ref_path
                .trim_start_matches("#/components/schemas/")
                .trim_start_matches("#/schemas/");

            // If the referenced schema is an array, resolve to Vec<…> inline
            // instead of returning a PascalCase wrapper name.  Array schemas
            // should never be generated as standalone structs.
            if let Some(target) = schemas.get(ref_name) {
                if target.schema_type.as_deref() == Some("array") {
                    return self.schema_to_rust_type(target, schemas);
                }
                // A named string/number/boolean, free-form map or union is just
                // that type: use it directly, so a field reads `Option<f64>`
                // rather than an alias name, and structs made of such fields
                // compare equal across modules (see `emit`). A `$ref` to a `$ref`
                // keeps the name: following chains could loop.
                if target.ref_path.is_none() {
                    if let Some(plain) = self.plain_rust_type(target, schemas) {
                        return plain;
                    }
                }
            }

            // Rename std type conflicts
            let pascal_name = crate::to_pascal_case(ref_name);
            return rename_std_type_conflict(&pascal_name);
        }

        // Check schema type
        match schema.schema_type.as_deref() {
            Some("string") => "String".to_string(),
            Some("integer") => "i64".to_string(),
            Some("number") => "f64".to_string(),
            Some("boolean") => "bool".to_string(),
            Some("array") => {
                if let Some(items) = &schema.items {
                    let item_type = self.schema_to_rust_type(items, schemas);
                    format!("Vec<{}>", item_type)
                } else {
                    "Vec<serde_json::Value>".to_string()
                }
            }
            Some("object") => {
                // Check if it's an inline object with properties
                if schema.properties.as_ref().is_some_and(|p| !p.is_empty()) {
                    // Inline objects use HashMap
                    "std::collections::HashMap<String, serde_json::Value>".to_string()
                } else {
                    // Empty object or unknown object
                    "serde_json::Value".to_string()
                }
            }
            _ => "serde_json::Value".to_string(),
        }
    }

    /// Generate client functions for a single endpoint.
    fn generate_endpoint_client_functions(
        &self,
        out: &mut String,
        ep: &EndpointInfo,
        _group: &ApiGroup,
    ) -> Result<(), GenError> {
        let fn_prefix = to_snake_case(&sanitize_identifier(&ep.operation_id));
        // A streamed body is handed back unread — as a stream of items or of
        // bytes — never collected: it can be unbounded (a tar archive, a
        // followed log) or be how the server reports progress on work it does
        // while it writes (a JSON-lines pull or build).
        let streaming = ep.response_type.as_ref().filter(|rt| rt.is_streaming());
        let return_type = match ep.response_type.as_ref() {
            Some(ResponseType::JsonLines(item)) => format!("super::shared::JsonLinesStream<{item}>"),
            Some(ResponseType::Binary) => "super::shared::ByteStream".to_string(),
            Some(rt) => rt.as_rust_type().to_string(),
            None => "()".to_string(),
        };

        let args_name = format!(
            "{}Args",
            to_pascal_case(&sanitize_identifier(&ep.operation_id))
        );

        // Header comment for this endpoint
        writeln!(
            out,
            "// -----------------------------------------------------------------------------"
        )?;
        writeln!(out, "// {} {}", ep.method, ep.path)?;
        writeln!(
            out,
            "// -----------------------------------------------------------------------------"
        )?;
        writeln!(out)?;

        // Single merged async fn: builds request, applies optional modifications, sends.
        writeln!(out, "/// {} {}.", ep.method, ep.path)?;
        writeln!(out, "///")?;
        writeln!(
            out,
            "/// Takes a client and args, builds the request, optionally applies"
        )?;
        writeln!(out, "/// modifications, then sends it via `send_async().await`.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Arguments")?;
        writeln!(out, "///")?;
        writeln!(
            out,
            "/// * `client` - HTTP client for making the request: a `DynNetClient`, or an `ApiClient` carrying the `BodyLimits` the response is read under"
        )?;
        writeln!(
            out,
            "/// * `args` - Request arguments (path params, query params, body)"
        )?;
        if let Some(raw) = &ep.raw_request_body {
            writeln!(
                out,
                "/// * `body` - Request body, sent as-is as `{}`{}",
                raw.media_type,
                if raw.required { "" } else { " (`None` sends none)" }
            )?;
        }
        writeln!(out, "/// * `builder_mod` - Optional closure to modify the request builder (e.g., add headers)")?;
        if let Some(rt) = streaming {
            writeln!(out, "///")?;
            writeln!(out, "/// # Response body")?;
            writeln!(out, "///")?;
            match rt {
                ResponseType::JsonLines(item) => writeln!(
                    out,
                    "/// Returned unread, as a stream of `{item}`, one per line: read it with `.next().await`."
                )?,
                _ => writeln!(
                    out,
                    "/// Returned unread, as a stream of byte chunks: read it with `.next().await`."
                )?,
            }
            writeln!(out, "/// The status is checked before the body is touched.")?;
        }
        writeln!(out, "///")?;
        writeln!(out, "/// # Example")?;
        writeln!(out, "///")?;
        writeln!(out, "/// ```ignore")?;
        writeln!(
            out,
            "/// let response = {}_request(client.clone(), &args, Some(|b: &mut PreparedRequestBuilder| {{",
            fn_prefix
        )?;
        writeln!(out, "///     b.header(\"X-Custom-Header\", \"value\");")?;
        writeln!(out, "/// }})).await?;")?;
        writeln!(out, "/// ```")?;
        // `args` is only read when the endpoint has path params, query params,
        // or a request body. Name it `_args` otherwise so the (always-emitted)
        // parameter doesn't trip `unused_variables`.
        let args_uses = !ep.path_params.is_empty()
            || !ep.query_params.is_empty()
            || ep.request_type.is_some();
        let args_binding = if args_uses { "args" } else { "_args" };
        writeln!(out, "pub async fn {}_request<F>(", fn_prefix)?;
        // `impl Into<ApiClient>`: a bare `DynNetClient` still works (with the
        // default `BodyLimits`), and a client that configures its own limits
        // passes an `ApiClient` carrying them down to the shared senders.
        writeln!(out, "    client: impl Into<super::shared::ApiClient>,")?;
        writeln!(out, "    {}: &{},", args_binding, args_name)?;
        // WHY: The base URL (e.g. "http://localhost/v1.53") is configurable so
        // callers can point at remote Docker daemons, not just Unix-socket-local.
        // For providers whose spec declares an explicit baseUrl, the caller
        // should pass that value; for Docker the caller derives it from
        // DockerClient::base_url().
        writeln!(out, "    base_url: &str,")?;
        if let Some(raw) = &ep.raw_request_body {
            if raw.required {
                writeln!(out, "    body: foundation_netio::shared::http::SendSafeBody,")?;
            } else {
                writeln!(out, "    body: Option<foundation_netio::shared::http::SendSafeBody>,")?;
            }
        }
        writeln!(out, "    builder_mod: Option<F>,")?;
        writeln!(out, ") -> Result<ApiResponse<{}>, super::shared::ApiError>", return_type)?;
        writeln!(out, "where")?;
        writeln!(out, "    F: FnOnce(&mut PreparedRequestBuilder),")?;
        writeln!(out, "{{")?;

        // Build URL: base_url is the caller-supplied scheme+host+version prefix
        // (e.g. "http://localhost/v1.53"); we format the path separately then
        // join — avoids nested format!() indentation issues.
        let (escaped_path, params_in_url_order) = escape_url_for_format(&ep.path, &ep.path_params);
        writeln!(out, "    let path = format!(\"{}\",", escaped_path)?;
        for param_name in &params_in_url_order {
            let safe_param = escape_rust_keyword(param_name);
            writeln!(out, "        args.{safe_param},")?;
        }
        writeln!(out, "    );")?;
        writeln!(out, "    let endpoint_url = format!(\"{{}}{{}}\", base_url, path);")?;
        writeln!(out)?;

        // Build request via the cross-platform PreparedRequestBuilder.
        let method_lower = ep.method.to_lowercase();
        writeln!(
            out,
            "    let mut builder = PreparedRequestBuilder::{}(&endpoint_url)",
            method_lower
        )?;
        writeln!(
            out,
            "        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;"
        )?;
        writeln!(out)?;

        // Structured query params from args (None values are skipped).
        for qp in &ep.query_params {
            let safe_qp = escape_rust_keyword(&to_snake_case(&sanitize_identifier(qp)));
            if ep.array_query_params.contains(qp) {
                writeln!(out, "    for value in &args.{safe_qp} {{")?;
                writeln!(out, "        builder = builder.query(\"{qp}\", Some(value.as_str()));")?;
                writeln!(out, "    }}")?;
            } else {
                writeln!(
                    out,
                    "    builder = builder.query(\"{qp}\", args.{safe_qp}.as_deref());"
                )?;
            }
        }
        if !ep.query_params.is_empty() {
            writeln!(out)?;
        }

        // Add body if present.
        if ep.request_type.is_some() {
            writeln!(out, "    builder = builder.body_json(&args.body)")?;
            writeln!(out, "        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;")?;
            writeln!(out)?;
        }

        // A non-JSON body (tar archive, raw bytes) goes on as-is.
        if let Some(raw) = &ep.raw_request_body {
            if raw.required {
                writeln!(
                    out,
                    "    builder = super::shared::with_raw_body(builder, \"{}\", body);",
                    raw.media_type
                )?;
            } else {
                writeln!(out, "    if let Some(body) = body {{")?;
                writeln!(
                    out,
                    "        builder = super::shared::with_raw_body(builder, \"{}\", body);",
                    raw.media_type
                )?;
                writeln!(out, "    }}")?;
            }
            writeln!(out)?;
        }

        // Apply user modifications.
        writeln!(out, "    if let Some(f) = builder_mod {{")?;
        writeln!(out, "        f(&mut builder);")?;
        writeln!(out, "    }}")?;
        writeln!(out)?;

        // Send, check the status, parse — in shared generic functions
        // (`common::send_json` and friends), not a copy per endpoint:
        // thousands of copies of the same async body were much of what a
        // large provider crate's `rustc` spent its memory on. A non-2xx status
        // becomes `ApiError::HttpStatus` with the vendor's error body, read up
        // to the client's `BodyLimits::max_error_body` (see
        // `common::send_checked`); a JSON-lines body's lines are capped at its
        // `max_json_line`.
        let sender = match streaming {
            Some(ResponseType::JsonLines(_)) => "send_json_lines",
            Some(_) => "send_bytes",
            None if return_type == "()" => "send_without_body",
            None => "send_json",
        };
        writeln!(out, "    super::shared::{sender}(client.into(), builder.build()).await")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    /// Resolve a PascalCase type name to the original OpenAPI spec schema key.
    ///
    /// The `schemas` map uses the spec's original keys (typically `snake_case`),
    /// but `shared_resources` stores `PascalCase` names. Try direct lookup first
    /// (some specs use PascalCase keys), then convert to snake_case.
    fn resolve_schema_key<'a>(
        type_name: &str,
        schemas: &'a std::collections::BTreeMap<String, crate::spec::Schema>,
    ) -> Option<&'a crate::spec::Schema> {
        // 1. Direct lookup — the spec's key already matches the type name.
        if let Some(s) = schemas.get(type_name) {
            return Some(s);
        }
        // 2. Convert to snake_case (Cloudflare's spec uses snake_case keys).
        let snake = crate::to_snake_case(type_name);
        if let Some(s) = schemas.get(&snake) {
            return Some(s);
        }
        // 3. Lowercase the first char (e.g. "FooBar" → "fooBar").
        if let Some(first) = type_name.chars().next() {
            let lower_first = first.to_lowercase().collect::<String>() + &type_name[first.len_utf8()..];
            if let Some(s) = schemas.get(&lower_first) {
                return Some(s);
            }
        }
        // 4. Match on the *pascal-cased* key.
        //
        // The three guesses above try to invert pascal-casing, and pascal-casing
        // is not invertible: Hetzner keys a schema `CreateServerResponseServerPublic_net`
        // (its property is `public_net`), which the generator refers to as
        // `…PublicNet`. No amount of snake-casing that name gets back to the
        // original, so the lookup missed and the type fell back to an opaque
        // `HashMap<String, Value>` blob — 52 of Hetzner's 100 types, including
        // `public_net`, which is where the server's IP address lives.
        //
        // Comparing in the pascal-cased space is the direction that *is* well
        // defined: it is exactly the transform the reference site applies.
        let target = crate::to_pascal_case(type_name);
        schemas
            .iter()
            .find(|(key, _)| crate::to_pascal_case(key) == target)
            .map(|(_, schema)| schema)
    }

    /// Generate shared module for cross-group types.
    ///
    /// WHY: Types used by multiple groups (shared_resources) must live in one place so
    /// group modules can import them via `super::shared::`. The OLD behaviour was to emit
    /// a `HashMap<String, Value>` stub for every shared type regardless of the actual
    /// OpenAPI schema. The NEW behaviour resolves each type's schema and generates a
    /// properly typed struct with real fields, only falling back to HashMap when the
    /// schema truly cannot be resolved.
    fn generate_shared_module(
        &self,
        analysis: &super::analyzer::AnalysisResult,
    ) -> Result<ModuleFile, GenError> {
        let mut module = ModuleFile::new("shared/mod.rs");

        // Build dependency graph across ALL schemas (including shared ones) so we
        // detect mutually recursive types that need Box wrapping.
        let type_deps = build_type_dependencies(&analysis.schemas);
        let recursive_types = find_recursive_types(&type_deps);

        let mut out = String::new();
        writeln!(
            out,
            "//! Shared types for {} (used by multiple groups).",
            analysis.provider
        )?;
        writeln!(out, "//!")?;
        writeln!(
            out,
            "//! Generated by `cargo run --bin ewe_platform gen_api`."
        )?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out)?;
        // For split-out providers (provider_dir_override set), the shared
        // module lives in its own crate so it must import from
        // foundation_deployment rather than crate::providers::common.
        let common_path = if self.provider_dir_override.is_some() {
            "foundation_deployment::providers::common"
        } else {
            "crate::providers::common"
        };
        writeln!(
            out,
            "// Re-export common API types from foundation_deployment"
        )?;
        writeln!(out, "pub use {common_path}::{{ApiError, ApiPending, ApiResponse, BoxedSendExecutionAction, Empty, Operation}};")?;
        // Sending, streaming bodies and the bounded error-body read the
        // `*_request` functions use (see `generate_endpoint_client_functions`).
        writeln!(out, "pub use {common_path}::{{read_error_body, send_bytes, send_checked, send_json, send_json_lines, send_without_body, with_raw_body, ApiClient, BodyLimits, ByteStream, JsonLinesStream}};")?;
        writeln!(out)?;
        writeln!(out, "// Imports for shared resource types")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out, "use serde::{{Deserialize, Serialize}};")?;
        writeln!(out)?;
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out, "// SHARED RESOURCE TYPES")?;
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out)?;
        module.raw(std::mem::take(&mut out));

        // Transitively expand the shared resource list so that any type referenced
        // by a shared struct's fields (nested objects, array element types, etc.)
        // is *also* emitted here. Without this a shared struct can reference a type
        // that lives only in some group module — an `E0425 cannot find type` in the
        // shared module itself. The convergence mirrors the per-group collection.
        let shared_types_to_emit: Vec<String> = {
            // BTreeSet to match `collect_referenced_type_names`, which must not
            // hand back a randomly-ordered set — see `all_types` below. Here it is
            // belt-and-braces: `extras` is sorted before it is appended anyway.
            let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            let mut to_process: Vec<String> = analysis.shared_resources.clone();
            for t in &analysis.shared_resources {
                seen.insert(t.clone());
            }
            let mut traversed: std::collections::HashSet<String> = std::collections::HashSet::new();
            while let Some(type_name) = to_process.pop() {
                let key = type_name.to_lowercase();
                if traversed.contains(&key) {
                    continue;
                }
                traversed.insert(key);
                if let Some(schema) = Self::resolve_schema_key(&type_name, &analysis.schemas) {
                    let mut discovered: Vec<String> = Vec::new();
                    collect_referenced_type_names(schema, &mut seen, &mut discovered);
                    to_process.extend(discovered);
                }
            }
            // Preserve the original shared_resources ordering first, then append the
            // transitively discovered types (which are stored PascalCase in `seen`).
            let mut ordered: Vec<String> = analysis.shared_resources.clone();
            let original: std::collections::HashSet<String> =
                analysis.shared_resources.iter().cloned().collect();
            let mut extras: Vec<String> = seen
                .into_iter()
                .filter(|t| !original.contains(t))
                .collect();
            extras.sort();
            ordered.extend(extras);
            ordered
        };

        let mut emitted_shared: std::collections::HashSet<String> = std::collections::HashSet::new();
        for type_name in &shared_types_to_emit {
            // Skip types that aren't valid identifiers (generics like Vec<...>, paths with ::, etc.)
            if type_name.contains('<') || type_name.contains('>') || type_name.contains("::") {
                continue;
            }
            // Skip types starting with non-alphabetic characters (except _)
            if !type_name
                .chars()
                .next()
                .map(|c| c.is_alphabetic() || c == '_')
                .unwrap_or(false)
            {
                continue;
            }
            // Skip array-like types (Vec, Option, etc. used as raw type names without generics)
            if [
                "Vec", "Option", "HashMap", "BTreeMap", "Box", "Rc", "Arc", "Cow",
            ]
            .contains(&type_name.as_str())
            {
                continue;
            }
            // Skip Empty and Operation — provided by crate::providers::common
            if type_name == "Empty" || type_name == "Operation" {
                continue;
            }

            // Rename types that conflict with std types
            let safe_name = rename_std_type_conflict(type_name);

            // Deduplicate — a type may be reached both directly and transitively.
            if !emitted_shared.insert(safe_name.clone()) {
                continue;
            }

            // Array schemas inline to `Vec<Item>` at field positions, so they
            // must not be emitted as standalone structs (the element type is
            // emitted instead, having been collected transitively).
            if let Some(schema) = Self::resolve_schema_key(type_name, &analysis.schemas) {
                if schema.schema_type.as_deref() == Some("array") {
                    continue;
                }
            }

            // Try to resolve the schema for this shared type.
            // Schemas are keyed by original spec names (usually snake_case);
            // shared_resources stores PascalCase names. resolve_schema_key
            // tries multiple naming conventions.
            let def = match Self::resolve_schema_key(type_name, &analysis.schemas) {
                Some(schema) => self.type_def_from_schema(
                    &safe_name,
                    schema,
                    &analysis.schemas,
                    &recursive_types,
                ),
                // No schema to say what it is: any JSON value.
                None => TypeDef::alias(
                    &safe_name,
                    &format!("Shared type `{safe_name}` (no schema in the spec)."),
                    "serde_json::Value",
                ),
            };
            module.push(def);
        }

        Ok(module)
    }

    /// Generate provider mod.rs with feature guards.
    /// Generate `mod.rs` inside `generated/` that declares all sub-modules.
    fn generate_provider_mod(
        &self,
        provider: &str,
        groups: &[ApiGroup],
        _shared_resources: &[String],
        generated_dir: &Path,
    ) -> Result<(), GenError> {
        let feature_name = provider.replace('-', "_").replace('/', "_");

        let mut out = String::new();
        writeln!(out, "//! Auto-generated module for {provider}.",)?;
        writeln!(out, "//!")?;
        writeln!(
            out,
            "//! Generated by `cargo run --bin genapi generate`."
        )?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{feature_name}\")]")?;
        writeln!(
            out,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity)]"
        )?;
        writeln!(
            out,
            "#![allow(clippy::missing_errors_doc, clippy::doc_markdown, clippy::useless_format)]"
        )?;
        writeln!(out, "#![allow(unused_imports)]")?;
        writeln!(out)?;

        // Shared module - always generated (contains ApiError, ApiResponse, etc.)
        writeln!(out, "pub mod shared;")?;
        writeln!(out)?;

        // Group modules (use sanitized names, deduplicated)
        let mut seen_modules = std::collections::HashSet::new();
        for group in groups {
            let safe_name = sanitize_group_name(&group.name);
            let safe_name = sanitize_module_name(&safe_name);
            if !seen_modules.insert(safe_name.clone()) {
                continue; // duplicate module name — skip
            }
            writeln!(out, "#[cfg(feature = \"{feature_name:}_{safe_name:}\")]")?;
            writeln!(out, "pub mod {safe_name:};")?;
        }

        fs::create_dir_all(generated_dir)?;
        fs::write(generated_dir.join("mod.rs"), out)?;

        Ok(())
    }
}
