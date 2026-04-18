//! Analyzer for OpenAPI specs with provider-specific grouping logic.
//!
//! WHY: Analyzes specs to determine optimal grouping and detect shared resources.
//!
//! WHAT: Groups endpoints (10-200 per group), detects shared types.
//!
//! HOW: Path prefix grouping, split large groups, provider-specific smart grouping.
//!      Semantically similar concepts are grouped together thoughtfully.

use crate::EndpointInfo;
use std::collections::{HashMap, HashSet};

// =============================================================================
// CLOUDFLARE GROUPING PATTERNS
// =============================================================================
// Order matters - first match wins. More specific patterns come first.
// Semantically similar concepts are grouped under the same category.

const CLOUDFLARE_GROUP_PATTERNS: &[(&str, &str)] = &[
    // === AI/ML (all AI-related together) ===
    ("/ai/", "ai"),
    ("ai-gateway", "ai"),
    ("ai-search", "ai"),
    ("autorag", "ai"),
    ("vectorize", "ai"),
    ("finetune", "ai"),
    ("inference", "ai"),
    // AI model names
    ("llama", "ai"),
    ("gemma", "ai"),
    ("mistral", "ai"),
    ("flux", "ai"),
    ("deepseek", "ai"),
    ("qwen", "ai"),
    ("instruct", "ai"),
    ("embedding", "ai"),
    ("reranker", "ai"),
    ("completions", "ai"),
    ("chat", "ai"),

    // === Workers & Compute (serverless/runtime) ===
    ("/workers/", "workers"),
    ("hyperdrive", "workers"),
    ("containers", "workers"),
    ("pages", "workers"),
    ("builds", "workers"),
    ("d1", "workers"),
    ("pipelines", "workers"),
    ("dispatch", "workers"),
    ("cron", "workers"),
    ("bindings", "workers"),
    ("triggers", "workers"),
    ("assets", "workers"),

    // === Stream & Media (video/audio/images) ===
    ("/stream/", "stream"),
    ("images", "media"),
    ("image", "media"),
    ("browser-rendering", "media"),
    ("calls", "media"),
    ("realtime", "media"),
    ("livestream", "media"),
    ("video", "media"),
    ("audio", "media"),

    // === Network & Tunnels (connectivity) ===
    ("/tunnels/", "network"),
    ("cfd_tunnel", "network"),
    ("warp_connector", "network"),
    ("warp", "network"),
    ("teamnet", "network"),
    ("dex", "network"),
    ("connectivity", "network"),
    ("interconnect", "network"),
    ("cni", "network"),
    ("ipsec", "network"),
    ("gre", "network"),
    ("vlan", "network"),
    ("bgp", "network"),

    // === DNS & Zones (domain management) ===
    ("/zones/", "zones"),
    ("/dns/", "dns"),
    ("dns_records", "dns"),
    ("dns_firewall", "dns"),
    ("dns_settings", "dns"),
    ("secondary_dns", "dns"),
    ("custom_ns", "dns"),
    ("hostname", "dns"),
    ("hostnames", "dns"),
    ("domain", "domains"),
    ("domains", "domains"),
    ("registrar", "domains"),
    ("tlds", "domains"),

    // === Security (threat protection) ===
    ("cloudforce", "security"),
    ("intel", "security"),
    ("brand-protection", "security"),
    ("botnet", "security"),
    ("urlscanner", "security"),
    ("threat", "security"),
    ("attacks", "security"),
    ("firewall", "security"),
    ("waf", "security"),
    ("ddos", "security"),
    ("bot", "security"),
    ("rate_limit", "security"),
    ("challenges", "security"),
    ("fingerprints", "security"),

    // === Access & Identity (auth/users) ===
    ("/access/", "access"),
    ("iam", "access"),
    ("sso_connectors", "access"),
    ("scim", "access"),
    ("members", "access"),
    ("member", "access"),
    ("organizations", "access"),
    ("roles", "access"),
    ("permissions", "access"),
    ("users", "access"),
    ("user", "access"),

    // === Email ===
    ("email-security", "email"),
    ("/email/", "email"),
    ("dkim", "email"),
    ("dmarc", "email"),
    ("spf", "email"),
    ("email_routing", "email"),

    // === Devices & Gateway (Zero Trust) ===
    ("/devices/", "devices"),
    ("ip-profiles", "devices"),
    ("gateway", "devices"),
    ("zerotrust", "devices"),
    ("zt_risk", "devices"),

    // === Storage (R2/buckets) ===
    ("/r2/", "storage"),
    ("r2-catalog", "storage"),
    ("bucket", "storage"),
    ("storage", "storage"),
    ("objects", "storage"),

    // === Radar/Analytics (traffic insights) ===
    ("/radar/", "radar"),
    ("analytics", "analytics"),
    ("logs", "logs"),
    ("logpush", "logs"),
    ("log", "logs"),

    // === Billing ===
    ("billing", "billing"),
    ("subscription", "billing"),
    ("subscriptions", "billing"),
    ("pay-per-crawl", "billing"),
    ("invoice", "billing"),
    ("plans", "billing"),

    // === Rules ===
    ("rulesets", "rules"),
    ("rules", "rules"),
    ("pagerules", "rules"),
    ("filters", "rules"),
    ("policies", "rules"),
    ("policy", "rules"),

    // === Certificates ===
    ("ssl", "certificates"),
    ("tls", "certificates"),
    ("certificate", "certificates"),
    ("certificates", "certificates"),
    ("mtls", "certificates"),
    ("keyless", "certificates"),
    ("acm", "certificates"),

    // === Cache ===
    ("cache", "cache"),
    ("purge", "cache"),
    ("argo", "cache"),
    ("tiered_cache", "cache"),

    // === Load Balancing ===
    ("load_balancer", "load_balancers"),
    ("loadbalancer", "load_balancers"),
    ("pool", "load_balancers"),
    ("pools", "load_balancers"),
    ("healthcheck", "load_balancers"),
    ("healthchecks", "load_balancers"),
    ("monitor", "load_balancers"),
    ("monitors", "load_balancers"),

    // === Magic WAN ===
    ("magic/", "magic_wan"),
    ("magic_", "magic_wan"),
    ("wan", "magic_wan"),
    ("addressing", "magic_wan"),

    // === Data (schemas, datasets, documents) ===
    ("dlp", "data"),
    ("datasets", "data"),
    ("dataset", "data"),
    ("schemas", "data"),
    ("schema", "data"),
    ("mapping", "data"),
    ("mappings", "data"),
    ("documents", "data"),
    ("document", "data"),
    ("entries", "data"),
    ("records", "data"),

    // === Queues ===
    ("queue", "queues"),
    ("queues", "queues"),

    // === Secrets & Tokens ===
    ("secrets_store", "secrets"),
    ("secrets", "secrets"),
    ("tokens", "tokens"),
    ("token", "tokens"),
    ("api_token", "tokens"),

    // === Events ===
    ("event", "events"),
    ("events", "events"),
    ("tags", "events"),
    ("tag", "events"),
    ("alerting", "events"),
    ("alerts", "events"),
    ("notifications", "events"),

    // === Waiting Rooms ===
    ("waiting_room", "waiting_rooms"),
    ("waitingrooms", "waiting_rooms"),

    // === Custom Pages ===
    ("custom_pages", "custom_pages"),
    ("custom_hostnames", "custom_pages"),

    // === API Gateway ===
    ("api_gateway", "api_gateway"),

    // === Page Shield ===
    ("page_shield", "page_shield"),

    // === Security Center ===
    ("security-center", "security_center"),

    // === Spectrum ===
    ("spectrum", "spectrum"),

    // === Web3 ===
    ("web3", "web3"),

    // === Snippets ===
    ("snippets", "snippets"),

    // === Config ===
    ("settings", "config"),
    ("config", "config"),
    ("configuration", "config"),

    // === Apps ===
    ("app", "apps"),
    ("apps", "apps"),

    // === Shares ===
    ("share", "shares"),
    ("shares", "shares"),

    // === Migration ===
    ("move", "migration"),
    ("migration", "migration"),
    ("migrate", "migration"),
    ("transfer", "migration"),

    // === Metrics ===
    ("stats", "metrics"),
    ("statistics", "metrics"),
    ("metrics", "metrics"),
    ("slots", "metrics"),

    // === Tasks ===
    ("task", "tasks"),
    ("tasks", "tasks"),
    ("job", "tasks"),
    ("jobs", "tasks"),

    // === Reports ===
    ("reports", "reports"),
    ("report", "reports"),
    ("predefined_reports", "reports"),

    // === Routes ===
    ("routes", "routes"),
    ("route", "routes"),
    ("routing", "routes"),

    // === Stores ===
    ("stores", "stores"),
    ("store", "stores"),

    // === Subnets ===
    ("subnets", "subnets"),
    ("subnet", "subnets"),
    ("prefixes", "subnets"),
    ("prefix", "subnets"),

    // === Targets ===
    ("target", "targets"),
    ("targets", "targets"),

    // === Sessions ===
    ("abort", "sessions"),
    ("recording", "sessions"),
    ("session", "sessions"),
    ("sessions", "sessions"),

    // === Audit ===
    ("audit_log", "audit"),
    ("audit_logs", "audit"),
    ("audit", "audit"),

    // === Entitlements ===
    ("entitlement", "entitlements"),
    ("entitlements", "entitlements"),

    // === Diagnostics ===
    ("diagnostic", "diagnostics"),
    ("diagnostics", "diagnostics"),

    // === Infrastructure ===
    ("infrastructure", "infrastructure"),

    // === Resources ===
    ("resource-library", "resources"),
    ("resource", "resources"),

    // === Slurper ===
    ("slurper", "slurper"),

    // === Tracer ===
    ("request-tracer", "tracer"),
    ("request_tracer", "tracer"),

    // === Credentials ===
    ("leaked-credential", "credentials"),
    ("leaked_credential", "credentials"),

    // === Origin ===
    ("origin_tls", "origin"),
    ("origin-tls", "origin"),

    // === Validation ===
    ("token_validation", "validation"),
    ("token-validation", "validation"),

    // === Fraud ===
    ("fraud_detection", "fraud"),
    ("fraud-detection", "fraud"),

    // === Shield ===
    ("smart_shield", "shield"),
    ("smart-shield", "shield"),

    // === Speed ===
    ("speed_api", "speed"),
    ("speed-api", "speed"),

    // === Scan ===
    ("content-upload-scan", "scan"),
    ("content_upload_scan", "scan"),

    // === Hold ===
    ("hold", "hold"),

    // === Connector ===
    ("cloud_connector", "connector"),
    ("cloud-connector", "connector"),

    // === Headers ===
    ("managed_headers", "headers"),
    ("managed-headers", "headers"),

    // === Profile ===
    ("profile", "profile"),
    ("profiles", "profile"),

    // === Memberships ===
    ("membership", "memberships"),
    ("memberships", "memberships"),

    // === Tenants ===
    ("tenant", "tenants"),
    ("tenants", "tenants"),

    // === System ===
    ("system/", "system"),
    ("internal/", "internal"),

    // === Health ===
    ("ready", "health"),
    ("health", "health"),
    ("live", "health"),

    // === IPs ===
    ("/ips", "ips"),

    // === Signed URL ===
    ("signed-url", "signed_url"),
    ("signed_url", "signed_url"),

    // === Accounts ===
    ("/accounts/", "accounts"),

    // === User ===
    ("/user/", "user"),
    ("/users/", "user"),
];

