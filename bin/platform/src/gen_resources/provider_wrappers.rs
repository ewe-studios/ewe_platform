//! WHY: Generates provider wrapper APIs with automatic state tracking.
//!
//! WHAT: Uses foundation_openapi ApiCatalog to get structured endpoint data
//! and generates provider wrapper structs that wrap _task() functions with
//! StoreStateIdentifierTask for automatic state persistence.
//!
//! HOW: For each provider:
//! 1. Use ApiCatalog to discover APIs and endpoints from OpenAPI specs
//! 2. Group endpoints by API (for GCP) or use single catalog (for single-spec providers)
//! 3. Generate Provider struct with methods per endpoint
//! 4. Each method wraps the task with StoreStateIdentifierTask

use foundation_openapi::{ApiCatalog, EndpointInfo, to_snake_case, to_pascal_case, to_sentence_case};
use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Sanitize special characters in operation IDs before converting to Rust identifiers
// ---------------------------------------------------------------------------

fn sanitize_operation_id(op_id: &str) -> String {
    op_id
        .replace('-', "_")
        .replace('.', "_")
        .replace('@', "_")
        .replace(':', "_")
        .replace('<', "_")
        .replace('>', "_")
        .replace('[', "_")
        .replace(']', "_")
        .replace('(', "_")
        .replace(')', "_")
        .replace('\'', "_")
        .replace(',', "_")
        .replace('~', "_")
        .replace('/', "_")
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Escape Rust keywords by appending `_rs`.
fn escape_keyword(name: &str) -> String {
    match name {
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
        | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
        | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
        | "use" | "where" | "while" | "async" | "await" | "dyn" | "override" => format!("{}_rs", name),
        _ => name.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, derive_more::Display)]
pub enum GenWrapperError {
    #[display("failed to read {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[display("catalog build failed: {_0}")]
    CatalogBuildFailed(String),

    #[display("fmt error: {_0}")]
    FmtError(std::fmt::Error),
}

impl std::error::Error for GenWrapperError {}

impl From<std::io::Error> for GenWrapperError {
    fn from(e: std::io::Error) -> Self {
        GenWrapperError::ReadFile {
            path: String::new(),
            source: e,
        }
    }
}

impl From<std::fmt::Error> for GenWrapperError {
    fn from(e: std::fmt::Error) -> Self {
        GenWrapperError::FmtError(e)
    }
}

// ---------------------------------------------------------------------------
// Intermediate representation
// ---------------------------------------------------------------------------

/// Represents an endpoint function to generate.
#[derive(Debug, Clone)]
struct EndpointFn {
    /// Function name without suffix (e.g., "folders_get_autokey_config")
    base_name: String,
    /// Args struct name (e.g., "CloudkmsFoldersGetAutokeyConfigArgs")
    args_type: String,
    /// Response type name (e.g., "AutokeyConfig")
    response_type: String,
    /// Is this a mutating operation (create/update/delete)?
    is_mutating: bool,
    /// Path parameter names (e.g., ["project", "region", "address"])
    path_params: Vec<String>,
    /// Query parameter names
    query_params: Vec<String>,
}

/// Represents a provider wrapper to generate.
#[derive(Debug, Clone)]
struct ProviderWrapper {
    /// Name of the wrapper (e.g., "CloudKmsProvider")
    name: String,
    /// API name (e.g., "cloudkms")
    api_name: String,
    /// Endpoints to wrap
    endpoints: Vec<EndpointFn>,
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

pub struct ProviderWrapperGenerator {
    artefacts_dir: PathBuf,
    output_dir: PathBuf,
}

impl ProviderWrapperGenerator {
    pub fn new(artefacts_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            artefacts_dir,
            output_dir,
        }
    }

    /// Generate wrappers for all providers.
    pub fn generate_all(&self) -> Result<(), GenWrapperError> {
        let providers = foundation_openapi::discover_providers(&self.artefacts_dir)
            .map_err(|e| GenWrapperError::CatalogBuildFailed(e.to_string()))?;
        eprintln!("DEBUG: Found providers: {:?}", providers);
        tracing::info!("Found {} providers: {:?}", providers.len(), providers);

        for provider in &providers {
            eprintln!("DEBUG: Generating for: {}", provider);
            tracing::info!("Generating for: {}", provider);
            self.generate_for_provider(provider)?;
        }

        Ok(())
    }

    /// Generate wrappers for a single provider using ApiCatalog.
    pub fn generate_for_provider(&self, provider: &str) -> Result<(), GenWrapperError> {
        let provider_dir = self.artefacts_dir.join(provider);

        if !provider_dir.exists() {
            return Ok(());
        }

        let provider_output_dir = self.output_dir.join(provider);
        let api_dir = provider_output_dir.join("api");

        // Create api directory if it doesn't exist
        fs::create_dir_all(&api_dir)?;

        // Build catalog from specs - skip failing APIs
        let catalog = match ApiCatalog::builder(provider)
            .from_provider_dir(&provider_dir)
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("    Skipping {}: {}", provider, e);
                return Ok(());
            }
        };

        tracing::info!("    Found {} APIs with {} total endpoints", catalog.apis.len(), catalog.total_endpoints());

        if catalog.total_endpoints() == 0 {
            tracing::warn!("    No endpoints found, skipping");
            return Ok(());
        }

        // Generate wrapper for each API
        for api in &catalog.apis {
            self.generate_for_api(&catalog.provider, api, &api_dir)?;
        }

        // Generate api/mod.rs
        self.generate_api_mod_rs(&catalog, &api_dir.join("mod.rs"))?;

        Ok(())
    }

    /// Generate wrapper for a single API.
    fn generate_for_api(
        &self,
        provider: &str,
        api: &foundation_openapi::ApiInfo,
        api_dir: &Path,
    ) -> Result<(), GenWrapperError> {
        let output_path = api_dir.join(format!("{}.rs", api.name));

        if api.endpoints.is_empty() {
            tracing::debug!("    No endpoints for {}, skipping", api.name);
            return Ok(());
        }

        // Convert endpoints to EndpointFn format - ALL endpoints, not just mutating
        let mut endpoints = Vec::new();
        for ep in &api.endpoints {
            // Use operation_type to determine if mutating
            let is_mutating = ep.operation_type.requires_state_tracking();

            let sanitized_id = sanitize_operation_id(&ep.operation_id);
            let base_name = to_snake_case(&sanitized_id);

            // Sanitize parameter names (replace dots and other special chars with underscores)
            let path_params: Vec<String> = ep.path_params.iter()
                .map(|p| sanitize_operation_id(p))
                .collect();
            let query_params: Vec<String> = ep.query_params.iter()
                .map(|p| sanitize_operation_id(p))
                .collect();

            endpoints.push(EndpointFn {
                base_name: base_name.clone(),
                args_type: to_pascal_case(&sanitized_id) + "Args",
                response_type: ep.response_type.as_ref().map(|rt| rt.as_rust_type()).unwrap_or("serde_json::Value").to_string(),
                is_mutating,
                path_params,
                query_params,
            });
        }

        if endpoints.is_empty() {
            tracing::debug!("    No endpoints for {}, writing empty module", api.name);
            // Write empty module with proper structure
            let provider_name = to_pascal_case(&api.name) + "Provider";
            let mut out = String::new();
            writeln!(out, "//! {} - State-aware {} API client.", provider_name, api.name)?;
            writeln!(out, "//!")?;
            writeln!(out, "//! No endpoints to wrap.")?;
            writeln!(out)?;
            writeln!(out, "#![cfg(feature = \"{}\")]", provider)?;
            writeln!(out)?;
            writeln!(out, "use crate::provider_client::{{ProviderClient, ProviderError}};")?;
            writeln!(out, "use foundation_core::wire::simple_http::client::SimpleHttpClient;")?;
            writeln!(out, "use std::sync::Arc;")?;
            writeln!(out)?;
            writeln!(out, "/// {} with automatic state tracking.", provider_name)?;
            writeln!(out, "#[derive(Clone)]")?;
            writeln!(out, "pub struct {}Provider<S>", to_pascal_case(&api.name))?;
            writeln!(out, "where")?;
            writeln!(out, "    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,")?;
            writeln!(out, "{{")?;
            writeln!(out, "    client: ProviderClient<S>,")?;
            writeln!(out, "    http_client: Arc<SimpleHttpClient>,")?;
            writeln!(out, "}}")?;
            writeln!(out)?;
            writeln!(out, "impl<S> {}Provider<S>", to_pascal_case(&api.name))?;
            writeln!(out, "where")?;
            writeln!(out, "    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,")?;
            writeln!(out, "{{")?;
            writeln!(out, "    /// Create new {}Provider.", to_pascal_case(&api.name))?;
            writeln!(out, "    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {{")?;
            writeln!(out, "        Self {{")?;
            writeln!(out, "            client,")?;
            writeln!(out, "            http_client: Arc::new(http_client),")?;
            writeln!(out, "        }}")?;
            writeln!(out, "    }}")?;
            writeln!(out)?;
            writeln!(out, "}}")?;
            fs::write(&output_path, out)?;
            return Ok(());
        }

        let wrapper = ProviderWrapper {
            name: to_pascal_case(&api.name) + "Provider",
            api_name: api.name.clone(),
            endpoints,
        };

        self.generate_wrapper(&wrapper, &output_path, provider)?;
        Ok(())
    }

    /// Generate a single provider wrapper file.
    fn generate_wrapper(
        &self,
        wrapper: &ProviderWrapper,
        output_path: &Path,
        provider: &str,
    ) -> Result<(), GenWrapperError> {
        tracing::debug!("    Generating {}", output_path.display());

        let mut out = String::new();

        // File header
        writeln!(out, "//! {} - State-aware {} API client.", wrapper.name, wrapper.api_name)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! WHY: Users need state-aware API clients that automatically track")?;
        writeln!(out, "//!      resource changes in the state store.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! WHAT: Provider wrapping ProviderClient<S> with methods for")?;
        writeln!(out, "//!       {} API endpoints that auto-store results.", wrapper.api_name)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! HOW: Each method wraps the task with StoreStateIdentifierTask")?;
        writeln!(out, "//!      for automatic state persistence on success.")?;
        writeln!(out)?;

        // Feature flag
        writeln!(out, "#![cfg(feature = \"{}\")]", provider)?;
        writeln!(out)?;

        // Imports - collect unique types first
        let mut response_types: BTreeMap<String, bool> = BTreeMap::new();
        let mut args_types: BTreeMap<String, bool> = BTreeMap::new();

        for endpoint in &wrapper.endpoints {
            response_types.insert(endpoint.response_type.clone(), true);
            args_types.insert(endpoint.args_type.clone(), true);
        }

        // Only generate imports for mutating endpoints if there are any
        // For single-API providers (api_name == provider), clients are in clients/mod.rs
        // For multi-API providers, clients are in clients/{api_name}.rs
        let clients_module = if wrapper.api_name == provider {
            "clients".to_string()
        } else {
            format!("clients::{}", wrapper.api_name)
        };

        if !wrapper.endpoints.is_empty() {
            writeln!(out, "use crate::providers::{}::{}::{{", provider, clients_module)?;
            for endpoint in &wrapper.endpoints {
                writeln!(
                    out,
                    "    {}_builder, {}_task,",
                    endpoint.base_name, endpoint.base_name
                )?;
            }
            writeln!(out, "}};")?;
        }
        writeln!(out, "use crate::providers::{}::clients::types::{{ApiError, ApiPending}};", provider)?;
        for response_type in response_types.keys() {
            if response_type != "()" && response_type != "serde_json::Value" {
                writeln!(out, "use crate::providers::{}::{}::{};", provider, clients_module, response_type)?;
            }
        }
        for args_type in args_types.keys() {
            writeln!(out, "use crate::providers::{}::{}::{};", provider, clients_module, args_type)?;
        }
        writeln!(out, "use crate::provider_client::{{ProviderClient, ProviderError}};")?;
        writeln!(out, "use foundation_core::valtron::{{execute, StreamIterator}};")?;
        writeln!(out, "use foundation_core::wire::simple_http::client::SimpleHttpClient;")?;
        writeln!(out, "use foundation_db::state::store_state_task::StoreStateIdentifierTask;")?;
        writeln!(out, "use std::sync::Arc;")?;
        writeln!(out)?;

        // Provider struct
        writeln!(out, "/// {} with automatic state tracking.", wrapper.name)?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Type Parameters")?;
        writeln!(out, "///")?;
        writeln!(out, "/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)")?;
        writeln!(out, "///")?;
        writeln!(out, "/// # Example")?;
        writeln!(out, "///")?;
        writeln!(out, "/// ```rust")?;
        writeln!(out, "/// let state_store = FileStateStore::new(\"/path\", \"my-project\", \"dev\");")?;
        writeln!(out, "/// let client = ProviderClient::new(\"my-project\", \"dev\", state_store);")?;
        writeln!(out, "/// let http_client = SimpleHttpClient::new(...);")?;
        writeln!(out, "/// let provider = {}::new(client, http_client);", wrapper.name)?;
        writeln!(out, "/// ```")?;
        writeln!(out, "#[derive(Clone)]")?;
        writeln!(out, "pub struct {}<S>", wrapper.name)?;
        writeln!(out, "where")?;
        writeln!(out, "    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,")?;
        writeln!(out, "{{")?;
        writeln!(out, "    client: ProviderClient<S>,")?;
        writeln!(out, "    http_client: Arc<SimpleHttpClient>,")?;
        writeln!(out, "}}")?;
        writeln!(out)?;

        // Implementation
        writeln!(out, "impl<S> {}<S>", wrapper.name)?;
        writeln!(out, "where")?;
        writeln!(out, "    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,")?;
        writeln!(out, "{{")?;
        writeln!(out, "    /// Create new {}.", wrapper.name)?;
        writeln!(out, "    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {{")?;
        writeln!(out, "        Self {{")?;
        writeln!(out, "            client,")?;
        writeln!(out, "            http_client: Arc::new(http_client),")?;
        writeln!(out, "        }}")?;
        writeln!(out, "    }}")?;
        writeln!(out)?;

        // Generate methods for mutating endpoints only
        for endpoint in &wrapper.endpoints {
            self.generate_method(&mut out, endpoint, provider, &wrapper.api_name)?;
        }

        writeln!(out, "}}")?;

        fs::write(output_path, out)?;
        Ok(())
    }

    /// Generate a single method for a provider wrapper.
    fn generate_method(
        &self,
        out: &mut String,
        endpoint: &EndpointFn,
        provider: &str,
        api_name: &str,
    ) -> Result<(), GenWrapperError> {
        let base_name = &endpoint.base_name;

        if endpoint.is_mutating {
            // Generate wrapped method with state tracking
            writeln!(out, "    /// {}.", to_sentence_case(base_name))?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// Automatically stores the result in the state store on success.")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Arguments")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// * `args` - Request arguments")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Returns")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// StreamIterator yielding the {} result.", endpoint.response_type)?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Errors")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// Returns ProviderError if the API request or state storage fails.")?;
            writeln!(out, "    pub fn {}(", base_name)?;
            writeln!(out, "        &self,")?;
            writeln!(out, "        args: &{},", endpoint.args_type)?;
            writeln!(out, "    ) -> Result<")?;
            writeln!(out, "        impl StreamIterator<")?;
            writeln!(out, "            D = Result<{}, ProviderError<ApiError>>,", endpoint.response_type)?;
            writeln!(out, "            P = crate::providers::{}::clients::types::ApiPending,", provider)?;
            writeln!(out, "        > + Send")?;
            writeln!(out, "        + 'static,")?;
            writeln!(out, "        ProviderError<ApiError>,")?;
            writeln!(out, "    > {{")?;
            writeln!(out, "        let builder = {}_builder(", base_name)?;
            writeln!(out, "            &self.http_client,")?;
            // Generate args fields from path_params
            for param in &endpoint.path_params {
                writeln!(out, "            &args.{},", escape_keyword(param))?;
            }
            // Generate args fields from query_params
            for param in &endpoint.query_params {
                writeln!(out, "            &args.{},", escape_keyword(param))?;
            }
            writeln!(out, "        )")?;
            writeln!(out, "        .map_err(ProviderError::Api)?;")?;
            writeln!(out)?;
            writeln!(out, "        let task = {}_task(builder)", base_name)?;
            writeln!(out, "            .map_err(ProviderError::Api)?;")?;
            writeln!(out)?;
            writeln!(out, "        let state_store = self.client.state_store.clone();")?;
            writeln!(out, "        let stage = Some(self.client.stage.clone());")?;
            writeln!(out)?;
            writeln!(out, "        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);")?;
            writeln!(out)?;
            writeln!(out, "        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))")?;
            writeln!(out, "    }}")?;
            writeln!(out)?;
        } else {
            // Generate simple execute method without state tracking
            writeln!(out, "    /// {}.", to_sentence_case(base_name))?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// Read-only operation - no state tracking.")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Arguments")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// * `args` - Request arguments")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Returns")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// StreamIterator yielding the {} result.", endpoint.response_type)?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// # Errors")?;
            writeln!(out, "    ///")?;
            writeln!(out, "    /// Returns ProviderError if the API request fails.")?;
            writeln!(out, "    pub fn {}(", base_name)?;
            writeln!(out, "        &self,")?;
            writeln!(out, "        args: &{},", endpoint.args_type)?;
            writeln!(out, "    ) -> Result<")?;
            writeln!(out, "        impl StreamIterator<")?;
            writeln!(out, "            D = Result<{}, ProviderError<ApiError>>,", endpoint.response_type)?;
            writeln!(out, "            P = crate::providers::{}::clients::types::ApiPending,", provider)?;
            writeln!(out, "        > + Send")?;
            writeln!(out, "        + 'static,")?;
            writeln!(out, "        ProviderError<ApiError>,")?;
            writeln!(out, "    > {{")?;
            writeln!(out, "        let builder = {}_builder(", base_name)?;
            writeln!(out, "            &self.http_client,")?;
            // Generate args fields from path_params
            for param in &endpoint.path_params {
                writeln!(out, "            &args.{},", escape_keyword(param))?;
            }
            // Generate args fields from query_params
            for param in &endpoint.query_params {
                writeln!(out, "            &args.{},", escape_keyword(param))?;
            }
            writeln!(out, "        )")?;
            writeln!(out, "        .map_err(ProviderError::Api)?;")?;
            writeln!(out)?;
            writeln!(out, "        let task = {}_task(builder)", base_name)?;
            writeln!(out, "            .map_err(ProviderError::Api)?;")?;
            writeln!(out)?;
            writeln!(out, "        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))")?;
            writeln!(out, "    }}")?;
            writeln!(out)?;
        }

        Ok(())
    }

    /// Generate api/mod.rs.
    fn generate_api_mod_rs(
        &self,
        catalog: &ApiCatalog,
        output_path: &Path,
    ) -> Result<(), GenWrapperError> {
        let mut out = String::new();

        writeln!(out, "//! {} API providers with automatic state tracking.", catalog.provider)?;
        writeln!(out, "//!")?;
        writeln!(out, "//! WHY: Users need stateful API clients that automatically track")?;
        writeln!(out, "//!      resource changes in the state store.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! WHAT: Per-API provider implementations using StoreStateIdentifierTask.")?;
        writeln!(out, "//!")?;
        writeln!(out, "//! HOW: Each provider wraps ProviderClient<S> and provides methods")?;
        writeln!(out, "//!      that automatically store state on successful operations.")?;
        writeln!(out)?;
        writeln!(out, "#![cfg(feature = \"{}\")]", catalog.provider)?;
        writeln!(out)?;

        for api in &catalog.apis {
            writeln!(out, "pub mod {};", api.name)?;
        }

        fs::write(output_path, out)?;
        Ok(())
    }
}
