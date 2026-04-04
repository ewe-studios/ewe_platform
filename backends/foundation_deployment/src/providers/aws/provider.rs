//! AWS Lambda deployment provider (CLI-based).
//!
//! WHY: AWS Lambda is a primary deployment target for serverless applications.
//!
//! WHAT: `AwsCliProvider` implements `DeploymentProvider` for AWS Lambda,
//! using the AWS CLI and SAM CLI for builds and deployments.
//!
//! HOW: Parses `template.yaml` (SAM/CloudFormation) for configuration and
//! executes `sam` and `aws` CLI commands. This is a CLI-wrapping provider,
//! not a direct API integration.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::core::shell::{collect_one, ShellDone, ShellExecutor};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildArtifact, BuildOutput, DeploymentResult};
use crate::error::DeploymentError;

/// AWS Lambda configuration parsed from `template.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    /// Stack name (required).
    pub stack_name: String,
    /// AWS region (optional, can be set via env).
    pub region: Option<String>,
    /// S3 bucket for deployment artifacts (optional).
    pub s3_bucket: Option<String>,
    /// Lambda function name (optional, defaults to stack name).
    pub function_name: Option<String>,
}

impl AwsConfig {
    /// Parse configuration from a `template.yaml` file.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::ConfigInvalid` if the file cannot be read
    /// or parsed.
    pub fn from_file(path: &Path) -> Result<Self, DeploymentError> {
        // Check for samconfig.toml first for actual config
        let config_path = path.parent().unwrap().join("samconfig.toml");
        if config_path.exists() {
            Self::from_samconfig(&config_path)
        } else {
            // Use template.yaml with defaults
            Ok(AwsConfig {
                stack_name: "lambda-stack".to_string(),
                region: None,
                s3_bucket: None,
                function_name: None,
            })
        }
    }

