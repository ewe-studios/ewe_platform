---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/06-aws-lambda-provider"
this_file: "specifications/11-foundation-deployment/features/06-aws-lambda-provider/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine"]

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# AWS Lambda Provider

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the AWS Lambda deployment provider. This provider is **API-first** — it deploys by calling the AWS Lambda API directly via `SimpleHttpClient` with SigV4 signing, with no CLI tools required.

The provider:
- **Deploys via API** - uploads function code, publishes versions, manages aliases via `lambda.{region}.amazonaws.com`
- **Captures state from API responses** - function ARNs, versions, aliases, API Gateway URLs stored in state store
- **Generates `template.yaml` on demand** - for local use with `sam local start-api`, not as deployment input
- **Falls back to CLI** - can optionally shell out to `sam` or `cargo-lambda` if the user prefers

State is stored in whichever state store the user configures (Turso, SQLite, or JSON files) — there is no special relationship between this provider and any particular state backend.

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ProcessExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration

Required by:
- `07-templates` - AWS-specific template configs
- `09-examples-documentation` - AWS examples

## Requirements

### template.yaml (SAM) Config Parsing

```rust
// providers/aws/config.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed SAM template.yaml - the source of truth for AWS Lambda deployments.
///
/// Reference: https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamTemplate {
    #[serde(rename = "AWSTemplateFormatVersion")]
    pub aws_template_format_version: Option<String>,
    pub transform: Option<String>,                   // "AWS::Serverless-2016-10-31"
    pub description: Option<String>,
    pub globals: Option<SamGlobals>,
    pub parameters: Option<HashMap<String, SamParameter>>,
    pub resources: HashMap<String, SamResource>,
    pub outputs: Option<HashMap<String, SamOutput>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamGlobals {
    pub function: Option<GlobalFunction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GlobalFunction {
    pub timeout: Option<u32>,
    pub runtime: Option<String>,
    pub memory_size: Option<u32>,
    pub architectures: Option<Vec<String>>,
    pub environment: Option<FunctionEnvironment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamResource {
    #[serde(rename = "Type")]
    pub resource_type: String,           // "AWS::Serverless::Function", "AWS::Serverless::HttpApi", etc.
    pub properties: serde_json::Value,   // Resource-specific properties
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FunctionProperties {
    pub handler: String,
    pub runtime: Option<String>,
    pub code_uri: String,
    pub memory_size: Option<u32>,
    pub timeout: Option<u32>,
    pub architectures: Option<Vec<String>>,
    pub environment: Option<FunctionEnvironment>,
    pub events: Option<HashMap<String, EventSource>>,
    pub policies: Option<Vec<serde_json::Value>>,
    pub layers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FunctionEnvironment {
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EventSource {
    #[serde(rename = "Type")]
    pub event_type: String,              // "HttpApi", "Schedule", "SQS", etc.
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamParameter {
    #[serde(rename = "Type")]
    pub param_type: String,
    pub default: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamOutput {
    pub description: Option<String>,
    pub value: serde_json::Value,
}

impl SamTemplate {
    pub fn load(path: &Path) -> Result<Self, DeploymentError> {
        let content = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&content).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })
    }

    /// Extract all Lambda functions from the template.
    pub fn functions(&self) -> Vec<(&str, &SamResource)> {
        self.resources.iter()
            .filter(|(_, r)| r.resource_type == "AWS::Serverless::Function")
            .map(|(name, resource)| (name.as_str(), resource))
            .collect()
    }

    /// Get the stack name (derived from template description or directory name).
    pub fn stack_name(&self) -> String {
        self.description.clone()
            .unwrap_or_else(|| "sam-stack".to_string())
            .to_lowercase()
            .replace(' ', "-")
    }

    pub fn validate(&self) -> Result<(), DeploymentError> {
        if self.resources.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "template.yaml".into(),
                reason: "no resources defined".into(),
            });
        }
        if self.functions().is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "template.yaml".into(),
                reason: "no Lambda functions defined".into(),
            });
        }
        Ok(())
    }
}
```

### AwsLambdaProvider Implementation

