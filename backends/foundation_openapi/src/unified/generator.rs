//! Unified code generator for OpenAPI specs.
//!
//! WHY: Generates cohesive per-endpoint units (types + clients + provider impl).
//!
//! WHAT: For each endpoint in a group, generates all related code together.
//!
//! HOW: String templates for each endpoint unit, grouped into modules.

use crate::{sanitize_identifier, to_pascal_case, to_snake_case, EndpointInfo};
use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;

use super::analyzer::ApiGroup;

/// Scan a shared module's content and extract all struct/enum type names it defines.
fn regex_shared_type_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // Match "pub struct TypeName" or "pub enum TypeName"
        if let Some(rest) = trimmed
            .strip_prefix("pub struct ")
            .or_else(|| trimmed.strip_prefix("pub enum "))
        {
            if let Some(name) = rest.split_whitespace().next() {
                let clean = name.trim_end_matches('{').trim_end();
                if !clean.is_empty()
                    && clean
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase() || c == '_')
                        .unwrap_or(false)
                {
                    names.push(clean.to_string());
                }
            }
        }
        // Also match "pub use crate::providers::common::{..., Empty, Operation, ...};"
        if trimmed.starts_with("pub use crate::providers::common::") {
            if let Some(start) = trimmed.find('{') {
                if let Some(end) = trimmed.find('}') {
                    let between = &trimmed[start + 1..end];
                    for item in between.split(',') {
                        let item = item.trim();
                        if !item.is_empty() && !item.starts_with('_') {
                            names.push(item.to_string());
                        }
                    }
                }
            }
        }
    }
    names
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
    all_types: &mut std::collections::HashSet<String>,
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
    seen_types: &mut std::collections::HashSet<String>,
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