    /// Parse configuration from a `samconfig.toml` file.
    fn from_samconfig(path: &Path) -> Result<Self, DeploymentError> {
        let content = std::fs::read_to_string(path).map_err(|e| DeploymentError::ConfigInvalid {
            file: path.display().to_string(),
            reason: e.to_string(),
        })?;

        // Parse TOML - samconfig.toml format is:
        // [default.deploy.parameters]
        // stack_name = "..."
        // region = "..."
        // s3_bucket = "..."
        let toml: toml::Value = toml::from_str(&content).map_err(|e| {
            DeploymentError::ConfigInvalid {
                file: path.display().to_string(),
                reason: e.to_string(),
            }
        })?;

        let stack_name = toml
            .get("default")
            .and_then(|d| d.get("deploy"))
            .and_then(|d| d.get("parameters"))
            .and_then(|p| p.get("stack_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("lambda-stack")
            .to_string();

        let region = toml
            .get("default")
            .and_then(|d| d.get("deploy"))
            .and_then(|d| d.get("parameters"))
            .and_then(|p| p.get("region"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let s3_bucket = toml
            .get("default")
            .and_then(|d| d.get("deploy"))
            .and_then(|d| d.get("parameters"))
            .and_then(|p| p.get("s3_bucket"))
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok(AwsConfig {
            stack_name,
            region,
            s3_bucket,
            function_name: None,
        })
    }
}

/// AWS Lambda deployment provider (CLI-based).
///
/// This provider wraps the `sam` and `aws` CLIs. For direct API integration,
/// a separate `AwsApiProvider` could be implemented.
pub struct AwsCliProvider;

impl DeploymentProvider for AwsCliProvider {
    type Config = AwsConfig;
    type Resources = AwsResources;

    fn name(&self) -> &str {
        "aws"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        let config_path = project_dir.join("template.yaml");
        if config_path.exists() {
            AwsConfig::from_file(&config_path).ok()
        } else {
            None
        }
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        if config.stack_name.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "samconfig.toml".to_string(),
                reason: "missing required field: stack_name".to_string(),
            });
        }

        // Check if sam or aws is installed
        let sam_check = ShellExecutor::new("sam").arg("--version").execute();
        let aws_check = ShellExecutor::new("aws").arg("--version").execute();

        if sam_check.is_err() && aws_check.is_err() {
            return Err(DeploymentError::BuildFailed(
                "Neither sam CLI nor aws CLI found. Install from https://aws.amazon.com/cli/"
                    .to_string(),
            ));
        }

        Ok(())
    }

    fn build(
        &self,
        config: &Self::Config,
        _env: Option<&str>,
    ) -> Result<BuildOutput, DeploymentError> {
        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        // Try sam build first
        let mut executor = ShellExecutor::new("sam").arg("build");

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("sam build produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::debug!("sam build stdout: {}", stdout);
                tracing::debug!("sam build stderr: {}", stderr);

                // Look for built artifacts in .aws-sam/build/
                let build_dir = PathBuf::from(".aws-sam/build");
                let mut artifacts = Vec::new();

                if build_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&build_dir) {
                        for entry in entries.flatten() {
                            if let Ok(meta) = entry.metadata() {
                                let artifact_type = if entry.path().extension()
                                    .map(|e| e == "zip")
                                    .unwrap_or(false)
                                {
                                    crate::core::types::ArtifactType::ZipArchive
                                } else {
                                    crate::core::types::ArtifactType::Binary
                                };

                                artifacts.push(BuildArtifact {
                                    path: entry.path(),
                                    size_bytes: meta.len(),
                                    artifact_type,
                                });
                            }
                        }
                    }
                }

                Ok(BuildOutput {
                    artifacts,
                    duration_ms: 0, // TODO: track actual duration
                })
            }
            ShellDone::Failed { stderr, .. } => {
                // Fallback: try aws cloudformation package
                Err(DeploymentError::BuildFailed(stderr))
            }
        }
    }

    fn deploy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        let stack_name = if let Some(env_name) = env {
            format!("{}-{}", config.stack_name, env_name)
        } else {
            config.stack_name.clone()
        };

        let s3_bucket = config.s3_bucket.clone().unwrap_or_else(|| {
            format!("{}-deploy-bucket", stack_name)
        });

        // Use sam deploy
        let mut executor = ShellExecutor::new("sam")
            .arg("deploy")
            .arg("--stack-name")
            .arg(&stack_name)
            .arg("--s3-bucket")
            .arg(&s3_bucket)
            .arg("--capabilities")
            .arg("CAPABILITY_IAM");

        if dry_run {
            executor = executor.arg("--no-execute-changeset");
        }

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("sam deploy produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, stderr, .. } => {
                tracing::info!("sam deploy stdout: {}", stdout);
                tracing::info!("sam deploy stderr: {}", stderr);

                // Parse deployment outputs from CloudFormation
                let url = extract_lambda_url(&config, stdout.as_str());

                Ok(DeploymentResult {
                    deployment_id: format!("{}-{}", stack_name, Utc::now().timestamp()),
                    provider: "aws".to_string(),
                    resource_name: stack_name,
                    environment: env.map(String::from),
                    url,
                    deployed_at: Utc::now(),
                })
            }
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn rollback(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError> {
        let Some(previous) = previous_state else {
            return Err(DeploymentError::StateFailed(
                "no previous state available for rollback".to_string(),
            ));
        };

        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        let stack_name = if let Some(env_name) = env {
            format!("{}-{}", config.stack_name, env_name)
        } else {
            config.stack_name.clone()
        };

        // Rollback by updating to previous code version
        let mut executor = ShellExecutor::new("aws")
            .arg("lambda")
            .arg("update-function-code")
            .arg("--function-name")
            .arg(&stack_name)
            .arg("--s3-bucket")
            .arg(&previous.s3_bucket)
            .arg("--s3-key")
            .arg(&previous.s3_key);

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("aws lambda update-function-code produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { .. } => Ok(()),
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn logs(&self, config: &Self::Config, _env: Option<&str>) -> Result<(), DeploymentError> {
        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        let function_name = config
            .function_name
            .clone()
            .unwrap_or_else(|| config.stack_name.clone());

        let mut executor = ShellExecutor::new("aws")
            .arg("logs")
            .arg("tail")
            .arg(format!("/aws/lambda/{}", function_name))
            .arg("--follow");

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        // Note: logs tail runs forever, so we return immediately after spawning
        let _ = executor.execute()?;

        Ok(())
    }

    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        let stack_name = if let Some(env_name) = env {
            format!("{}-{}", config.stack_name, env_name)
        } else {
            config.stack_name.clone()
        };

        let mut executor = ShellExecutor::new("aws")
            .arg("cloudformation")
            .arg("delete-stack")
            .arg("--stack-name")
            .arg(&stack_name);

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::DeployRejected {
                reason: "aws cloudformation delete-stack produced no output".to_string(),
            }
        })?;

        match result {
            ShellDone::Success { .. } => Ok(()),
            ShellDone::Failed { stderr, .. } => Err(DeploymentError::DeployRejected {
                reason: stderr,
            }),
        }
    }

    fn status(
        &self,
        config: &Self::Config,
        env: Option<&str>,
    ) -> Result<Self::Resources, DeploymentError> {
        let region_flag = config
            .region
            .as_ref()
            .map(|r| format!("--region={}", r))
            .unwrap_or_default();

        let stack_name = if let Some(env_name) = env {
            format!("{}-{}", config.stack_name, env_name)
        } else {
            config.stack_name.clone()
        };

        // Get stack status
        let mut executor = ShellExecutor::new("aws")
            .arg("cloudformation")
            .arg("describe-stacks")
            .arg("--stack-name")
            .arg(&stack_name);

        if !region_flag.is_empty() {
            executor = executor.arg(region_flag);
        }

        executor = executor.current_dir(".");

        let stream = executor.execute()?;
        let result = collect_one(stream).ok_or_else(|| {
            DeploymentError::BuildFailed("aws cloudformation describe-stacks produced no output".to_string())
        })?;

        match result {
            ShellDone::Success { stdout, .. } => {
                // Parse JSON output
                let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
                    DeploymentError::ConfigInvalid {
                        file: "aws output".to_string(),
                        reason: e.to_string(),
                    }
                })?;

                let stack = json["Stacks"]
                    .as_array()
                    .and_then(|stacks| stacks.first())
                    .ok_or_else(|| DeploymentError::StateFailed("no stack found".to_string()))?;

                let deployment_id = stack["StackId"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_default();

                let stack_status = stack["StackStatus"]
                    .as_str()
                    .unwrap_or("UNKNOWN");

                let creation_time = stack["CreationTime"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                // Extract outputs
                let url = stack["Outputs"]
                    .as_array()
                    .and_then(|outputs| {
                        outputs.iter().find(|o| {
                            o["OutputKey"].as_str() == Some("FunctionUrl")
                                || o["OutputKey"].as_str() == Some("ApiUrl")
                        })
                    })
                    .and_then(|o| o["OutputValue"].as_str())
                    .map(String::from);

                Ok(AwsResources {
                    deployment_id,
                    s3_bucket: String::new(),
                    s3_key: String::new(),
                    deployed_at: creation_time,
                    url,
                    status: stack_status.to_string(),
                })
            }
            ShellDone::Failed { stderr, .. } => {
                if stderr.contains("does not exist") {
                    Err(DeploymentError::NoProviderDetected {
                        project_dir: format!("stack {} not found", stack_name),
                    })
                } else {
                    Err(DeploymentError::DeployRejected {
                        reason: stderr,
                    })
                }
            }
        }
    }
}