// =============================================================================
// GCP GROUPING PATTERNS
// =============================================================================

const GCP_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Compute
    ("compute", "compute"),
    ("instance", "compute"),
    ("disk", "compute"),
    ("snapshot", "compute"),
    ("image", "compute"),
    ("zone", "compute"),
    ("region", "compute"),
    // Kubernetes Engine
    ("container", "gke"),
    ("gke", "gke"),
    ("cluster", "gke"),
    ("nodepool", "gke"),
    ("node_pool", "gke"),
    // Cloud Run
    ("run", "cloud_run"),
    ("revision", "cloud_run"),
    ("service", "cloud_run"),
    // Cloud Functions
    ("function", "cloud_functions"),
    ("functions", "cloud_functions"),
    // Storage
    ("storage", "storage"),
    ("bucket", "storage"),
    ("object", "storage"),
    // BigQuery
    ("bigquery", "bigquery"),
    ("dataset", "bigquery"),
    ("table", "bigquery"),
    ("job", "bigquery"),
    // Cloud SQL
    ("sql", "cloud_sql"),
    ("sqladmin", "cloud_sql"),
    ("instance", "cloud_sql"),
    ("backup", "cloud_sql"),
    // IAM
    ("iam", "iam"),
    ("policy", "iam"),
    ("role", "iam"),
    ("member", "iam"),
    // Networking
    ("network", "networking"),
    ("vpc", "networking"),
    ("firewall", "networking"),
    ("route", "networking"),
    ("subnet", "networking"),
    ("address", "networking"),
    ("forwarding", "networking"),
    // Monitoring
    ("monitoring", "monitoring"),
    ("metric", "monitoring"),
    ("alert", "monitoring"),
    ("uptime", "monitoring"),
    // Logging
    ("logging", "logging"),
    ("log", "logging"),
    ("sink", "logging"),
    // Secret Manager
    ("secret", "secret_manager"),
    ("secretmanager", "secret_manager"),
    // Pub/Sub
    ("pubsub", "pubsub"),
    ("topic", "pubsub"),
    ("subscription", "pubsub"),
    // Cloud Build
    ("build", "cloudbuild"),
    ("cloudbuild", "cloudbuild"),
    // Artifact Registry
    ("artifact", "artifact_registry"),
    ("repository", "artifact_registry"),
    // DNS
    ("dns", "dns"),
    // Billing
    ("billing", "billing"),
    ("invoice", "billing"),
    // Organization
    ("org", "organization"),
    ("organization", "organization"),
    ("folder", "organization"),
    // KMS
    ("kms", "kms"),
    ("key", "kms"),
    ("cryptokey", "kms"),
];