```rust
// providers/aws/mod.rs

pub struct AwsLambdaProvider {
    mode: AwsMode,
    working_dir: PathBuf,
    region: Option<String>,
}

pub enum AwsMode {
    /// Shell out to SAM CLI.
    Cli,
    /// Use AWS Lambda/CloudFormation APIs directly.
    Api {
        access_key_id: String,
        secret_access_key: String,
        region: String,
    },
}

impl AwsLambdaProvider {
    pub fn cli(working_dir: &Path) -> Self;
    pub fn api(working_dir: &Path, access_key: &str, secret_key: &str, region: &str) -> Self;

    /// Auto-detect from environment.
    /// Uses AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION.
    pub fn auto(working_dir: &Path) -> Self;

}

impl DeploymentProvider for AwsLambdaProvider {
    type Config = SamTemplate;
    type Resources = AwsResources;

    fn name(&self) -> &str { "aws" }

    fn detect(project_dir: &Path) -> Option<SamTemplate> {
        let config_path = project_dir.join("template.yaml");
        SamTemplate::load(&config_path).ok()
    }

    fn validate(&self, config: &SamTemplate) -> Result<(), DeploymentError> {
        config.validate()
    }

    fn build(&self, config: &SamTemplate, _env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        // For Rust Lambda: cargo lambda build --release
        // For generic: sam build
        let has_cargo = self.working_dir.join("Cargo.toml").exists();
        if has_cargo {
            let output = ProcessExecutor::new("cargo")
                .args(["lambda", "build", "--release"])
                .current_dir(&self.working_dir)
                .execute()?;
            if !output.success {
                return Err(DeploymentError::BuildFailed(output.stderr));
            }
        } else {
            let output = ProcessExecutor::new("sam")
                .arg("build")
                .current_dir(&self.working_dir)
                .execute()?;
            if !output.success {
                return Err(DeploymentError::BuildFailed(output.stderr));
            }
        }
        Ok(BuildOutput { artifacts: vec![], duration_ms: 0 })
    }

    fn deploy(
        &self,
        config: &SamTemplate,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        let result = match &self.mode {
            AwsMode::Cli => self.deploy_cli(config, env, dry_run),
            AwsMode::Api { access_key_id, secret_access_key, region } => {
                self.deploy_api(config, env, dry_run, access_key_id, secret_access_key, region)
            }
        }?;

        Ok(result)
    }

    fn logs(&self, _config: &SamTemplate, _env: Option<&str>) -> Result<(), DeploymentError> {
        ProcessExecutor::new("sam")
            .args(["logs", "--tail"])
            .current_dir(&self.working_dir)
            .execute_streaming(|line| println!("{}", line))?;
        Ok(())
    }

    fn destroy(&self, config: &SamTemplate, _env: Option<&str>) -> Result<(), DeploymentError> {
        ProcessExecutor::new("sam")
            .args(["delete", "--no-prompts"])
            .current_dir(&self.working_dir)
            .execute()?;
        Ok(())
    }

    fn status(&self, _config: &SamTemplate, _env: Option<&str>) -> Result<AwsResources, DeploymentError> {
        todo!()
    }
}
```

### SAM CLI Wrapper

```rust
// providers/aws/sam.rs

impl AwsLambdaProvider {
    fn deploy_cli(
        &self,
        config: &SamTemplate,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("aws", &config.stack_name()));
        }

        let stack_name = match env {
            Some(e) => format!("{}-{}", config.stack_name(), e),
            None => config.stack_name(),
        };

        // sam deploy --stack-name {name} --resolve-s3 --capabilities CAPABILITY_IAM --no-confirm-changeset
        let output = ProcessExecutor::new("sam")
            .args([
                "deploy",
                "--stack-name", &stack_name,
                "--resolve-s3",
                "--capabilities", "CAPABILITY_IAM",
                "--no-confirm-changeset",
                "--no-fail-on-empty-changeset",
            ])
            .current_dir(&self.working_dir)
            .execute()?;

        if !output.success {
            return Err(DeploymentError::ProcessFailed {
                command: "sam deploy".into(),
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }

        Ok(DeploymentResult {
            deployment_id: extract_changeset_id(&output.stdout)
                .unwrap_or_else(|| chrono::Utc::now().timestamp().to_string()),
            provider: "aws".to_string(),
            resource_name: stack_name,
            environment: env.map(String::from),
            url: extract_api_url(&output.stdout),
            deployed_at: chrono::Utc::now(),
        })
    }
}
```

### AWS Lambda API Client