/// Update Cargo.toml with missing feature flags for the provider.
fn update_cargo_toml(
    provider: &str,
    groups: &[ApiGroup],
    cargo_toml_path: &Path,
) -> Result<(), std::io::Error> {
    let content = fs::read_to_string(cargo_toml_path)?;

    // Parse TOML into a mutable Value
    let mut doc: Value = toml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let provider_feature = provider.replace('-', "_").replace('/', "_");

    // Get or create features table
    let features = doc
        .get_mut("features")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "missing [features] section",
            )
        })?;

    // Check if this is a hierarchical provider (e.g., gcp/admin -> gcp_admin under gcp)
    // We detect this by checking if the provider name contains underscores that indicate nesting
    // e.g., "gcp_admin" has two segments, "cloudflare" has one
    let provider_parts: Vec<&str> = provider_feature.split('_').collect();
    let parent_feature = if provider_parts.len() > 1 {
        // This is a sub-provider like gcp_admin, parent is gcp
        Some(provider_parts[0].to_string())
    } else {
        None
    };

    // Remove all existing feature flags for this specific provider (not parent)
    let keys_to_remove: Vec<String> = features
        .keys()
        .filter(|k| k.starts_with(&format!("{}_", provider_feature)))
        .cloned()
        .collect();
    for key in keys_to_remove {
        features.remove(&key);
    }

    // Collect group feature names
    let mut group_features: Vec<String> = Vec::new();

    // Add group-level features
    for group in groups {
        let safe_name = sanitize_group_name(&group.name);
        let safe_name = sanitize_module_name(&safe_name);
        let feature_name = format!("{}_{}", provider_feature, safe_name);
        features.insert(feature_name.clone(), Value::Array(vec![]));
        group_features.push(feature_name);
    }

    // Update provider-level feature to enable all group features
    // e.g., gcp_admin = ["gcp_admin_applications", "gcp_admin_dates", ...]
    let provider_feature_array: Value = Value::Array(
        group_features
            .iter()
            .map(|f| Value::String(f.clone()))
            .collect(),
    );
    features.insert(provider_feature.clone(), provider_feature_array);

    // If this is a sub-provider (like gcp_admin), update the parent feature
    // to include ALL sub-providers under it (not just this one)
    if let Some(parent) = parent_feature {
        // Collect all sub-provider features (e.g., gcp_admin, gcp_cloudkms, etc.)
        let mut sub_providers: Vec<String> = Vec::new();
        for key in features.keys() {
            // Match keys like "gcp_admin", "gcp_cloudkms" but NOT "gcp_admin_applications"
            if key.starts_with(&format!("{}_", parent)) {
                let parts: Vec<&str> = key.split('_').collect();
                if parts.len() == 2 {
                    // This is a sub-provider (exactly 2 parts: parent_subprovider)
                    sub_providers.push(key.clone());
                }
            }
        }

        // Update parent feature to include all sub-providers
        let parent_feature_array: Value = Value::Array(
            sub_providers
                .iter()
                .map(|f| Value::String(f.clone()))
                .collect(),
        );
        features.insert(parent.clone(), parent_feature_array);
    }

    // Note: shared module doesn't need a feature flag - it's always compiled when provider is enabled

    // Serialize back to TOML with nice formatting
    let output = toml::to_string_pretty(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    fs::write(cargo_toml_path, output + "\n")?;
    Ok(())
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
        self.generate_shared_module(&analysis, &generated_dir)?;

        // Generate one module per group
        for group in &analysis.groups {
            self.generate_group_module(
                provider,
                group,
                &analysis.shared_resources,
                &analysis.schemas,
                &generated_dir,
            )?;
        }

        // Generate generated/mod.rs with feature guards — this becomes the
        // single entry point for all generated code.
        self.generate_provider_mod(provider, &analysis.groups, &analysis.shared_resources, &generated_dir)?;

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
        output_dir: &Path,
    ) -> Result<(), GenError> {
        // Sanitize group name for use as directory/file name
        let safe_name = sanitize_group_name(&group.name);
        let safe_name = sanitize_module_name(&safe_name);
        let group_dir = output_dir.join(&safe_name);
        fs::create_dir_all(&group_dir)?;

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
        let mut all_types: std::collections::HashSet<String> = HashSet::new();

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

        // Scan the actual shared module file to get all types it defines.
        // This is more reliable than relying on the analyzer's shared_resources list.
        let shared_mod_path = output_dir.join("shared").join("mod.rs");
        let mut actual_shared_types: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        if let Ok(shared_content) = std::fs::read_to_string(&shared_mod_path) {
            // Match "pub struct TypeName" and "pub enum TypeName" patterns
            for cap in regex_shared_type_names(&shared_content) {
                actual_shared_types.insert(cap);
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
            if let Some(schema) = Self::resolve_schema_key(type_name, schemas) {
                // Generate proper struct from schema, wrapping recursive refs in Box<>
                self.generate_type_from_schema(
                    &mut out,
                    &safe_type_name,
                    schema,
                    schemas,
                    &recursive_types,
                )?;
            } else {
                // No schema found - generate placeholder struct
                writeln!(out, "/// `{}` response type.", safe_type_name)?;
                writeln!(
                    out,
                    "#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonHash)]"
                )?;
                writeln!(out, "pub struct {} {{", safe_type_name)?;
                writeln!(
                    out,
                    "    /// Raw JSON value - full schema generated from `OpenAPI`"
                )?;
                writeln!(out, "    #[serde(flatten)]")?;
                writeln!(
                    out,
                    "    pub data: std::collections::HashMap<String, serde_json::Value>,"
                )?;
                writeln!(out, "}}")?;
                writeln!(out)?;
            }

            generated_types.insert(safe_type_name.clone());
        }

        // Generate Args types per endpoint
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out, "// ARGS TYPES (per-endpoint)")?;
        writeln!(
            out,
            "// ============================================================================="
        )?;
        writeln!(out)?;

        for ep in &group.endpoints {
            let args_name = format!(
                "{}Args",
                to_pascal_case(&sanitize_identifier(&ep.operation_id))
            );

            // Always emit the Args struct — the generated `*_request` function
            // unconditionally takes `args: &{Op}Args`, so a param-less endpoint
            // still needs the (empty) struct to exist, otherwise its function
            // references an undefined type (`E0425`).
            writeln!(out, "/// Arguments for [`{}_request`].", ep.operation_id)?;
            writeln!(out, "#[derive(Debug, Clone, Default, Serialize, JsonHash)]")?;
            writeln!(out, "pub struct {} {{", args_name)?;

            for param in &ep.path_params {
                let param_name =
                    escape_rust_keyword(&to_snake_case(&sanitize_identifier(param)));
                writeln!(out, "    /// Path parameter: `{}`.", param)?;
                writeln!(out, "    pub {}: String,", param_name)?;
            }
            for param in &ep.query_params {
                let param_name =
                    escape_rust_keyword(&to_snake_case(&sanitize_identifier(param)));
                writeln!(out, "    /// Query parameter: `{}`.", param)?;
                writeln!(out, "    pub {}: Option<String>,", param_name)?;
            }
            if let Some(rt) = &ep.request_type {
                writeln!(out, "    /// Request body.")?;
                writeln!(out, "    pub body: {},", rt)?;
            }

            writeln!(out, "}}")?;
            writeln!(out)?;
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

        // Write output
        let output_path = group_dir.join("mod.rs");
        fs::write(&output_path, out)?;

        // Format
        let _ = std::process::Command::new("rustfmt")
            .arg(&output_path)
            .output();

        Ok(())
    }

    /// Generate a type definition from an OpenAPI schema.
    fn generate_type_from_schema(
        &self,
        out: &mut String,
        type_name: &str,
        schema: &crate::spec::Schema,
        schemas: &std::collections::BTreeMap<String, crate::spec::Schema>,
        recursive_types: &std::collections::HashSet<String>,
    ) -> Result<(), GenError> {
        use crate::spec::Schema as SpecSchema;
        use std::collections::BTreeMap;

        writeln!(out, "/// `{}` type.", type_name)?;
        writeln!(
            out,
            "#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonHash)]"
        )?;
        // WHY: OpenAPI specs use mixed naming conventions (PascalCase, camelCase,
        // snake_case). We convert all property names to snake_case for Rust fields,
        // so we must emit per-field #[serde(rename)] when the original name differs
        // from the snake_cased field name. A single rename_all can't handle mixed
        // specs like Docker (ApiVersion + architecture in the same schema).
        writeln!(out, "pub struct {} {{", type_name)?;

        // Handle allOf - merge properties from all members
        if let Some(all_of) = &schema.all_of {
            let mut all_properties: BTreeMap<String, (String, SpecSchema)> = BTreeMap::new();
            let mut required: std::collections::HashSet<String> = std::collections::HashSet::new();

            for member in all_of {
                if let Some(props) = &member.properties {
                    for (k, v) in props {
                        // Deduplicate by snake_case field name to avoid duplicates like "Version" and "version"
                        let field_name = field_ident(k);
                        all_properties.insert(field_name, (k.clone(), v.clone()));
                    }
                }
                for r in &member.required {
                    required.insert(r.clone());
                }
            }

            // Generate fields from merged properties
            for (field_name, (prop_name, prop_schema)) in &all_properties {
                let rust_type = self.schema_to_rust_type(prop_schema, schemas);
                let rust_type = maybe_box_type(&rust_type, recursive_types);
                let is_required = required.contains(prop_name);

                writeln!(out, "    /// `{}` property.", prop_name)?;
                // Emit per-field serde rename when the snake_cased field name
                // differs from the original property name (handles PascalCase,
                // camelCase, and mixed-casing specs like Docker).
                if prop_name.as_str() != field_name.as_str() {
                    writeln!(out, "    #[serde(rename = \"{}\")]", prop_name)?;
                }
                if is_required {
                    writeln!(out, "    pub {}: {},", field_name, rust_type)?;
                } else {
                    writeln!(out, "    pub {}: Option<{}>,", field_name, rust_type)?;
                }
            }
        }
        // Handle object with properties
        else if let Some(properties) = &schema.properties {
            let required: std::collections::HashSet<String> =
                schema.required.iter().cloned().collect();
            let mut generated_fields: BTreeMap<String, (String, &SpecSchema)> = BTreeMap::new();

            for (prop_name, prop_schema) in properties {
                let field_name = field_ident(prop_name);
                // Skip if we already generated this field (handles "Version" vs "version" duplicates)
                if generated_fields.contains_key(&field_name) {
                    continue;
                }
                generated_fields.insert(field_name.clone(), (prop_name.clone(), prop_schema));

                let rust_type = self.schema_to_rust_type(prop_schema, schemas);
                let rust_type = maybe_box_type(&rust_type, recursive_types);
                let is_required = required.contains(prop_name);

                writeln!(out, "    /// {} property.", prop_name)?;
                // Emit per-field serde rename when the snake_cased field name
                // differs from the original property name.
                if prop_name != &field_name {
                    writeln!(out, "    #[serde(rename = \"{}\")]", prop_name)?;
                }
                if is_required {
                    writeln!(out, "    pub {}: {},", field_name, rust_type)?;
                } else {
                    writeln!(out, "    pub {}: Option<{}>,", field_name, rust_type)?;
                }
            }
        }

        // If no properties were generated, the schema is opaque — emit a
        // placeholder. (Shared resource types with real schemas are now
        // resolved in generate_shared_module; this fallback only fires for
        // truly unresolvable cases.)
        if schema.properties.is_none() && schema.all_of.is_none() {
            writeln!(out, "    #[serde(flatten)]")?;
            writeln!(
                out,
                "    pub data: std::collections::HashMap<String, serde_json::Value>,"
            )?;
        }

        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
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
        let return_type = ep
            .response_type
            .as_ref()
            .map(|rt| rt.as_rust_type().to_string())
            .unwrap_or_else(|| "()".to_string());

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
            "/// Takes a `DynNetClient` and args, builds the request, optionally applies"
        )?;
        writeln!(out, "/// modifications, then sends it via `send_async().await`.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Arguments")?;
        writeln!(out, "///")?;
        writeln!(out, "/// * `client` - HTTP client for making the request")?;
        writeln!(
            out,
            "/// * `args` - Request arguments (path params, query params, body)"
        )?;
        writeln!(out, "/// * `builder_mod` - Optional closure to modify the request builder (e.g., add headers)")?;
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
        writeln!(out, "    client: DynNetClient,")?;
        writeln!(out, "    {}: &{},", args_binding, args_name)?;
        // WHY: The base URL (e.g. "http://localhost/v1.53") is configurable so
        // callers can point at remote Docker daemons, not just Unix-socket-local.
        // For providers whose spec declares an explicit baseUrl, the caller
        // should pass that value; for Docker the caller derives it from
        // DockerClient::base_url().
        writeln!(out, "    base_url: &str,")?;
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
            writeln!(
                out,
                "    builder = builder.query(\"{qp}\", args.{safe_qp}.as_deref());"
            )?;
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

        // Apply user modifications.
        writeln!(out, "    if let Some(f) = builder_mod {{")?;
        writeln!(out, "        f(&mut builder);")?;
        writeln!(out, "    }}")?;
        writeln!(out)?;

        // Send asynchronously and parse the response.
        writeln!(out, "    let response = client.send_async(builder.build()).await")?;
        writeln!(out, "        .map_err(|e| super::shared::ApiError::RequestSendFailed(e.to_string()))?;")?;
        writeln!(out)?;
        writeln!(out, "    let status: usize = response.get_status().into();")?;
        writeln!(out, "    let headers = response.get_headers_ref().clone();")?;
        writeln!(out, "    if status < 200 || status >= 300 {{")?;
        writeln!(out, "        return Err(super::shared::ApiError::HttpStatus {{ code: status as u16, headers, body: None }});")?;
        writeln!(out, "    }}")?;
        if return_type == "()" {
            writeln!(out, "    Ok(ApiResponse {{ status: status as u16, headers, body: () }})")?;
        } else {
            writeln!(out, "    let body_bytes = foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe(response.take_body());")?;
            writeln!(out, "    let parsed: {} = serde_json::from_slice(&body_bytes).map_err(|e: serde_json::Error| super::shared::ApiError::ParseFailed(e.to_string()))?;", return_type)?;
            writeln!(out, "    Ok(ApiResponse {{ status: status as u16, headers, body: parsed }})")?;
        }
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
        output_dir: &Path,
    ) -> Result<(), GenError> {
        let shared_dir = output_dir.join("shared");
        fs::create_dir_all(&shared_dir)?;

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

        // Transitively expand the shared resource list so that any type referenced
        // by a shared struct's fields (nested objects, array element types, etc.)
        // is *also* emitted here. Without this a shared struct can reference a type
        // that lives only in some group module — an `E0425 cannot find type` in the
        // shared module itself. The convergence mirrors the per-group collection.
        let shared_types_to_emit: Vec<String> = {
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
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
            if let Some(schema) = Self::resolve_schema_key(type_name, &analysis.schemas) {
                // Generate a properly typed struct from the OpenAPI schema.
                self.generate_type_from_schema(
                    &mut out,
                    &safe_name,
                    schema,
                    &analysis.schemas,
                    &recursive_types,
                )?;
            } else {
                // No schema found — emit a placeholder HashMap wrapper so the
                // generated code still compiles (callers should investigate
                // why the schema wasn't resolved).
                writeln!(out, "/// Shared type: `{}`.", safe_name)?;
                writeln!(
                    out,
                    "#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonHash)]"
                )?;
                writeln!(out, "pub struct {} {{", safe_name)?;
                writeln!(out, "    #[serde(flatten)]")?;
                writeln!(
                    out,
                    "    pub data: std::collections::HashMap<String, serde_json::Value>,"
                )?;
                writeln!(out, "}}")?;
                writeln!(out)?;
            }
        }

        fs::write(shared_dir.join("mod.rs"), out)?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::{field_ident, UnifiedGenerator};

    /// `field_ident` is private, so this lives here rather than in `tests/`.
    ///
    /// Every input below is a real property key from a vendor's spec, and each one
    /// used to be emitted verbatim into a struct definition.
    #[test]
    fn a_property_key_becomes_a_field_name_rust_accepts() {
        // Cloudflare keys properties by ASN, and by a bare `*`.
        assert_eq!(field_ident("13335"), "field_13335");
        assert_eq!(field_ident("*"), "field", "punctuation sanitises to nothing");
        // Leading punctuation sanitises to `_`, which to_snake_case then drops —
        // the wire name is kept by the caller's #[serde(rename)].
        assert_eq!(field_ident("$metadata"), "metadata");
        // Linode's filter operators.
        assert_eq!(field_ident("+gt"), "gt");
        // DigitalOcean's Postgres settings.
        assert_eq!(field_ident("pg_partman_bgw.interval"), "pg_partman_bgw_interval");
        // Keywords still take the raw-identifier form the generator has always used.
        assert_eq!(field_ident("type"), "r#type");
        assert_eq!(field_ident("self"), "_self");
    }

    #[test]
    fn a_valid_key_is_left_exactly_alone() {
        // The guarantee that keeps every provider's committed output from moving:
        // this function only changes names that were already broken.
        for name in ["account_id", "zone", "result_info", "id"] {
            assert_eq!(field_ident(name), name);
        }
        // Casing conversion is unchanged.
        assert_eq!(field_ident("ApiVersion"), "api_version");
    }

    /// `resolve_schema_key` is private; this is the regression that made 52 of
    /// Hetzner's 100 generated types opaque blobs.
    #[test]
    fn a_schema_key_resolves_even_when_pascal_casing_is_not_invertible() {
        use crate::spec::Schema;

        let mut schemas = std::collections::BTreeMap::new();
        // Hetzner's real key: the property is `public_net`, so the hoisted
        // component carries the underscore.
        schemas.insert(
            "CreateServerResponseServerPublic_net".to_string(),
            Schema {
                schema_type: Some("object".to_string()),
                properties: Some(Default::default()),
                ..Default::default()
            },
        );

        // The reference site pascal-cases, giving `…PublicNet` — which no amount
        // of snake-casing turns back into `…Public_net`. Matching in the
        // pascal-cased space is the direction that is actually well defined.
        assert!(
            UnifiedGenerator::resolve_schema_key(
                "CreateServerResponseServerPublicNet",
                &schemas
            )
            .is_some(),
            "the schema exists; failing to find it emits an opaque blob instead"
        );

        // A name nothing declares must still miss — the fallback is a real signal.
        assert!(UnifiedGenerator::resolve_schema_key("NoSuchType", &schemas).is_none());
    }

    #[test]
    fn a_direct_key_match_still_wins() {
        use crate::spec::Schema;

        let mut schemas = std::collections::BTreeMap::new();
        for key in ["Exact", "exact_other"] {
            schemas.insert(
                key.to_string(),
                Schema {
                    schema_type: Some("object".to_string()),
                    ..Default::default()
                },
            );
        }
        assert!(UnifiedGenerator::resolve_schema_key("Exact", &schemas).is_some());
        // Cloudflare's snake_case keys, via the existing rule.
        assert!(UnifiedGenerator::resolve_schema_key("ExactOther", &schemas).is_some());
    }
}