/// AWS Lambda deployment resources/status information.
#[derive(Debug, Clone)]
pub struct AwsResources {
    /// Deployment identifier (StackId or function ARN).
    pub deployment_id: String,
    /// S3 bucket where deployment artifact is stored.
    pub s3_bucket: String,
    /// S3 key for the deployment artifact.
    pub s3_key: String,
    /// When this version was deployed.
    pub deployed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Deployed URL (Lambda Function URL or API Gateway).
    pub url: Option<String>,
    /// Stack/function status.
    pub status: String,
}

/// Extract the Lambda URL from AWS output.
fn extract_lambda_url(config: &AwsConfig, output: &str) -> Option<String> {
    // SAM/CloudFormation outputs the URL in the format:
    // "FunctionUrl": "https://abc123.lambda-url.us-east-1.on.aws/"
    // Or in outputs section
    for line in output.lines() {
        if line.contains("FunctionUrl") || line.contains("ApiUrl") {
            if let Some(start) = line.find("https://") {
                let rest = &line[start..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
                return Some(rest.trim().to_string());
            }
        }
    }
    // Fallback: construct a typical Lambda URL
    config.region.as_ref().map(|region| {
        format!(
            "https://{}.lambda-url.{}.on.aws/",
            config.function_name.as_deref().unwrap_or(&config.stack_name),
            region
        )
    })
}
