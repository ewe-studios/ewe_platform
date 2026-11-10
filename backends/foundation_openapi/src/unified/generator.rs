//! Unified code generator for `OpenAPI` specs.
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
use super::emit::{
    self, Field, ModuleFile, TypeDef, Variant, VariantShape, ARGS_DERIVES, SCHEMA_DERIVES,
};

/// How deep `allOf` members are followed through `$ref`s before giving up — a
/// guard against a reference cycle, far past any real spec's nesting.
const MAX_ALL_OF_DEPTH: usize = 16;

/// The spec's schemas, with the lookups the generator makes on top of them.
///
/// `resolve` tries the same four guesses the generator always made. The last one
/// compares `PascalCase` names. Scanning every schema for it on each miss made
/// generation quadratic: Cloudflare has 11,937 schemas and spent most of its
/// time there. The index keeps, for each `PascalCase` name, the first schema in
/// key order, which is the one that scan returned. The dependency graph is built
/// once here as well, not once per group.
pub(crate) struct SchemaIndex<'a> {
    schemas: &'a std::collections::BTreeMap<String, crate::spec::Schema>,
    by_pascal: std::collections::HashMap<String, &'a crate::spec::Schema>,
    type_deps: std::collections::HashMap<String, std::collections::HashSet<String>>,
}

impl<'a> SchemaIndex<'a> {
    /// Index `schemas`, which must be the analysed spec's schema map.
    pub(crate) fn new(
        schemas: &'a std::collections::BTreeMap<String, crate::spec::Schema>,
    ) -> Self {
        let mut by_pascal = std::collections::HashMap::with_capacity(schemas.len());
        for (key, schema) in schemas {
            by_pascal
                .entry(crate::to_pascal_case(key))
                .or_insert(schema);
        }
        Self {
            schemas,
            by_pascal,
            type_deps: build_type_dependencies(schemas),
        }
    }

    /// The schema stored under exactly `key`.
    pub(crate) fn get(&self, key: &str) -> Option<&'a crate::spec::Schema> {
        self.schemas.get(key)
    }

    /// Which generated type names each type refers to.
    pub(crate) fn type_deps(
        &self,
    ) -> &std::collections::HashMap<String, std::collections::HashSet<String>> {
        &self.type_deps
    }

    /// Resolve a `PascalCase` type name to the original `OpenAPI` spec schema key.
    ///
    /// The `schemas` map uses the spec's original keys (typically `snake_case`),
    /// but `shared_resources` stores `PascalCase` names. Try direct lookup first
    /// (some specs use `PascalCase` keys), then convert to `snake_case`.
    pub(crate) fn resolve(&self, type_name: &str) -> Option<&'a crate::spec::Schema> {
        // 1. Direct lookup — the spec's key already matches the type name.
        if let Some(s) = self.get(type_name) {
            return Some(s);
        }
        // 2. Convert to snake_case (Cloudflare's spec uses snake_case keys).
        let snake = crate::to_snake_case(type_name);
        if let Some(s) = self.get(&snake) {
            return Some(s);
        }
        // 3. Lowercase the first char (e.g. "FooBar" → "fooBar").
        if let Some(first) = type_name.chars().next() {
            let lower_first =
                first.to_lowercase().collect::<String>() + &type_name[first.len_utf8()..];
            if let Some(s) = self.get(&lower_first) {
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
        self.by_pascal
            .get(&crate::to_pascal_case(type_name))
            .copied()
    }
}

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
    schemas: &SchemaIndex<'_>,
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
        format!("r#{ident}")
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
/// `DigitalOcean` has `pg_partman_bgw.interval`. Those went straight into the
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
fn is_nullable(schema: &crate::spec::Schema, schemas: &SchemaIndex<'_>) -> bool {
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
        match schemas.resolve(name) {
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
        format!("r#{ident}")
    } else {
        ident.to_string()
    }
}

/// Sanitize a module name so it can be used in `pub mod NAME;`.
/// `r#move` is invalid syntax, so we rename problematic keywords with a `_` suffix.
fn sanitize_module_name(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("{name}_mod")
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
        format!("{type_name}Type")
    } else {
        type_name.to_string()
    }
}

/// Extract inner type names from a type string (handles Vec<T>, Option<T>, etc.).
/// Adds extracted type names to the `all_types` set.
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
/// Adds found type names (in `PascalCase`) to `seen_types` and `types_to_process`.
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
        for prop_schema in properties.values() {
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
/// Returns: Map<`type_name`, Set<types it references>>
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

#[allow(clippy::only_used_in_recursion)] // The map is passed down the recursion; removing it changes every caller.
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
        for prop in properties.values() {
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
            return format!("Option<Box<{inner}>>");
        }
    }
    // Handle Vec<T> -> Vec<Box<T>> if T is recursive
    if let Some(inner) = field_type
        .strip_prefix("Vec<")
        .and_then(|s| s.strip_suffix(">"))
    {
        if recursive_types.contains(inner) {
            return format!("Vec<Box<{inner}>>");
        }
    }
    // Handle Option<Vec<T>> -> Option<Vec<Box<T>>> if T is recursive
    if let Some(inner) = field_type
        .strip_prefix("Option<Vec<")
        .and_then(|s| s.strip_suffix(">>"))
    {
        if recursive_types.contains(inner) {
            return format!("Option<Vec<Box<{inner}>>>");
        }
    }
    // Direct type reference that is recursive
    if recursive_types.contains(field_type) {
        return format!("Box<{field_type}>");
    }

    field_type.to_string()
}

/// Whether `ty` mentions the identifier `name` (as a whole word).
fn type_ident_in(ty: &str, name: &str) -> bool {
    ty.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|ident| ident == name)
}