// =============================================================================
// AWS GROUPING PATTERNS
// =============================================================================

const AWS_GROUP_PATTERNS: &[(&str, &str)] = &[
    // EC2
    ("instance", "ec2"),
    ("ec2", "ec2"),
    ("ami", "ec2"),
    ("volume", "ec2"),
    ("snapshot", "ec2"),
    ("securitygroup", "ec2"),
    ("vpc", "ec2"),
    ("subnet", "ec2"),
    // S3
    ("s3", "s3"),
    ("bucket", "s3"),
    ("object", "s3"),
    // Lambda
    ("lambda", "lambda"),
    ("function", "lambda"),
    ("layer", "lambda"),
    // IAM
    ("iam", "iam"),
    ("user", "iam"),
    ("role", "iam"),
    ("policy", "iam"),
    ("group", "iam"),
    // RDS
    ("rds", "rds"),
    ("dbinstance", "rds"),
    ("dbcluster", "rds"),
    ("snapshot", "rds"),
    // DynamoDB
    ("dynamodb", "dynamodb"),
    ("table", "dynamodb"),
    ("item", "dynamodb"),
    // CloudFormation
    ("cloudformation", "cloudformation"),
    ("stack", "cloudformation"),
    // ECS/EKS
    ("ecs", "ecs"),
    ("eks", "eks"),
    ("cluster", "ecs"),
    ("task", "ecs"),
    ("service", "ecs"),
    // API Gateway
    ("apigateway", "api_gateway"),
    ("api", "api_gateway"),
    ("restapi", "api_gateway"),
    // CloudWatch
    ("cloudwatch", "cloudwatch"),
    ("alarm", "cloudwatch"),
    ("metric", "cloudwatch"),
    ("log", "cloudwatch"),
    // Secrets Manager
    ("secret", "secrets_manager"),
    ("secretmanager", "secrets_manager"),
    // SQS/SNS
    ("sqs", "sqs"),
    ("queue", "sqs"),
    ("sns", "sns"),
];

