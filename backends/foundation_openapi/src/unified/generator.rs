//! Unified code generator for OpenAPI specs.
//!
//! WHY: Generates cohesive per-endpoint units (types + clients + provider impl).
//!
//! WHAT: For each endpoint in a group, generates all related code together.
//!
//! HOW: String templates for each endpoint unit, grouped into modules.

use crate::{
    EndpointInfo,
    to_pascal_case, to_snake_case, sanitize_identifier,
};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::fs;
use toml::Value;

use super::analyzer::ApiGroup;

/// Rust keywords that must be escaped when used as identifiers.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
    "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "abstract", "become", "box", "do", "final", "macro", "override", "priv",
    "typeof", "unsized", "virtual", "yield", "try", "union", "raw",
];

/// Escape a Rust keyword by prefixing with `r#`.
fn escape_rust_keyword(ident: &str) -> String {
    if RUST_KEYWORDS.contains(&ident) {
        format!("r#{}", ident)
    } else {
        ident.to_string()
    }
}

/// Transform an OpenAPI path into a Rust format! string.
/// Converts `{param}` placeholders to `{}` only for params in path_params.
/// Other braces are escaped as literal braces.
fn escape_url_for_format(path: &str, path_params: &[String]) -> String {
    let mut result = String::new();
    let mut chars = path.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                // Check if this is a path parameter like {param}
                let mut param_content = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '}' {
                        chars.next(); // consume the closing brace
                        // Check if this param is in our path_params list
                        if path_params.iter().any(|p| {
                            let sanitized = sanitize_identifier(p);
                            sanitized == param_content || p == &param_content
                        }) {
                            // This is a known path parameter - replace with {}
                            result.push_str("{}");
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

    result
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
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
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
fn update_cargo_toml(provider: &str, groups: &[ApiGroup], cargo_toml_path: &Path) -> Result<(), std::io::Error> {
    let content = fs::read_to_string(cargo_toml_path)?;

    // Parse TOML into a mutable Value
    let mut doc: Value = toml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let provider_feature = provider.replace('-', "_");

    // Get or create features table
    let features = doc
        .get_mut("features")
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing [features] section"))?;

    // Remove all existing feature flags for this provider (both provider-level and group-level)
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
        let feature_name = format!("{}_{}", provider_feature, safe_name);
        features.insert(feature_name.clone(), Value::Array(vec![]));
        group_features.push(feature_name);
    }

    // Update provider-level feature to enable all group features
    // e.g., cloudflare = ["cloudflare_assets", "cloudflare_bulk", ...]
    let provider_feature_array: Value = Value::Array(
        group_features.iter().map(|f| Value::String(f.clone())).collect()
    );
    features.insert(provider_feature.clone(), provider_feature_array);

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
        Self {
            output_dir,
        }
    }

    /// Generate all artifacts for a provider as cohesive per-endpoint units.
    pub fn generate(&self, provider: &str, spec_content: &str, options: &super::analyzer::AnalysisOptions) -> Result<(), GenError> {
        use super::analyzer::analyze_spec;

        // Analyze spec
        let analysis = analyze_spec(spec_content, provider, options)
            .map_err(|e| GenError::AnalysisFailed(e.to_string()))?;

        let provider_output_dir = self.output_dir.join(provider);
        fs::create_dir_all(&provider_output_dir)?;

        // Generate shared/ module if there are shared resources
        if !analysis.shared_resources.is_empty() {
            self.generate_shared_module(&analysis, &provider_output_dir)?;
        }

        // Generate one module per group
        for group in &analysis.groups {
            self.generate_group_module(provider, group, &provider_output_dir)?;
        }

        // Generate provider mod.rs with feature guards
        self.generate_provider_mod(provider, &analysis.groups)?;

        // Update Cargo.toml with missing feature flags
        // output_dir is typically "backends/foundation_deployment/src/providers"
        // We need to go up 2 levels to reach the crate root where Cargo.toml is:
        // - ancestors[0] = backends/foundation_deployment/src/providers
        // - ancestors[1] = backends/foundation_deployment/src
        // - ancestors[2] = backends/foundation_deployment (crate root with Cargo.toml)
        let cargo_toml_path = self.output_dir
            .ancestors()
            .nth(2)
            .map(|p| p.join("Cargo.toml"))
            .unwrap_or_else(|| PathBuf::from("backends/foundation_deployment/Cargo.toml"));

        if cargo_toml_path.exists() {
            update_cargo_toml(provider, &analysis.groups, &cargo_toml_path)
                .map_err(|e| GenError::WriteFile {
                    path: cargo_toml_path.display().to_string(),
                    source: e,
                })?;
        }

        Ok(())
    }

    /// Generate a single group module with per-endpoint cohesive units.
    fn generate_group_module(
        &self,
        provider: &str,
        group: &ApiGroup,
        output_dir: &Path,
    ) -> Result<(), GenError> {
        // Sanitize group name for use as directory/file name
        let safe_name = sanitize_group_name(&group.name);
        let group_dir = output_dir.join(&safe_name);
        fs::create_dir_all(&group_dir)?;

        let mut out = String::new();

        // File header
        writeln!(out, "//! Auto-generated API module for {} {}.", provider, group.name)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_api`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Feature flag: `{}_{} `", provider.replace('-', "_"), safe_name)?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}_{}\")]", provider.replace('-', "_"), safe_name)?;
        writeln!(out)?;

        // Common imports
        writeln!(out, "use foundation_core::valtron::{{execute, StreamIterator, TaskIterator}};")?;
        writeln!(out, "use foundation_core::wire::simple_http::client::{{ClientRequestBuilder, SimpleHttpClient}};")?;
        writeln!(out, "use serde::{{Deserialize, Serialize}};")?;
        writeln!(out, "use foundation_macros::JsonHash;")?;
        writeln!(out)?;

        // Track types we've already generated (for shared types)
        let mut generated_types: HashSet<String> = HashSet::new();

        // Collect all endpoint units for this group
        // We'll generate: types, then client functions, then provider impl
        // But organized per-endpoint for readability

        // First pass: collect all unique types
        let mut all_types: BTreeMap<String, &EndpointInfo> = BTreeMap::new();
        for ep in &group.endpoints {
            if let Some(rt) = &ep.response_type {
                let type_name = rt.as_rust_type().to_string();
                // Skip invalid types
                if type_name == "()" || type_name == "serde_json::Value" {
                    continue;
                }
                // Skip generic types like Vec<...>, Option<...>, etc.
                if type_name.contains('<') || type_name.contains('>') || type_name.contains("::") {
                    continue;
                }
                // Skip primitive collection types
                if ["Vec", "Option", "HashMap", "BTreeMap", "Box", "Rc", "Arc", "Cow"].contains(&type_name.as_str()) {
                    continue;
                }
                all_types.entry(type_name).or_insert(ep);
            }
            if let Some(rt) = &ep.request_type {
                // Skip generic types for request types too
                if !rt.contains('<') && !rt.contains('>') && !rt.contains("::") {
                    all_types.entry(rt.clone()).or_insert(ep);
                }
            }
        }

        // Generate type stubs for forward references
        // (Types will be fully generated per-endpoint, but we need declarations first)
        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// TYPE DECLARATIONS")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        // Generate types - simplified for now (full types would come from schema analysis)
        for (type_name, _ep) in &all_types {
            if generated_types.contains(type_name) {
                continue;
            }

            // Generate a placeholder struct - in full impl, this would come from schema
            writeln!(out, "/// {} response type.", type_name)?;
            writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]")?;
            writeln!(out, "pub struct {} {{", type_name)?;
            writeln!(out, "    /// Raw JSON value - full schema generated from OpenAPI")?;
            writeln!(out, "    #[serde(flatten)]")?;
            writeln!(out, "    pub data: std::collections::HashMap<String, serde_json::Value>,")?;
            writeln!(out, "}}")?;
            writeln!(out)?;

            generated_types.insert(type_name.clone());
        }

        // Generate Args types per endpoint
        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// ARGS TYPES (per-endpoint)")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        for ep in &group.endpoints {
            let args_name = format!("{}Args", to_pascal_case(&sanitize_identifier(&ep.operation_id)));

            // Check if this endpoint has params
            let has_params = !ep.path_params.is_empty()
                || !ep.query_params.is_empty()
                || ep.request_type.is_some();

            if has_params {
                writeln!(out, "/// Arguments for [`{}_builder`].", ep.operation_id)?;
                writeln!(out, "#[derive(Debug, Clone, Serialize, JsonHash)]")?;
                writeln!(out, "pub struct {} {{", args_name)?;

                for param in &ep.path_params {
                    let param_name = escape_rust_keyword(&sanitize_identifier(param));
                    writeln!(out, "    /// Path parameter: {}", param)?;
                    writeln!(out, "    pub {}: String,", param_name)?;
                }
                for param in &ep.query_params {
                    let param_name = escape_rust_keyword(&sanitize_identifier(param));
                    writeln!(out, "    /// Query parameter: {}", param)?;
                    writeln!(out, "    pub {}: Option<String>,", param_name)?;
                }
                if let Some(rt) = &ep.request_type {
                    writeln!(out, "    /// Request body.")?;
                    writeln!(out, "    pub body: {},", rt)?;
                }

                writeln!(out, "}}")?;
                writeln!(out)?;
            }
        }

        // Generate client functions per endpoint
        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// CLIENT FUNCTIONS (per-endpoint)")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        for ep in &group.endpoints {
            self.generate_endpoint_client_functions(&mut out, ep, group)?;
        }

        // DISABLED: ProviderClient impl wrapper methods - causing more complexity than value
        // These wrapper methods were generating state-aware API calls but added significant
        // complexity. Users can use the builder functions directly instead.
        //
        // writeln!(out, "// =============================================================================")?;
        // writeln!(out, "// PROVIDERCLIENT IMPL (wrapper methods)")?;
        // writeln!(out, "// =============================================================================")?;
        // writeln!(out)?;
        // self.generate_provider_client_impl(&mut out, provider, group)?;

        // Write output
        let output_path = group_dir.join("mod.rs");
        fs::write(&output_path, out)?;

        // Format
        let _ = std::process::Command::new("rustfmt").arg(&output_path).output();

        Ok(())
    }

    /// Generate client functions for a single endpoint.
    fn generate_endpoint_client_functions(
        &self,
        out: &mut String,
        ep: &EndpointInfo,
        _group: &ApiGroup,
    ) -> Result<(), GenError> {
        let fn_prefix = to_snake_case(&sanitize_identifier(&ep.operation_id));
        let return_type = ep.response_type
            .as_ref()
            .map(|rt| rt.as_rust_type().to_string())
            .unwrap_or_else(|| "()".to_string());

        let has_params = !ep.path_params.is_empty()
            || !ep.query_params.is_empty()
            || ep.request_type.is_some();

        let args_name = format!("{}Args", to_pascal_case(&sanitize_identifier(&ep.operation_id)));

        // Header comment for this endpoint
        writeln!(out, "// -----------------------------------------------------------------------------")?;
        writeln!(out, "// {} {}", ep.method, ep.path)?;
        writeln!(out, "// -----------------------------------------------------------------------------")?;
        writeln!(out)?;

        // Single merged function: builds request, applies optional modifications, returns task
        writeln!(out, "/// {} {}.", ep.method, ep.path)?;
        writeln!(out, "///")?;
        writeln!(out, "/// Takes client and args, builds the request, optionally applies modifications,")?;
        writeln!(out, "/// and returns a `TaskIterator` for execution.")?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Arguments")?;
        writeln!(out, "///")?;
        writeln!(out, "/// * `client` - HTTP client for making the request")?;
        if has_params {
            writeln!(out, "/// * `args` - Request arguments (path params, query params, body)")?;
        }
        writeln!(out, "/// * `builder_mod` - Optional closure to modify the request builder (e.g., add headers)")?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Example")?;
        writeln!(out, "///")?;
        writeln!(out, "/// ```ignore")?;
        writeln!(out, "/// let task = {}_request(&client, &args, Some(|b| {{", fn_prefix)?;
        writeln!(out, "///     b.header(\"X-Custom-Header\", \"value\")")?;
        writeln!(out, "/// }}))?;")?;
        writeln!(out, "/// ```")?;
        writeln!(out)?;
        writeln!(out, "#[inline]")?;
        writeln!(out, "pub fn {}_request<R, F>(", fn_prefix)?;
        writeln!(out, "    client: &SimpleHttpClient<R>,")?;
        if has_params {
            writeln!(out, "    args: &{},", args_name)?;
        }
        writeln!(out, "    builder_mod: Option<F>,")?;
        writeln!(out, ") -> Result<impl TaskIterator<Ready = Result<ApiResponse<{}>, super::shared::ApiError>, Pending = super::shared::ApiPending, Spawner = super::shared::BoxedSendExecutionAction> + Send + 'static, super::shared::ApiError> {{", return_type)?;
        writeln!(out, "where")?;
        writeln!(out, "    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,")?;
        writeln!(out, "    F: FnOnce(&mut ClientRequestBuilder<R>),")?;
        writeln!(out, "{{")?;

        // Build URL
        let escaped_path = escape_url_for_format(&ep.path, &ep.path_params);
        let escaped_base = escape_url_for_format(ep.base_url.as_deref().unwrap_or("https://api.example.com"), &[]);
        writeln!(out, "    let endpoint_url = format!(")?;
        writeln!(out, "        \"{}{}\",", escaped_base, escaped_path)?;
        for param in &ep.path_params {
            let param_name = sanitize_identifier(param);
            if has_params {
                writeln!(out, "        args.{},", param_name)?;
            } else {
                writeln!(out, "        /* {} */ \"placeholder\",", param)?;
            }
        }
        writeln!(out, "    );")?;
        writeln!(out)?;

        // Build request
        let method_lower = ep.method.to_lowercase();
        writeln!(out, "    let mut builder = client.{}(&endpoint_url)", method_lower)?;
        writeln!(out, "        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;")?;
        writeln!(out)?;

        // Add body if present
        if ep.request_type.is_some() && has_params {
            writeln!(out, "    builder = builder.body_json(&args.body)")?;
            writeln!(out, "        .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;")?;
            writeln!(out)?;
        }

        // Apply user modifications
        writeln!(out, "    if let Some(f) = builder_mod {{")?;
        writeln!(out, "        f(&mut builder);")?;
        writeln!(out, "    }}")?;
        writeln!(out)?;

        // Build and return task
        writeln!(out, "    Ok(")?;
        writeln!(out, "        builder")?;
        writeln!(out, "            .build_send_request()")?;
        writeln!(out, "            .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?")?;
        writeln!(out, "            .map_ready(|intro| match intro {{")?;
        writeln!(out, "                super::shared::RequestIntro::Success {{ stream, intro, headers, .. }} => {{")?;
        writeln!(out, "                    let status: usize = intro.0.into();")?;
        writeln!(out, "                    if status < 200 || status >= 300 {{")?;
        writeln!(out, "                        return Err(super::shared::ApiError::HttpStatus {{ code: status as u16, headers: headers.clone(), body: None }});")?;
        writeln!(out, "                    }}")?;
        if return_type == "()" {
            writeln!(out, "                    Ok(ApiResponse {{ status: status as u16, headers: headers.clone(), body: () }})")?;
        } else {
            writeln!(out, "                    let body = foundation_core::wire::simple_http::body_reader::collect_string(stream);")?;
            writeln!(out, "                    let parsed: {} = serde_json::from_str(&body).map_err(|e| super::shared::ApiError::ParseFailed(e.to_string()))?;", return_type)?;
            writeln!(out, "                    Ok(ApiResponse {{ status: status as u16, headers: headers.clone(), body: parsed }})")?;
        }
        writeln!(out, "                }}")?;
        writeln!(out, "                super::shared::RequestIntro::Failed(e) => Err(super::shared::ApiError::RequestSendFailed(e.to_string())),")?;
        writeln!(out, "            }})")?;
        writeln!(out, "            .map_pending(|_| super::shared::ApiPending::Sending)")?;
        writeln!(out, "    )")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        // DISABLED: Execute function - users can call execute() directly on the task
        // writeln!(out, "/// {} {} - execute function.", ep.method, ep.path)?;
        // writeln!(out, "///")?;
        // writeln!(out, "/// Takes a `ClientRequestBuilder` and executes it.")?;
        // writeln!(out)?;
        // writeln!(out, "#[inline]")?;
        // writeln!(out, "pub fn {}_execute<R>(", fn_prefix)?;
        // writeln!(out, "    builder: ClientRequestBuilder<R>,")?;
        // writeln!(out, ") -> Result<impl StreamIterator<D = Result<ApiResponse<{}>, super::shared::ApiError>, P = super::shared::ApiPending> + Send + 'static, super::shared::ApiError> {{", return_type)?;
        // writeln!(out, "where R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,")?;
        // writeln!(out, "    let task = {}_task(builder)?;", fn_prefix)?;
        // writeln!(out, "    execute(task, None).map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))")?;
        // writeln!(out, "}}")?;
        // writeln!(out)?;


        // DISABLED: Convenience function - users can use builder + task directly
        // if has_params {
        //     writeln!(out, "/// {} {} - convenience function.", ep.method, ep.path)?;
        //     writeln!(out, "///")?;
        //     writeln!(out, "/// Combines builder and execute in one call.")?;
        //     writeln!(out)?;
        //     writeln!(out, "#[inline]")?;
        //     writeln!(out, "pub fn {}<R>(", fn_prefix)?;
        //     writeln!(out, "    client: &SimpleHttpClient<R>,")?;
        //     writeln!(out, "    args: &{},", args_name)?;
        //     writeln!(out, ") -> Result<impl StreamIterator<D = Result<ApiResponse<{}>, super::shared::ApiError>, P = super::shared::ApiPending> + Send + 'static, super::shared::ApiError> {{", return_type)?;
        //     writeln!(out, "where R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,")?;
        //     writeln!(out, "    let builder = {}_builder(client, args)?;", fn_prefix)?;
        //     writeln!(out, "    {}_execute(builder)", fn_prefix)?;
        //     writeln!(out, "}}")?;
        //     writeln!(out)?;
        // }

        Ok(())
    }

    // DISABLED: ProviderClient impl wrapper generation - no longer used
    // /// Generate ProviderClient impl block with wrapper methods.
    // fn generate_provider_client_impl(
    //     &self,
    //     out: &mut String,
    //     provider: &str,
    //     group: &ApiGroup,
    // ) -> Result<(), GenError> {
    //     writeln!(out, "/// ProviderClient extension methods for {} {}.", provider, group.name)?;
    //     writeln!(out, "///")?;
    //     writeln!(out, "/// These wrapper methods provide state-aware API access.")?;
    //     writeln!(out)?;
    //     writeln!(out, "impl<S, R> crate::ProviderClient<S, R>")?;
    //     writeln!(out, "where")?;
    //     writeln!(out, "    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,")?;
    //     writeln!(out, "    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,")?;
    //     writeln!(out, "{{")?;
    //
    //     for ep in &group.endpoints {
    //         let fn_prefix = to_snake_case(&sanitize_identifier(&ep.operation_id));
    //         let wrapper_fn = format!("{}_{}_{}", provider.replace('-', "_"), group.name, fn_prefix);
    //         let return_type = ep.response_type
    //             .as_ref()
    //             .map(|rt| rt.as_rust_type().to_string())
    //             .unwrap_or_else(|| "()".to_string());
    //
    //         let has_params = !ep.path_params.is_empty()
    //             || !ep.query_params.is_empty()
    //             || ep.request_type.is_some();
    //
    //         let args_name = format!("{}Args", to_pascal_case(&sanitize_identifier(&ep.operation_id)));
    //
    //         writeln!(out, "    /// {} {}.", ep.method, ep.path)?;
    //         writeln!(out, "    ///")?;
    //         writeln!(out, "    /// Wrapper method with automatic state tracking.")?;
    //         writeln!(out, "    pub fn {}(&self, ", wrapper_fn)?;
    //         if has_params {
    //             writeln!(out, "        args: &{},", args_name)?;
    //         }
    //         writeln!(out, "    ) -> Result<impl StreamIterator<D = Result<ApiResponse<{}>, super::shared::ApiError>, P = super::shared::ApiPending> + Send + 'static, super::shared::ApiError> {{", return_type)?;
    //         writeln!(out, "        let builder = {}_builder(&self.http_client", fn_prefix)?;
    //         if has_params {
    //             writeln!(out, "            , args")?;
    //         }
    //         writeln!(out, "        )?;")?;
    //         writeln!(out, "        {}_execute(builder)", fn_prefix)?;
    //         writeln!(out, "    }}")?;
    //         writeln!(out)?;
    //     }
    //
    //     writeln!(out, "}}")?;
    //     writeln!(out)?;
    //
    //     Ok(())
    // }

    /// Generate shared module for cross-group types.
    fn generate_shared_module(
        &self,
        analysis: &super::analyzer::AnalysisResult,
        output_dir: &Path,
    ) -> Result<(), GenError> {
        let shared_dir = output_dir.join("shared");
        fs::create_dir_all(&shared_dir)?;

        let mut out = String::new();
        writeln!(out, "//! Shared types for {} (used by multiple groups).", analysis.provider)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_api`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out)?;

        // Imports
        writeln!(out, "use foundation_core::wire::simple_http::SimpleHeaders;")?;
        writeln!(out, "use serde::{{Deserialize, Serialize}};")?;
        writeln!(out)?;

        // Re-export types from foundation_core for convenience
        writeln!(out, "// Re-export types from foundation_core for convenience")?;
        writeln!(out, "pub use foundation_core::valtron::{{BoxedSendExecutionAction, RequestIntro}};")?;
        writeln!(out)?;

        // Error types
        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// ERROR TYPES")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        writeln!(out, "/// Provider-agnostic error type for API operations.")?;
        writeln!(out, "#[derive(Debug)]")?;
        writeln!(out, "pub enum ApiError {{")?;
        writeln!(out, "    RequestBuildFailed(String),")?;
        writeln!(out, "    RequestSendFailed(String),")?;
        writeln!(out, "    HttpStatus {{")?;
        writeln!(out, "        code: u16,")?;
        writeln!(out, "        headers: SimpleHeaders,")?;
        writeln!(out, "        body: Option<String>,")?;
        writeln!(out, "    }},")?;
        writeln!(out, "    ParseFailed(String),")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        writeln!(out, "impl std::fmt::Display for ApiError {{")?;
        writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")?;
        writeln!(out, "        match self {{")?;
        writeln!(out, "            ApiError::RequestBuildFailed(e) => write!(f, \"request build failed: {{}}\", e),")?;
        writeln!(out, "            ApiError::RequestSendFailed(e) => write!(f, \"request send failed: {{}}\", e),")?;
        writeln!(out, "            ApiError::HttpStatus {{ code, body, .. }} => {{")?;
        writeln!(out, "                write!(f, \"HTTP status {{}}\", code)?;")?;
        writeln!(out, "                if let Some(b) = body {{")?;
        writeln!(out, "                    write!(f, \": {{}}\", b)?;")?;
        writeln!(out, "                }}")?;
        writeln!(out, "                Ok(())")?;
        writeln!(out, "            }}")?;
        writeln!(out, "            ApiError::ParseFailed(e) => write!(f, \"parse failed: {{}}\", e),")?;
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        writeln!(out, "impl std::error::Error for ApiError {{}}")?;
        writeln!(out)?;

        writeln!(out, "/// Progress states for API operations.")?;
        writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]")?;
        writeln!(out, "pub enum ApiPending {{")?;
        writeln!(out, "    Building,")?;
        writeln!(out, "    Sending,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        writeln!(out, "/// Generic API response with status, headers, and parsed body.")?;
        writeln!(out, "#[derive(Debug, Clone)]")?;
        writeln!(out, "pub struct ApiResponse<T> {{")?;
        writeln!(out, "    pub status: u16,")?;
        writeln!(out, "    pub headers: SimpleHeaders,")?;
        writeln!(out, "    pub body: T,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        writeln!(out, "// =============================================================================")?;
        writeln!(out, "// SHARED RESOURCE TYPES")?;
        writeln!(out, "// =============================================================================")?;
        writeln!(out)?;

        for type_name in &analysis.shared_resources {
            // Skip types that aren't valid identifiers (generics like Vec<...>, paths with ::, etc.)
            // Check for generic type patterns - anything with < > or :: is not a simple identifier
            if type_name.contains('<') || type_name.contains('>') || type_name.contains("::") {
                continue;
            }
            // Skip types starting with non-alphabetic characters (except _)
            if !type_name.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
                continue;
            }
            // Skip array-like types (Vec, Option, etc. used as raw type names without generics)
            if ["Vec", "Option", "HashMap", "BTreeMap", "Box", "Rc", "Arc", "Cow"].contains(&type_name.as_str()) {
                continue;
            }

            writeln!(out, "/// Shared type: {}.", type_name)?;
            writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]")?;
            writeln!(out, "pub struct {} {{", type_name)?;
            writeln!(out, "    #[serde(flatten)]")?;
            writeln!(out, "    pub data: std::collections::HashMap<String, serde_json::Value>,")?;
            writeln!(out, "}}")?;
            writeln!(out)?;
        }

        fs::write(shared_dir.join("mod.rs"), out)?;
        Ok(())
    }

    /// Generate provider mod.rs with feature guards.
    fn generate_provider_mod(
        &self,
        provider: &str,
        groups: &[ApiGroup],
    ) -> Result<(), GenError> {
        let feature_name = provider.replace('-', "_");

        let mut out = String::new();
        writeln!(out, "//! Auto-generated provider module for {}.", provider)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! Generated by `cargo run --bin ewe_platform gen_api`.")?;
        writeln!(out, "//! DO NOT EDIT MANUALLY.")?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}\")]", feature_name)?;
        writeln!(out)?;

        // Shared module - always compiled when provider is enabled (no feature flag)
        writeln!(out, "pub mod shared;")?;
        writeln!(out)?;

        // Group modules (use sanitized names)
        for group in groups {
            let safe_name = sanitize_group_name(&group.name);
            writeln!(out, "#[cfg(feature = \"{}_{}\")]", feature_name, safe_name)?;
            writeln!(out, "pub mod {};", safe_name)?;
        }

        let provider_dir = self.output_dir.join(provider);
        fs::create_dir_all(&provider_dir)?;
        fs::write(provider_dir.join("mod.rs"), out)?;

        Ok(())
    }
}