```rust
// providers/aws/api.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// AWS API uses Signature Version 4 for authentication.
/// For the API mode, we implement SigV4 signing.

impl AwsLambdaProvider {
    fn deploy_api(
        &self,
        config: &SamTemplate,
        env: Option<&str>,
        dry_run: bool,
        access_key: &str,
        secret_key: &str,
        region: &str,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("aws", &config.stack_name()));
        }

        // For direct API deployment:
        // 1. Package Lambda code (zip the build artifacts)
        // 2. Upload to S3 (or use inline zip for small functions)
        // 3. Create/Update Lambda function via PUT /functions/{name}/code
        // 4. Publish new version
        // 5. Update alias to point to new version

        // AWS Lambda API: https://docs.aws.amazon.com/lambda/latest/api/
        let lambda_url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions",
            region
        );

        // ... SimpleHttpClient with SigV4 auth headers

        Ok(DeploymentResult {
            deployment_id: "api-deploy".to_string(),
            provider: "aws".to_string(),
            resource_name: config.stack_name(),
            environment: env.map(String::from),
            url: None,
            deployed_at: chrono::Utc::now(),
        })
    }
}

/// AWS SigV4 request signing.
pub fn sign_aws_request(
    method: &str,
    url: &str,
    headers: &mut Vec<(String, String)>,
    body: &[u8],
    access_key: &str,
    secret_key: &str,
    region: &str,
    service: &str,
) {
    // Implement AWS Signature Version 4
    // 1. Create canonical request
    // 2. Create string to sign
    // 3. Calculate signature
    // 4. Add Authorization header
}

#[derive(Debug)]
pub struct AwsResources {
    pub stack_name: String,
    pub functions: Vec<LambdaFunctionInfo>,
    pub api_endpoints: Vec<String>,
    pub outputs: HashMap<String, String>,
}

#[derive(Debug)]
pub struct LambdaFunctionInfo {
    pub name: String,
    pub runtime: String,
    pub memory_mb: u32,
    pub timeout_sec: u32,
    pub last_modified: String,
    pub version: String,
}
```

## Tasks

1. **Create module structure**
   - [ ] Create `src/providers/aws/mod.rs`, `config.rs`, `sam.rs`, `api.rs`
   - [ ] Register in `src/providers/mod.rs`

2. **Implement template.yaml parsing**
   - [ ] Define SAM template config structs
   - [ ] Implement `SamTemplate::load()`
   - [ ] Implement `functions()`, `stack_name()`, `validate()`
   - [ ] Handle globals inheritance (timeout, runtime from Globals to Functions)
   - [ ] Write unit tests with sample SAM templates

3. **Implement AwsLambdaProvider trait**
   - [ ] Implement `detect()`, `validate()`, `build()`, `deploy()`, `logs()`, `destroy()`, `status()`
   - [ ] Handle Rust Lambda (cargo lambda) vs generic (sam build) builds
   - [ ] Auto-detect from AWS environment variables

4. **Implement CLI mode (SAM wrapper)**
   - [ ] Implement `deploy_cli()` using `sam deploy`
   - [ ] Handle `--stack-name` with environment suffix
   - [ ] Parse SAM deploy output for stack outputs
   - [ ] Write tests with mock output

5. **Implement API mode**
   - [ ] Implement AWS SigV4 request signing
   - [ ] Implement Lambda function create/update via API
   - [ ] Implement version publishing and alias management
   - [ ] Write tests with mock HTTP responses

6. **Implement cargo-lambda support**
   - [ ] Detect Rust Lambda projects (Cargo.toml with lambda runtime)
   - [ ] Build via `cargo lambda build --release --arm64`
   - [ ] Package bootstrap binary for `provided.al2023` runtime

7. **Write integration tests**
   - [ ] Test SAM template parsing with real templates
   - [ ] Test CLI deploy (requires SAM CLI + AWS, mark `#[ignore]`)
   - [ ] Test API deploy (requires AWS credentials, mark `#[ignore]`)

## AWS API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `PUT` | `lambda.{region}.amazonaws.com/.../functions/{name}/code` | Update function code |
| `POST` | `lambda.{region}.amazonaws.com/.../functions` | Create function |
| `POST` | `lambda.{region}.amazonaws.com/.../functions/{name}/versions` | Publish version |
| `PUT` | `lambda.{region}.amazonaws.com/.../functions/{name}/aliases/{alias}` | Update alias |
| `POST` | `lambda.{region}.amazonaws.com/.../functions/{name}/invocations` | Invoke function |

## Success Criteria

- [ ] All 7 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] SAM template parsing handles common configurations
- [ ] CLI deploy works with `sam deploy`
- [ ] API mode deploys via Lambda API with SigV4 signing
- [ ] Rust Lambda builds with `cargo lambda`
- [ ] Environment support works (staging, production)

## Verification

```bash
cd backends/foundation_deployment
cargo test aws -- --nocapture

# Integration (requires SAM CLI + AWS credentials)
cargo test aws_integration -- --ignored --nocapture
```

---

_Created: 2026-03-26_