// =============================================================================
// STRIPE GROUPING PATTERNS
// =============================================================================

const STRIPE_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Payments
    ("payment_intent", "payments"),
    ("payment", "payments"),
    ("charge", "payments"),
    ("refund", "payments"),
    ("capture", "payments"),
    // Customers
    ("customer", "customers"),
    ("address", "customers"),
    // Products & Pricing
    ("product", "products"),
    ("price", "products"),
    ("sku", "products"),
    // Subscriptions
    ("subscription", "subscriptions"),
    ("subscriptionitem", "subscriptions"),
    ("subscriptionschedule", "subscriptions"),
    // Invoicing
    ("invoice", "invoicing"),
    ("invoiceitem", "invoicing"),
    // Payment Methods
    ("payment_method", "payment_methods"),
    ("card", "payment_methods"),
    ("bankaccount", "payment_methods"),
    ("source", "payment_methods"),
    // Checkout
    ("checkout", "checkout"),
    ("session", "checkout"),
    // Connect
    ("account", "connect"),
    ("transfer", "connect"),
    ("payout", "connect"),
    // Webhooks
    ("webhook", "webhooks"),
    ("endpoint", "webhooks"),
    // Events
    ("event", "events"),
    // Disputes
    ("dispute", "disputes"),
    ("chargeback", "disputes"),
    // Radar (fraud)
    ("radar", "radar"),
    ("fraud", "radar"),
    ("review", "radar"),
    // Terminal
    ("terminal", "terminal"),
    ("reader", "terminal"),
    // Reporting
    ("report", "reporting"),
    ("reportingrun", "reporting"),
];