/// Transform an `OpenAPI` path into a Rust format! string.
/// Converts `{param}` placeholders to `{}` only for params in `path_params`.
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
                            write!(result, "{{{{{param_content}}}}}")
                                .expect("writing to a String cannot fail");
                        }
                        break;
                    }
                    param_content.push(chars.next().unwrap());
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
/// Converts `PascalCase` to `snake_case`, removes redundant words, and normalizes identifiers.
fn sanitize_group_name(name: &str) -> String {
    // Step 1: Convert PascalCase/CamelCase to snake_case
    // Insert underscore before each uppercase letter that follows a lowercase letter or digit
    let mut snake_case = String::new();
    let mut prev_was_upper_or_digit = false;

    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            // Insert underscore before uppercase if previous char was lowercase or digit
            if i > 0 && !prev_was_upper_or_digit {
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
        result = format!("{result}_{hash}");
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

/// Errors from [`rewrite_features_for_groups`] and [`rewrite_umbrella_for_split`].
#[derive(Debug, derive_more::Display)]
pub enum FeatureRewriteError {
    /// The manifest is not valid TOML.
    #[display("Cargo.toml does not parse: {_0}")]
    Parse(toml_edit::TomlError),
    /// The manifest has no `[features]` table to add the group features to.
    #[display("Cargo.toml has no [features] table")]
    MissingFeatures,
    /// The manifest could not be read or written.
    #[display("{}: {}", path.display(), source)]
    Io {
        /// The manifest path.
        path: std::path::PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
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
    let mut doc: toml_edit::DocumentMut = manifest.parse().map_err(FeatureRewriteError::Parse)?;

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

/// Rewrite the umbrella manifest for a per-group-crate split (feature 22 E3).
///
/// The old layout had one crate with `cloudflare_<group> = []` marker features
/// gating modules in `src/generated/`. The split layout keeps the feature names
/// but makes each one pull its own crate:
/// `cloudflare_<group> = ["dep:foundation_cloudflare_<group>"]`, with the
/// optional dependency added beside it. The `cloudflare` and `cloudflare_all`
/// umbrellas keep listing every group, so a caller enabling `cloudflare` (or
/// `default`) still gets every group, exactly as before.
///
/// # Errors
/// [`FeatureRewriteError`] when the manifest does not parse or has no
/// `[features]` table.
pub fn rewrite_umbrella_for_split(
    manifest_path: &Path,
    provider: &str,
    groups: &[ApiGroup],
) -> Result<(), FeatureRewriteError> {
    let raw = fs::read_to_string(manifest_path).map_err(|e| FeatureRewriteError::Io {
        path: manifest_path.to_path_buf(),
        source: e,
    })?;
    let updated = rewrite_umbrella_for_split_str(&raw, provider, groups)?;
    if updated != raw {
        fs::write(manifest_path, updated).map_err(|e| FeatureRewriteError::Io {
            path: manifest_path.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

/// The target key for dependencies the umbrella needs only on native targets.
pub const NATIVE_ONLY_CFG: &str = "cfg(not(target_family = \"wasm\"))";

/// The pure half of [`rewrite_umbrella_for_split`].
///
/// Puts every group crate and the shared types crate in the umbrella's
/// native-only dependency table (see [`NATIVE_ONLY_CFG`]) and moves any entries an
/// older generator left in the all-target `[dependencies]` table.
///
/// # Errors
///
/// [`FeatureRewriteError::Parse`] if `manifest` is not valid TOML, and
/// [`FeatureRewriteError::MissingFeatures`] if it has no `[features]` table (or a
/// `target` / `dependencies` entry that is not a table).
#[allow(clippy::too_many_lines)] // One template per generated unit; splitting it is a refactor, not part of this lint pass.
pub fn rewrite_umbrella_for_split_str(
    manifest: &str,
    provider: &str,
    groups: &[ApiGroup],
) -> Result<String, FeatureRewriteError> {
    let mut doc: toml_edit::DocumentMut = manifest.parse().map_err(FeatureRewriteError::Parse)?;

    let provider_feature = provider.replace(['-', '/'], "_");
    let all_feature = format!("{provider_feature}_all");
    let group_prefix = format!("{provider_feature}_");

    // Group features in a stable order, deduplicated by sanitised name.
    let mut group_entries: Vec<(String, String)> = Vec::new(); // (feature, crate)
    let mut seen = std::collections::HashSet::new();
    for group in groups {
        let safe = sanitize_module_name(&sanitize_group_name(&group.name));
        if !seen.insert(safe.clone()) {
            continue;
        }
        group_entries.push((
            format!("{provider_feature}_{safe}"),
            format!("foundation_{provider_feature}_{safe}"),
        ));
    }

    // Optional dependencies: one per group crate, plus the shared types crate the
    // umbrella shim re-exports. They go in the NATIVE-ONLY target table: the
    // umbrella's `generated` module is `#[cfg(not(target_family = "wasm"))]`, and
    // each group crate pulls `foundation_deployment` and `foundation_netio/multi`
    // (the native thread pool). Declared for every target, they forced
    // `foundation_core/multi` into every wasm32 build of the umbrella, which does
    // not compile there. Scoped first so the later `features` borrow of `doc`
    // does not overlap it.
    {
        let mut names: Vec<String> = group_entries.iter().map(|(_, c)| c.clone()).collect();
        names.push("foundation_cloudflare_types".to_string());

        // Move entries an older generator wrote into the all-target table, and
        // drop the hash dependencies the umbrella no longer needs (the shared
        // module with its `JsonHash` derives lives in the types and group crates).
        if let Some(all) = doc
            .get_mut("dependencies")
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            for name in &names {
                all.remove(name);
            }
            all.remove("base85rs");
            all.remove("sha2");
        }

        let target = doc
            .entry("target")
            .or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            })
            .as_table_like_mut()
            .ok_or(FeatureRewriteError::MissingFeatures)?;
        let native = target
            .entry(NATIVE_ONLY_CFG)
            .or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            })
            .as_table_like_mut()
            .ok_or(FeatureRewriteError::MissingFeatures)?;
        let deps = native
            .entry("dependencies")
            .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_like_mut()
            .ok_or(FeatureRewriteError::MissingFeatures)?;
        for name in &names {
            if deps.get(name).is_none() {
                let mut entry = toml_edit::InlineTable::new();
                entry.insert("path", format!("../{name}").into());
                entry.insert("version", "0.1.0".into());
                entry.insert("optional", true.into());
                deps.insert(name, toml_edit::value(entry));
            }
        }
    }

    let features = doc
        .get_mut("features")
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or(FeatureRewriteError::MissingFeatures)?;

    // One feature per group, enabling its crate. A single-string array.
    let str_array = |items: &[String]| -> toml_edit::Item {
        let mut array = toml_edit::Array::new();
        for item in items {
            let mut value = toml_edit::Value::from(item.as_str());
            value.decor_mut().set_prefix("\n    ");
            array.push_formatted(value);
        }
        if !items.is_empty() {
            array.set_trailing("\n");
            array.set_trailing_comma(true);
        }
        toml_edit::value(array)
    };

    // Each group feature now enables its crate. The `<provider>_<group>` key
    // stays exactly the same name callers already pass.
    for (feature, crate_name) in &group_entries {
        set_feature(features, feature, str_array(&[format!("dep:{crate_name}")]));
    }

    // Drop stale group features whose group no longer exists.
    let stale: Vec<String> = features
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| {
            k.starts_with(&group_prefix)
                && *k != all_feature
                && !group_entries.iter().any(|(f, _)| f == k)
        })
        .collect();
    for key in stale {
        features.remove(&key);
    }

    // The umbrellas keep listing every group, plus any hand-written entries.
    let group_features: Vec<String> = group_entries.iter().map(|(f, _)| f.clone()).collect();
    for umbrella in [&provider_feature, &all_feature] {
        let mut entries: Vec<String> = features
            .get(umbrella)
            .and_then(toml_edit::Item::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .filter(|e| !e.starts_with(&group_prefix))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        entries.extend(group_features.iter().cloned());
        // The shim's `shared` module (compiled only when the base feature is on)
        // re-exports the shared types crate, so the base feature pulls it in.
        if *umbrella == provider_feature
            && !entries
                .iter()
                .any(|e| e == "dep:foundation_cloudflare_types")
        {
            entries.push("dep:foundation_cloudflare_types".to_string());
        }
        set_feature(features, umbrella, str_array(&entries));
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
///
/// `rustfmt` runs with an **empty configuration**, not the repository's
/// `rustfmt.toml`. That file lists the generated Cloudflare crates under
/// `ignore` (so `cargo fmt` leaves them alone), and `rustfmt` honours `ignore`
/// even for a file named on the command line: an in-tree run then wrote
/// unformatted code, while `Codegen::check`, which generates in a scratch tree
/// the file does not reach, compared against formatted code. One configuration
/// for both means a regeneration is byte-for-byte what `check` expects.
fn format_files(paths: &[PathBuf]) -> Result<(), GenError> {
    if paths.is_empty() {
        return Ok(());
    }
    let config_dir = std::env::temp_dir().join(format!(
        "ewe-openapi-rustfmt-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&config_dir)?;
    fs::write(config_dir.join("rustfmt.toml"), "")?;
    let output = std::process::Command::new("rustfmt")
        .args(["--edition", "2021", "--config-path"])
        .arg(&config_dir)
        .args(paths)
        .output();
    let _ = fs::remove_dir_all(&config_dir);
    let output = output.map_err(|e| GenError::Rustfmt(format!("could not run rustfmt: {e}")))?;
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
    /// crate (e.g. cloudflare → `foundation_deployment_cloudflare/src`).
    provider_dir_override: Option<PathBuf>,
    /// Per-group crate split: one crate per group is created under this
    /// directory (a workspace `backends/` root). The umbrella crate keeps only
    /// a re-export shim, so its compile cost drops to the groups a caller
    /// enables. See [`UnifiedGenerator::split_group_crates`].
    groups_dir: Option<PathBuf>,
    /// The shared types crate (`foundation_cloudflare_types`) that the group
    /// crates import their cross-group types from. In split mode the shared
    /// module is written here instead of `generated/shared/mod.rs`.
    shared_crate_dir: Option<PathBuf>,
    /// Where the group crates' sibling crates are, for reading their versions
    /// into the group manifests. Defaults to [`Self::groups_dir`]. Set it when
    /// `groups_dir` is a scratch tree (see [`UnifiedGenerator::with_sibling_crates`]).
    sibling_crates_dir: Option<PathBuf>,
    /// Schemas (`PascalCase` type names) whose `oneOf` is emitted as an untagged
    /// enum instead of `serde_json::Value`. Opt-in per schema: a `oneOf` has no
    /// single right Rust shape, and turning every one into an enum would change
    /// the public types of every provider.
    typed_unions: std::collections::BTreeSet<String>,
    /// Shared-types-only mode (see [`UnifiedGenerator::shared_types_only`]).
    shared_types_only: bool,
}

/// The `Cargo.toml` of one generated group crate.
///
/// **WHY:** the manifest names a version for every path dependency so the crate
/// stays publishable. A version written into the generator goes stale as soon
/// as the sibling crate's version moves. The build script then finds the
/// committed manifests stale, rewrites them, and the next `cargo metadata`
/// fails: the requirement names a version the sibling no longer has.
///
/// **WHAT:** renders the manifest, reading each sibling's version from that
/// crate's own manifest. The group crate's own version comes from its committed
/// manifest in `siblings`, or is `0.1.0` when the crate does not exist yet.
///
/// **HOW:** the group crates live under `backends/`, next to the crates they
/// depend on, so `siblings` is the workspace root that holds those crates and
/// each dependency's manifest is `<siblings>/<dependency>/Cargo.toml`. Pass the
/// shipped workspace root, not the directory being written: a scratch render
/// has no siblings in it.
///
/// # Errors
/// [`GenError::ReadFile`] when a sibling manifest cannot be read, and
/// [`GenError::ManifestVersion`] when it carries no `[package] version`.
pub fn group_crate_manifest(
    siblings: &Path,
    crate_name: &str,
    safe: &str,
) -> Result<String, GenError> {
    use std::fmt::Write as _;

    let types = sibling_version(siblings, "foundation_cloudflare_types")?;
    let deployment = sibling_version(siblings, "foundation_deployment")?;
    let macros_crate = sibling_version(siblings, "foundation_macros")?;
    let netio = sibling_version(siblings, "foundation_netio")?;
    // The group crate keeps its own committed version, so a release bump
    // survives a regeneration. A group crate that does not exist yet starts at
    // 0.1.0. Any other read or parse failure is an error, not a silent default.
    let own = match sibling_version(siblings, crate_name) {
        Ok(version) => version,
        Err(GenError::ReadFile { ref source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            "0.1.0".to_string()
        }
        Err(other) => return Err(other),
    };

    let mut manifest = String::new();
    writeln!(manifest, "[package]").unwrap();
    writeln!(manifest, "name = \"{crate_name}\"").unwrap();
    writeln!(manifest, "version = \"{own}\"").unwrap();
    writeln!(manifest, "edition.workspace = true").unwrap();
    writeln!(manifest, "rust-version.workspace = true").unwrap();
    writeln!(manifest, "license.workspace = true").unwrap();
    writeln!(manifest, "authors.workspace = true").unwrap();
    writeln!(manifest, "repository.workspace = true").unwrap();
    writeln!(
        manifest,
        "description = \"Generated Cloudflare API client: {safe} group\""
    )
    .unwrap();
    writeln!(manifest).unwrap();
    writeln!(manifest, "[dependencies]").unwrap();
    writeln!(
        manifest,
        "foundation_cloudflare_types = {{ path = \"../foundation_cloudflare_types\", version = \"{types}\" }}"
    )
    .unwrap();
    writeln!(
        manifest,
        "foundation_deployment = {{ path = \"../foundation_deployment\", version = \"{deployment}\" }}"
    )
    .unwrap();
    writeln!(
        manifest,
        "foundation_macros = {{ path = \"../foundation_macros\", version = \"{macros_crate}\" }}"
    )
    .unwrap();
    writeln!(
        manifest,
        "foundation_netio = {{ path = \"../foundation_netio\", version = \"{netio}\", default-features = false, features = [\"ssl\", \"multi\"] }}"
    )
    .unwrap();
    writeln!(manifest, "serde = {{ workspace = true }}").unwrap();
    writeln!(manifest, "serde_json = {{ workspace = true }}").unwrap();
    // The generated module derives `foundation_macros::JsonHash`, whose
    // expansion uses `sha2` for the digest and `base85rs` for the encoding.
    writeln!(manifest, "base85rs = \"0.1\"").unwrap();
    writeln!(manifest, "sha2 = \"0.10\"").unwrap();
    writeln!(manifest).unwrap();
    writeln!(manifest, "[lints]").unwrap();
    writeln!(manifest, "workspace = true").unwrap();
    Ok(manifest)
}

/// The version that a group crate in `siblings` must require of the sibling
/// crate `dependency`.
///
/// A path dependency needs a version too, or `cargo publish` rejects the crate.
/// Reading it from the sibling's own `[package] version` keeps the requirement
/// and the crate it points at from drifting apart.
fn sibling_version(siblings: &Path, dependency: &str) -> Result<String, GenError> {
    let path = siblings.join(dependency).join("Cargo.toml");
    let display = path.display().to_string();
    let raw = fs::read_to_string(&path).map_err(|source| GenError::ReadFile {
        path: display.clone(),
        source,
    })?;
    let doc: toml_edit::DocumentMut = raw.parse().map_err(|_| GenError::ManifestVersion {
        path: display.clone(),
        dependency: dependency.to_string(),
    })?;
    // `get`, not indexing: a manifest with no `[package]` table must return
    // this error, not panic.
    doc.get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml_edit::Item::as_str)
        .map(str::to_string)
        .ok_or_else(|| GenError::ManifestVersion {
            path: display,
            dependency: dependency.to_string(),
        })
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
    #[display("{path} names no [package] version to require {dependency} by")]
    ManifestVersion { path: String, dependency: String },
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
    #[must_use]
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            provider_dir_override: None,
            groups_dir: None,
            shared_crate_dir: None,
            sibling_crates_dir: None,
            typed_unions: std::collections::BTreeSet::new(),
            shared_types_only: false,
        }
    }

    /// Emit the named schemas' `oneOf` as untagged enums.
    ///
    /// WHY: a `oneOf` request body such as D1's `d1_batch_query` (one query, or
    /// a batch of them) generated as `serde_json::Value`, so callers built the
    /// body by hand with `json!`. Names are the spec's schema names
    /// (`d1_batch_query`); a name that is no `oneOf` schema is ignored.
    #[must_use]
    pub fn with_typed_unions<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.typed_unions = names
            .into_iter()
            .map(|n| rename_std_type_conflict(&crate::to_pascal_case(n.as_ref())))
            .collect();
        self
    }

    /// Override the provider output directory. When set, `generate()` writes
    /// files to this path instead of `output_dir.join(provider)`.
    #[must_use]
    pub fn with_provider_dir(mut self, dir: PathBuf) -> Self {
        self.provider_dir_override = Some(dir);
        self
    }

    /// Split the generated groups into one crate per group.
    ///
    /// WHY: one provider crate compiling every generated group at once made a
    /// single `rustc` need ~12 GB and get OOM-killed in CI (feature 22 E3). One
    /// crate per group lets rustc compile the groups in parallel and lets a
    /// caller pay only for the groups it enables.
    ///
    /// WHAT: `groups_dir` names the workspace root the group crates are created
    /// under (one `foundation_<provider>_<group>/` per group); `shared_crate_dir`
    /// names the crate that receives the cross-group `shared` types. The umbrella
    /// crate keeps only a feature-gated re-export shim.
    #[must_use]
    pub fn split_group_crates(mut self, groups_dir: PathBuf, shared_crate_dir: PathBuf) -> Self {
        self.groups_dir = Some(groups_dir);
        self.shared_crate_dir = Some(shared_crate_dir);
        self
    }

    /// Name the workspace root that holds the group crates' sibling crates.
    ///
    /// WHY: a group crate's manifest requires a version of each path dependency,
    /// and that version is read from the sibling's own `Cargo.toml`. When the
    /// group crates themselves are rendered into a scratch tree (`Codegen::check`
    /// does that), the scratch tree holds no siblings, so the versions must come
    /// from the shipped workspace instead.
    ///
    /// WHAT: `dir` is read for `[package] version` only; nothing is written to
    /// it. It defaults to the `groups_dir` given to
    /// [`UnifiedGenerator::split_group_crates`].
    #[must_use]
    pub fn with_sibling_crates(mut self, dir: PathBuf) -> Self {
        self.sibling_crates_dir = Some(dir);
        self
    }

    /// Write only the shared types module, not the group crates.
    ///
    /// Split mode normally writes three things: the shared types crate's
    /// `generated.rs`, one crate per group, and the umbrella shim. With this set,
    /// [`UnifiedGenerator::generate`] still analyses the spec and builds every
    /// group, because the hoisted shared types depend on all of them. It then
    /// writes and formats `generated.rs` alone and stops. It exists for
    /// `Codegen::check_shared_types`, which compares that one file.
    #[must_use]
    pub fn shared_types_only(mut self) -> Self {
        self.shared_types_only = true;
        self
    }

    /// Generate all artifacts for a provider as cohesive per-endpoint units.
    ///
    /// # Errors
    /// [`GenError`] when the analysis fails, a file cannot be written, or `rustfmt` fails.
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
        let index = SchemaIndex::new(&analysis.schemas);
        let mut modules = vec![shared];
        for group in &analysis.groups {
            let module = self.generate_group_module(
                provider,
                group,
                &analysis.shared_resources,
                &index,
                &shared_names,
            )?;
            modules.push(module);
        }

        // Collapse the duplicates (see `emit`), then write and format.
        emit::shrink(&mut modules, 0);

        // Split mode: one crate per group, plus a shared types crate and a thin
        // re-export shim in the umbrella crate. The single-crate layout below is
        // the monolith (and existing split-out-provider) path.
        if let (Some(groups_dir), Some(shared_crate_dir)) =
            (&self.groups_dir, &self.shared_crate_dir)
        {
            return self.write_split(
                provider,
                &analysis.groups,
                &modules,
                groups_dir,
                shared_crate_dir,
                &generated_dir,
            );
        }

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
        self.generate_provider_mod(
            provider,
            &analysis.groups,
            &analysis.shared_resources,
            &generated_dir,
        )?;
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
            self.output_dir.ancestors().nth(2).map_or_else(
                || PathBuf::from("backends/foundation_deployment/Cargo.toml"),
                |p| p.join("Cargo.toml"),
            )
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

    /// Split mode: emit the shared types crate, one crate per group, and the
    /// umbrella's re-export shim. See [`UnifiedGenerator::split_group_crates`].
    ///
    /// `modules[0]` is the shared module (after `emit::shrink` hoisted every
    /// cross-group shape into it); `modules[1..]` are the per-group modules, in
    /// the same order as `groups`.
    fn write_split(
        &self,
        provider: &str,
        groups: &[ApiGroup],
        modules: &[ModuleFile],
        groups_dir: &Path,
        shared_crate_dir: &Path,
        generated_dir: &Path,
    ) -> Result<(), GenError> {
        let provider_safe = provider.replace(['-', '/'], "_");

        // 1. Shared types crate: `src/generated.rs` (the shared module, minus the
        //    common re-export header, which each group crate re-declares itself).
        let shared_module = &modules[0];
        let shared_src = shared_crate_dir.join("src/generated.rs");
        if let Some(dir) = shared_src.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&shared_src, shared_module.render()?)?;

        // Shared-types-only mode stops here: the shared module is the one file
        // the caller compares, so the group crates are neither rendered nor written.
        if self.shared_types_only {
            format_files(std::slice::from_ref(&shared_src))?;
            return Ok(());
        }

        // 2. One crate per group. The group module's `super::shared` resolves to
        //    the `shared` module we emit in the crate root, so the module body is
        //    the same as the single-crate layout — only the feature gate is gone.
        let group_modules = &modules[1..];
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut written = vec![shared_src.clone()];
        // Each group manifest requires the versions the sibling crates have. A
        // caller that renders into a scratch tree names the shipped root instead
        // (see `with_sibling_crates`), because a scratch tree holds no siblings.
        let siblings = self
            .sibling_crates_dir
            .clone()
            .unwrap_or_else(|| groups_dir.to_path_buf());
        for (group, module) in groups.iter().zip(group_modules) {
            let safe = sanitize_module_name(&sanitize_group_name(&group.name));
            if !seen.insert(safe.clone()) {
                continue; // two groups sanitising to one name — keep the first
            }
            let crate_name = format!("foundation_{provider_safe}_{safe}");
            let crate_dir = groups_dir.join(&crate_name);
            self.write_group_crate(&crate_name, &safe, module, &crate_dir, &siblings)?;
            // The shim `lib.rs` is formatted too: it carries the `#![allow]`
            // attributes and the `pub use` lists that rustfmt wraps and sorts.
            written.push(crate_dir.join("src/lib.rs"));
            written.push(crate_dir.join("src").join(format!("{safe}/mod.rs")));
        }

        // 3. Umbrella re-export shim replaces the old per-group modules. Clear
        //    the generated directory first: the monolith layout left one module
        //    directory per group (plus `shared/`), and a group that disappears
        //    or renames must not leave dead modules behind.
        if generated_dir.exists() {
            fs::remove_dir_all(generated_dir).map_err(|e| GenError::WriteFile {
                path: generated_dir.display().to_string(),
                source: e,
            })?;
        }
        self.write_umbrella_shim(provider, groups, generated_dir)?;
        written.push(generated_dir.join("mod.rs"));
        format_files(&written)?;

        // 4. The umbrella manifest now turns each group feature into an optional
        //    dependency on its group crate.
        let umbrella_manifest = self.output_dir.join("Cargo.toml");
        if umbrella_manifest.exists() {
            rewrite_umbrella_for_split(&umbrella_manifest, provider, groups).map_err(|e| {
                GenError::WriteFile {
                    path: umbrella_manifest.display().to_string(),
                    source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                }
            })?;
        }

        Ok(())
    }

    /// Emit one group crate: manifest, `lib.rs` (a `shared` re-export module +
    /// the group module), and the group module file itself.
    ///
    /// The manifest comes from [`group_crate_manifest`].
    #[allow(clippy::too_many_lines)] // One template per generated unit; splitting it is a refactor, not part of this lint pass.
    #[allow(clippy::unused_self)] // Kept on `self` so it sits with the other generator methods.
    fn write_group_crate(
        &self,
        crate_name: &str,
        safe: &str,
        module: &ModuleFile,
        crate_dir: &Path,
        siblings: &Path,
    ) -> Result<(), GenError> {
        use std::fmt::Write as _;

        fs::create_dir_all(crate_dir.join("src"))?;

        // The generated manifest is deterministic and self-contained. Its deps
        // mirror what the group module imports directly.
        let manifest = group_crate_manifest(siblings, crate_name, safe)?;
        fs::write(crate_dir.join("Cargo.toml"), manifest)?;

        // lib.rs: the `shared` module the group module imports via `super::shared`,
        // then the group module itself.
        let mut lib = String::new();
        writeln!(
            lib,
            "//! Generated Cloudflare API client for the `{safe}` group."
        )
        .unwrap();
        writeln!(lib, "//!").unwrap();
        writeln!(lib, "//! Generated by `cargo run --bin genapi generate`.").unwrap();
        writeln!(lib, "//! DO NOT EDIT MANUALLY.").unwrap();
        writeln!(lib).unwrap();
        writeln!(
            lib,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines, clippy::struct_excessive_bools)]"
        )
        .unwrap();
        writeln!(
            lib,
            "#![allow(clippy::missing_errors_doc, clippy::doc_markdown, clippy::useless_format)]"
        )
        .unwrap();
        writeln!(lib, "#![allow(unused_imports)]").unwrap();
        writeln!(lib).unwrap();
        writeln!(lib, "pub mod shared {{").unwrap();
        writeln!(
            lib,
            "    pub use foundation_deployment::providers::common::{{"
        )
        .unwrap();
        writeln!(
            lib,
            "        read_error_body, send_bytes, send_checked, send_json, send_json_lines, send_without_body,"
        )
        .unwrap();
        writeln!(
            lib,
            "        with_raw_body, ApiClient, BodyLimits, ByteStream, JsonLinesStream,"
        )
        .unwrap();
        writeln!(lib, "    }};").unwrap();
        writeln!(
            lib,
            "    pub use foundation_deployment::providers::common::{{"
        )
        .unwrap();
        writeln!(
            lib,
            "        ApiError, ApiPending, ApiResponse, BoxedSendExecutionAction, Empty, Operation,"
        )
        .unwrap();
        writeln!(lib, "    }};").unwrap();
        writeln!(
            lib,
            "    pub use foundation_cloudflare_types::generated::*;"
        )
        .unwrap();
        writeln!(lib, "}}").unwrap();
        writeln!(lib).unwrap();
        writeln!(lib, "pub mod {safe};").unwrap();
        fs::write(crate_dir.join("src/lib.rs"), lib)?;

        // The group module itself, verbatim (its `super::shared` resolves to the
        // `shared` module above).
        let mod_path = crate_dir.join("src").join(safe).join("mod.rs");
        fs::create_dir_all(mod_path.parent().expect("group module has a parent"))?;
        fs::write(&mod_path, module.render()?)?;

        Ok(())
    }

    /// The umbrella's `generated/mod.rs` in split mode: a feature-gated re-export
    /// shim for every group crate, plus a `shared` module mirroring the group
    /// crates' own, so `foundation_deployment_cloudflare::generated::*` keeps its
    /// old shape.
    #[allow(clippy::unused_self)] // Kept on `self` so it sits with the other generator methods.
    fn write_umbrella_shim(
        &self,
        provider: &str,
        groups: &[ApiGroup],
        generated_dir: &Path,
    ) -> Result<(), GenError> {
        use std::fmt::Write as _;
        let provider_safe = provider.replace(['-', '/'], "_");

        fs::create_dir_all(generated_dir)?;
        let mut out = String::new();
        writeln!(out, "//! Auto-generated module for {provider}.").unwrap();
        writeln!(out, "//!").unwrap();
        writeln!(out, "//! Generated by `cargo run --bin genapi generate`.").unwrap();
        writeln!(out, "//! DO NOT EDIT MANUALLY.").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "#![cfg(feature = \"{provider_safe}\")]").unwrap();
        writeln!(
            out,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines, clippy::struct_excessive_bools)]"
        )
        .unwrap();
        writeln!(
            out,
            "#![allow(clippy::missing_errors_doc, clippy::doc_markdown, clippy::useless_format)]"
        )
        .unwrap();
        writeln!(out, "#![allow(unused_imports)]").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "pub mod shared {{").unwrap();
        writeln!(
            out,
            "    pub use foundation_deployment::providers::common::{{"
        )
        .unwrap();
        writeln!(
            out,
            "        read_error_body, send_bytes, send_checked, send_json, send_json_lines, send_without_body,"
        )
        .unwrap();
        writeln!(
            out,
            "        with_raw_body, ApiClient, BodyLimits, ByteStream, JsonLinesStream,"
        )
        .unwrap();
        writeln!(out, "    }};").unwrap();
        writeln!(
            out,
            "    pub use foundation_deployment::providers::common::{{"
        )
        .unwrap();
        writeln!(
            out,
            "        ApiError, ApiPending, ApiResponse, BoxedSendExecutionAction, Empty, Operation,"
        )
        .unwrap();
        writeln!(out, "    }};").unwrap();
        writeln!(
            out,
            "    pub use foundation_cloudflare_types::generated::*;"
        )
        .unwrap();
        writeln!(out, "}}").unwrap();
        writeln!(out).unwrap();

        let mut seen = std::collections::HashSet::new();
        for group in groups {
            let safe = sanitize_module_name(&sanitize_group_name(&group.name));
            if !seen.insert(safe.clone()) {
                continue;
            }
            // A module, not a bare `pub use … as {safe}`: the monolith layout had
            // `generated::<safe>` be the group module itself, so its types live at
            // `generated::<safe>::<Type>`. The group crate wraps that module one
            // level deeper (`foundation_<provider>_<safe>::<safe>`); re-exporting
            // the module's contents here keeps every existing `generated::<safe>`
            // path unchanged.
            writeln!(out, "#[cfg(feature = \"{provider_safe}_{safe}\")]").unwrap();
            writeln!(out, "pub mod {safe} {{").unwrap();
            writeln!(
                out,
                "    pub use foundation_{provider_safe}_{safe}::{safe}::*;"
            )
            .unwrap();
            writeln!(out, "}}").unwrap();
        }

        fs::write(generated_dir.join("mod.rs"), out)?;
        Ok(())
    }

    /// Generate a single group module with per-endpoint cohesive units.
    #[allow(clippy::too_many_lines)] // One template per generated unit; splitting it is a refactor, not part of this lint pass.
    fn generate_group_module(
        &self,
        provider: &str,
        group: &ApiGroup,
        shared_resources: &[String],
        schemas: &SchemaIndex<'_>,
        actual_shared_types: &HashSet<String>,
    ) -> Result<ModuleFile, GenError> {
        // Sanitize group name for use as directory/file name
        let safe_name = sanitize_group_name(&group.name);
        let safe_name = sanitize_module_name(&safe_name);
        let mut module = ModuleFile::new(format!("{safe_name}/mod.rs"));

        let mut out = String::new();

        // File header
        let provider_safe = provider.replace(['-', '/'], "_");
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
        // In split mode the group is its own crate, so the umbrella feature gate
        // (and its doc line) do not exist; the crate is pulled in by the feature
        // on the umbrella manifest instead.
        if self.groups_dir.is_none() {
            writeln!(out, "//! Feature flag: `{provider_safe}_{safe_name} `")?;
            writeln!(out)?;
            writeln!(out, "#![cfg(feature = \"{provider_safe}_{safe_name}\")]")?;
        }
        writeln!(
            out,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines, clippy::struct_excessive_bools)]"
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
                .is_some_and(|c| c.is_alphabetic() || c == '_')
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
        let mut already_traversed: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        while let Some(type_name) = types_to_process.pop() {
            if already_traversed.contains(&type_name.to_lowercase()) {
                continue;
            }
            if let Some(schema) = schemas.resolve(&type_name) {
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
                if let Some(schema) = schemas.resolve(type_name) {
                    collect_referenced_type_names(schema, &mut all_types, &mut types_to_process);
                }
            }
            while let Some(type_name) = types_to_process.pop() {
                if already_traversed.contains(&type_name.to_lowercase()) {
                    continue;
                }
                if let Some(schema) = schemas.resolve(&type_name) {
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
        let type_deps = schemas.type_deps();
        let recursive_types = find_recursive_types(type_deps);

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
            let def = match schemas.resolve(type_name) {
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
                // A boolean is sent as `true`/`false`; an array is sent as one
                // `name=value` pair per value.
                let (doc, ty) = if ep.boolean_query_params.contains(param) {
                    (
                        format!("Query parameter: `{param}` (true/false)."),
                        "Option<bool>",
                    )
                } else if ep.array_query_params.contains(param) {
                    (
                        format!("Query parameter: `{param}` (one per value)."),
                        "Vec<String>",
                    )
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
        schemas: &SchemaIndex<'_>,
        recursive_types: &std::collections::HashSet<String>,
    ) -> TypeDef {
        use crate::spec::Schema as SpecSchema;
        use std::collections::BTreeMap;

        let doc = format!("`{type_name}` type.");
        if self.typed_unions.contains(type_name) {
            if let Some(def) = self.union_def(type_name, &doc, schema, schemas, recursive_types) {
                return def;
            }
        }
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
        let field =
            |field_name: &str, prop_name: &str, prop_schema: &SpecSchema, required: bool| {
                self.field_of(
                    field_name,
                    prop_name,
                    prop_schema,
                    required,
                    schemas,
                    recursive_types,
                )
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
                fields.push(field(
                    field_name,
                    prop_name,
                    prop_schema,
                    required.contains(prop_name),
                ));
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
                fields.push(field(
                    &field_name,
                    prop_name,
                    prop_schema,
                    required.contains(prop_name),
                ));
            }
        }

        TypeDef::structure(type_name, &doc, SCHEMA_DERIVES, fields)
    }

    /// One struct field for property `prop_name` of a schema.
    ///
    /// WHY: `OpenAPI` specs use mixed naming conventions (`PascalCase`,
    /// `camelCase`, `snake_case`). We convert all property names to `snake_case`
    /// for Rust fields, so we must emit per-field `#[serde(rename)]` when the
    /// original name differs from the `snake_cased` field name. A single
    /// `rename_all` can't handle mixed specs like Docker (`ApiVersion` +
    /// `architecture` in the same schema).
    ///
    /// `required` and `nullable` say different things: the first that the key is
    /// always present, the second that its value may be null. A field can be
    /// both — Hetzner's pagination `next_page` is always there and is null on
    /// the last page. Only a field that is required AND not nullable can be a
    /// bare T.
    fn field_of(
        &self,
        field_name: &str,
        prop_name: &str,
        prop_schema: &crate::spec::Schema,
        required: bool,
        schemas: &SchemaIndex<'_>,
        recursive_types: &std::collections::HashSet<String>,
    ) -> Field {
        let rust_type = self.schema_to_rust_type(prop_schema, schemas);
        let rust_type = maybe_box_type(&rust_type, recursive_types);
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
    }

    /// An untagged enum for a `oneOf` schema, or `None` when the schema is not
    /// one (so the caller falls back to the plain type).
    ///
    /// A `$ref` member becomes `Name(Name)`; an inline object becomes a struct
    /// variant named after its `title` (or `VariantN`); a scalar or list becomes
    /// a newtype variant named after its type. serde tries the variants in spec
    /// order, so a spec that lists the permissive member first would shadow the
    /// specific one; D1 lists the single query first and it requires `sql`, which
    /// the batch object lacks.
    fn union_def(
        &self,
        type_name: &str,
        doc: &str,
        schema: &crate::spec::Schema,
        schemas: &SchemaIndex<'_>,
        recursive_types: &std::collections::HashSet<String>,
    ) -> Option<TypeDef> {
        let members = schema.one_of.as_ref().filter(|m| m.len() >= 2)?;
        let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut variants = Vec::with_capacity(members.len());
        for (index, member) in members.iter().enumerate() {
            let (base, shape) = if let Some(ref_path) = &member.ref_path {
                let ref_name = ref_path
                    .trim_start_matches("#/components/schemas/")
                    .trim_start_matches("#/schemas/");
                let ty = self.schema_to_rust_type(member, schemas);
                (
                    rename_std_type_conflict(&crate::to_pascal_case(ref_name)),
                    VariantShape::Newtype(ty),
                )
            } else if let Some(properties) = member.properties.as_ref().filter(|p| !p.is_empty()) {
                let required: std::collections::HashSet<&String> = member.required.iter().collect();
                let mut seen_fields: HashSet<String> = HashSet::new();
                let mut fields = Vec::new();
                for (prop_name, prop_schema) in properties {
                    let field_name = field_ident(prop_name);
                    if !seen_fields.insert(field_name.clone()) {
                        continue;
                    }
                    fields.push(self.field_of(
                        &field_name,
                        prop_name,
                        prop_schema,
                        required.contains(prop_name),
                        schemas,
                        recursive_types,
                    ));
                }
                let base = member
                    .title
                    .as_deref()
                    .map(|title| {
                        // A title is prose ("multiple queries"): one word per
                        // alphanumeric run, each capitalised.
                        title
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|w| !w.is_empty())
                            .map(|w| {
                                let mut chars = w.chars();
                                chars.next().map_or_else(String::new, |first| {
                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                })
                            })
                            .collect::<String>()
                    })
                    .filter(|n| n.chars().next().is_some_and(char::is_alphabetic))
                    .unwrap_or_else(|| format!("Variant{index}"));
                (base, VariantShape::Struct(fields))
            } else {
                let ty = self.schema_to_rust_type(member, schemas);
                let base = match member.schema_type.as_deref() {
                    Some("string") => "Text".to_string(),
                    Some("integer") => "Integer".to_string(),
                    Some("number") => "Number".to_string(),
                    Some("boolean") => "Boolean".to_string(),
                    Some("array") => "List".to_string(),
                    _ => format!("Variant{index}"),
                };
                (base, VariantShape::Newtype(ty))
            };
            let mut name = base.clone();
            let mut n = 2;
            while !taken.insert(name.clone()) {
                name = format!("{base}{n}");
                n += 1;
            }
            variants.push(Variant {
                doc: member
                    .description
                    .clone()
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                name,
                shape,
            });
        }
        let mut def = TypeDef::union(type_name, doc, variants);
        if let Some(d) = &schema.description {
            def.doc.push(String::new());
            def.doc.extend(d.lines().map(str::to_string));
        }
        Some(def)
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
        schemas: &SchemaIndex<'_>,
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

    /// Convert an `OpenAPI` schema to a Rust type string.
    fn schema_to_rust_type(
        &self,
        schema: &crate::spec::Schema,
        schemas: &SchemaIndex<'_>,
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
                let pascal_name = rename_std_type_conflict(&crate::to_pascal_case(ref_name));
                if target.ref_path.is_none() && !self.typed_unions.contains(&pascal_name) {
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
                    format!("Vec<{item_type}>")
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
    #[allow(clippy::too_many_lines)] // One template per generated unit; splitting it is a refactor, not part of this lint pass.
    #[allow(clippy::unused_self)] // Kept on `self` so it sits with the other generator methods.
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
            Some(ResponseType::JsonLines(item)) => {
                format!("super::shared::JsonLinesStream<{item}>")
            }
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
        writeln!(
            out,
            "/// modifications, then sends it via `send_async().await`."
        )?;
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
                if raw.required {
                    ""
                } else {
                    " (`None` sends none)"
                }
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
            "/// let response = {fn_prefix}_request(client.clone(), &args, Some(|b: &mut PreparedRequestBuilder| {{"
        )?;
        writeln!(out, "///     b.header(\"X-Custom-Header\", \"value\");")?;
        writeln!(out, "/// }})).await?;")?;
        writeln!(out, "/// ```")?;
        // `args` is only read when the endpoint has path params, query params,
        // or a request body. Name it `_args` otherwise so the (always-emitted)
        // parameter doesn't trip `unused_variables`.
        let args_uses =
            !ep.path_params.is_empty() || !ep.query_params.is_empty() || ep.request_type.is_some();
        let args_binding = if args_uses { "args" } else { "_args" };
        writeln!(out, "pub async fn {fn_prefix}_request<F>(")?;
        // `impl Into<ApiClient>`: a bare `DynNetClient` still works (with the
        // default `BodyLimits`), and a client that configures its own limits
        // passes an `ApiClient` carrying them down to the shared senders.
        writeln!(out, "    client: impl Into<super::shared::ApiClient>,")?;
        writeln!(out, "    {args_binding}: &{args_name},")?;
        // WHY: The base URL (e.g. "http://localhost/v1.53") is configurable so
        // callers can point at remote Docker daemons, not just Unix-socket-local.
        // For providers whose spec declares an explicit baseUrl, the caller
        // should pass that value; for Docker the caller derives it from
        // DockerClient::base_url().
        writeln!(out, "    base_url: &str,")?;
        if let Some(raw) = &ep.raw_request_body {
            if raw.required {
                writeln!(
                    out,
                    "    body: foundation_netio::shared::http::SendSafeBody,"
                )?;
            } else {
                writeln!(
                    out,
                    "    body: Option<foundation_netio::shared::http::SendSafeBody>,"
                )?;
            }
        }
        writeln!(out, "    builder_mod: Option<F>,")?;
        writeln!(
            out,
            ") -> Result<ApiResponse<{return_type}>, super::shared::ApiError>"
        )?;
        writeln!(out, "where")?;
        writeln!(out, "    F: FnOnce(&mut PreparedRequestBuilder),")?;
        writeln!(out, "{{")?;

        // Build URL: base_url is the caller-supplied scheme+host+version prefix
        // (e.g. "http://localhost/v1.53"); we format the path separately then
        // join — avoids nested format!() indentation issues.
        //
        // A path with no parameters is a plain string literal: emitting
        // `format!("/x")` there would trip `clippy::useless_format`. A path
        // with parameters is one single-line `format!` call, so rustfmt adds
        // no trailing comma on it (`clippy::unnecessary_trailing_comma`).
        let (escaped_path, params_in_url_order) = escape_url_for_format(&ep.path, &ep.path_params);
        if params_in_url_order.is_empty() {
            // `escaped_path` is escaped for `format!`; a literal needs the raw
            // braces back. `{:?}` then quotes and escapes it as a Rust string.
            let unescaped = escaped_path.replace("{{", "{").replace("}}", "}");
            writeln!(out, "    let path = {unescaped:?};")?;
        } else {
            let args = params_in_url_order
                .iter()
                .map(|param_name| format!("args.{}", escape_rust_keyword(param_name)))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(out, "    let path = format!(\"{escaped_path}\", {args});")?;
        }
        writeln!(
            out,
            "    let endpoint_url = format!(\"{{base_url}}{{path}}\");"
        )?;
        writeln!(out)?;

        // Build request via the cross-platform PreparedRequestBuilder.
        let method_lower = ep.method.to_lowercase();
        writeln!(
            out,
            "    let mut builder = PreparedRequestBuilder::{method_lower}(&endpoint_url)"
        )?;
        writeln!(
            out,
            "        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;"
        )?;
        writeln!(out)?;

        // Structured query params from args (None values are skipped).
        for qp in &ep.query_params {
            let safe_qp = escape_rust_keyword(&to_snake_case(&sanitize_identifier(qp)));
            if ep.boolean_query_params.contains(qp) {
                writeln!(
                    out,
                    "    builder = builder.query(\"{qp}\", args.{safe_qp}.map(|b| if b {{ \"true\" }} else {{ \"false\" }}));"
                )?;
            } else if ep.array_query_params.contains(qp) {
                writeln!(out, "    for value in &args.{safe_qp} {{")?;
                writeln!(
                    out,
                    "        builder = builder.query(\"{qp}\", Some(value.as_str()));"
                )?;
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
        writeln!(
            out,
            "    super::shared::{sender}(client.into(), builder.build()).await"
        )?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        Ok(())
    }

    /// Generate shared module for cross-group types.
    ///
    /// WHY: Types used by multiple groups (`shared_resources`) must live in one place so
    /// group modules can import them via `super::shared::`. The OLD behaviour was to emit
    /// a `HashMap<String, Value>` stub for every shared type regardless of the actual
    /// `OpenAPI` schema. The NEW behaviour resolves each type's schema and generates a
    /// properly typed struct with real fields, only falling back to `HashMap` when the
    /// schema truly cannot be resolved.
    #[allow(clippy::too_many_lines)] // One template per generated unit; splitting it is a refactor, not part of this lint pass.
    fn generate_shared_module(
        &self,
        analysis: &super::analyzer::AnalysisResult,
    ) -> Result<ModuleFile, GenError> {
        let index = SchemaIndex::new(&analysis.schemas);
        let mut module = ModuleFile::new("shared/mod.rs");

        // Build dependency graph across ALL schemas (including shared ones) so we
        // detect mutually recursive types that need Box wrapping.
        let type_deps = index.type_deps();
        let recursive_types = find_recursive_types(type_deps);

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
        // The common re-export header is only written in the single-crate
        // layouts. In split mode the shared types move to their own leaf crate
        // (`foundation_cloudflare_types`) which must NOT depend on
        // `foundation_deployment` (that would cycle: foundation_deployment →
        // foundation_db → foundation_cloudflare_types). Each group crate (and
        // the umbrella shim) re-declares the `shared` module with the common
        // re-exports, so the group modules still see `super::shared::*`.
        if self.groups_dir.is_none() {
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
        }
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
                if let Some(schema) = index.resolve(&type_name) {
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
            let mut extras: Vec<String> =
                seen.into_iter().filter(|t| !original.contains(t)).collect();
            extras.sort();
            ordered.extend(extras);
            ordered
        };

        let mut emitted_shared: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for type_name in &shared_types_to_emit {
            // Skip types that aren't valid identifiers (generics like Vec<...>, paths with ::, etc.)
            if type_name.contains('<') || type_name.contains('>') || type_name.contains("::") {
                continue;
            }
            // Skip types starting with non-alphabetic characters (except _)
            if !type_name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
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
            if let Some(schema) = index.resolve(type_name) {
                if schema.schema_type.as_deref() == Some("array") {
                    continue;
                }
            }

            // Try to resolve the schema for this shared type.
            // Schemas are keyed by original spec names (usually snake_case);
            // shared_resources stores PascalCase names. resolve_schema_key
            // tries multiple naming conventions.
            let def = match index.resolve(type_name) {
                Some(schema) => {
                    self.type_def_from_schema(&safe_name, schema, &index, &recursive_types)
                }
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
    #[allow(clippy::unused_self)] // Kept on `self` so it sits with the other generator methods.
    fn generate_provider_mod(
        &self,
        provider: &str,
        groups: &[ApiGroup],
        _shared_resources: &[String],
        generated_dir: &Path,
    ) -> Result<(), GenError> {
        let feature_name = provider.replace(['-', '/'], "_");

        let mut out = String::new();
        writeln!(out, "//! Auto-generated module for {provider}.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin genapi generate`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{feature_name}\")]")?;
        writeln!(
            out,
            "#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::too_many_lines, clippy::struct_excessive_bools)]"
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