// =============================================================================
// SUPABASE GROUPING PATTERNS
// =============================================================================

const SUPABASE_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Database
    ("database", "database"),
    ("table", "database"),
    ("column", "database"),
    ("schema", "database"),
    ("query", "database"),
    // Auth
    ("auth", "auth"),
    ("user", "auth"),
    ("session", "auth"),
    ("token", "auth"),
    ("invite", "auth"),
    // Storage
    ("storage", "storage"),
    ("bucket", "storage"),
    ("object", "storage"),
    ("file", "storage"),
    // Functions
    ("function", "functions"),
    ("edge", "functions"),
    // Realtime
    ("realtime", "realtime"),
    ("channel", "realtime"),
    ("subscription", "realtime"),
    // Projects
    ("project", "projects"),
    ("organization", "projects"),
    // Secrets
    ("secret", "secrets"),
    // Backups
    ("backup", "backups"),
    ("restore", "backups"),
];

// =============================================================================
// NEON GROUPING PATTERNS
// =============================================================================

const NEON_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Projects
    ("project", "projects"),
    ("branch", "projects"),
    // Compute
    ("compute", "compute"),
    ("endpoint", "compute"),
    // Database
    ("database", "databases"),
    ("role", "databases"),
    // Operations
    ("operation", "operations"),
    // Connections
    ("connection", "connections"),
    ("pooler", "connections"),
    // Backups
    ("backup", "backups"),
    ("restore", "backups"),
    // Metrics
    ("metric", "metrics"),
    ("api_key", "api_keys"),
];

// =============================================================================
// FLY.IO GROUPING PATTERNS
// =============================================================================

const FLY_IO_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Apps
    ("app", "apps"),
    ("application", "apps"),
    // Machines
    ("machine", "machines"),
    ("vm", "machines"),
    // Volumes
    ("volume", "volumes"),
    ("disk", "volumes"),
    // Networking
    ("ip", "networking"),
    ("dns", "networking"),
    ("certificate", "networking"),
    ("tls", "networking"),
    // Deployments
    ("deploy", "deployments"),
    ("release", "deployments"),
    // Secrets
    ("secret", "secrets"),
    // Organizations
    ("org", "organizations"),
    ("member", "organizations"),
    // Regions
    ("region", "regions"),
    ("vm_size", "regions"),
];

// =============================================================================
// HUGGING FACE GROUPING PATTERNS
// =============================================================================

const HUGGINGFACE_GROUP_PATTERNS: &[(&str, &str)] = &[
    // Models
    ("model", "models"),
    ("repo", "models"),
    // Datasets
    ("dataset", "datasets"),
    // Spaces
    ("space", "spaces"),
    // Inference
    ("inference", "inference"),
    ("predict", "inference"),
    // Users/Orgs
    ("user", "users"),
    ("org", "organizations"),
    // Tasks
    ("task", "tasks"),
    // Collections
    ("collection", "collections"),
    // Webhooks
    ("webhook", "webhooks"),
];

/// Apply provider-specific grouping based on patterns.
fn apply_provider_grouping(
    endpoints: Vec<EndpointInfo>,
    provider: &str,
) -> HashMap<String, Vec<EndpointInfo>> {
    let patterns = match provider {
        "cloudflare" => CLOUDFLARE_GROUP_PATTERNS,
        "gcp" => GCP_GROUP_PATTERNS,
        "aws" => AWS_GROUP_PATTERNS,
        "stripe" => STRIPE_GROUP_PATTERNS,
        "supabase" => SUPABASE_GROUP_PATTERNS,
        "neon" => NEON_GROUP_PATTERNS,
        "fly_io" => FLY_IO_GROUP_PATTERNS,
        "huggingface" => HUGGINGFACE_GROUP_PATTERNS,
        _ => return HashMap::new(),
    };

    let mut groups: HashMap<String, Vec<EndpointInfo>> = HashMap::new();

    for endpoint in endpoints {
        let path_lower = endpoint.path.to_lowercase();
        let operation_lower = endpoint.operation_id.to_lowercase();
        let mut matched_group: Option<String> = None;

        for (pattern, group) in patterns {
            if path_lower.contains(pattern) || operation_lower.contains(pattern) {
                matched_group = Some(group.to_string());
                break;
            }
        }

        if let Some(group) = matched_group {
            groups.entry(group).or_default().push(endpoint);
        } else {
            let fallback_group = extract_group_name(&endpoint.path);
            groups.entry(fallback_group).or_default().push(endpoint);
        }
    }

    groups
}

/// Analysis result containing grouped endpoints, shared resources, and schemas.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub groups: Vec<ApiGroup>,
    pub shared_resources: Vec<String>,
    pub provider: String,
    pub schemas: std::sync::Arc<std::collections::BTreeMap<String, crate::spec::Schema>>,
}

/// A group of related endpoints.
#[derive(Debug, Clone)]
pub struct ApiGroup {
    pub name: String,
    pub endpoints: Vec<EndpointInfo>,
    pub response_types: Vec<String>,
    pub request_types: Vec<String>,
}

/// Options for analysis configuration.
#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    pub min_group_size: usize,
    pub max_group_size: usize,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            min_group_size: 10,
            max_group_size: 150,
        }
    }
}

/// Analyze an OpenAPI spec and return grouping information.
pub fn analyze_spec(spec_content: &str, provider: &str, options: &AnalysisOptions) -> Result<AnalysisResult, crate::ProcessError> {
    let processor = crate::process_spec(spec_content)?;
    let endpoints = processor.endpoints();

    let mut groups_map = apply_provider_grouping(endpoints, provider);

    if groups_map.is_empty() {
        for endpoint in processor.endpoints() {
            let group_name = extract_group_name(&endpoint.path);
            groups_map.entry(group_name).or_default().push(endpoint);
        }
    }

    let mut groups: Vec<ApiGroup> = groups_map
        .into_iter()
        .map(|(name, endpoints)| {
            let mut response_types = Vec::new();
            let mut request_types = Vec::new();

            for ep in &endpoints {
                if let Some(rt) = &ep.response_type {
                    let type_name = rt.as_rust_type().to_string();
                    if type_name != "()" && type_name != "serde_json::Value" {
                        response_types.push(type_name);
                    }
                }
                if let Some(rt) = &ep.request_type {
                    request_types.push(rt.clone());
                }
            }

            response_types.sort();
            response_types.dedup();
            request_types.sort();
            request_types.dedup();

            ApiGroup {
                name,
                endpoints,
                response_types,
                request_types,
            }
        })
        .collect();

    groups.sort_by(|a, b| a.name.cmp(&b.name));
    groups = apply_grouping_constraints(groups, options);
    let shared_resources = detect_shared_resources(&groups);

    // Get schemas from processor
    let schemas = processor.schemas();

    Ok(AnalysisResult {
        groups,
        shared_resources,
        provider: provider.to_string(),
        schemas,
    })
}

fn extract_group_name(path: &str) -> String {
    let path = path.trim_start_matches('/');
    let segments: Vec<&str> = path.split('/').collect();
    let non_param_segments: Vec<&&str> = segments
        .iter()
        .filter(|s| !s.starts_with('{') && !s.is_empty())
        .collect();

    if let Some(last) = non_param_segments.last() {
        last.to_string()
    } else {
        "default".to_string()
    }
}

/// Apply grouping constraints: keep semantic groups intact (no splitting).
fn apply_grouping_constraints(mut groups: Vec<ApiGroup>, _options: &AnalysisOptions) -> Vec<ApiGroup> {
    // Don't split groups - semantic grouping is more important than size limits
    // The patterns are designed to group related APIs together thoughtfully
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    groups
}

fn detect_shared_resources(groups: &[ApiGroup]) -> Vec<String> {
    let mut type_usage: HashMap<String, HashSet<String>> = HashMap::new();

    for group in groups {
        for type_name in &group.response_types {
            type_usage.entry(type_name.clone()).or_default().insert(group.name.clone());
        }
        for type_name in &group.request_types {
            type_usage.entry(type_name.clone()).or_default().insert(group.name.clone());
        }
    }

    let mut shared: Vec<String> = type_usage
        .into_iter()
        .filter(|(_, groups)| groups.len() >= 2)
        .map(|(name, _)| name)
        .collect();

    shared.sort();
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_group_name() {
        assert_eq!(extract_group_name("/v1/projects/{project}/instances/{instance}"), "instances");
        assert_eq!(extract_group_name("/v1/run/projects/{project}/locations/{location}/instances"), "instances");
        assert_eq!(extract_group_name("/projects"), "projects");
        assert_eq!(extract_group_name("/"), "default");
    }
}
