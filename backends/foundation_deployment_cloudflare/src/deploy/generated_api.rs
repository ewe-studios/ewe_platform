//! The `DeploymentApi` over the generated Cloudflare application programming
//! interface (API) calls.
//!
//! The container application create and update calls, and the Worker version
//! upload, use the generated functions of `foundation_cloudflare_workers`. The
//! routes, the traffic switch and the deployment state are not wired here: the
//! generated types model the response shape of those calls, not the request
//! shape, so this host reports them as unsupported instead of guessing a body.

use std::path::Path;

use foundation_auth::server_credential::Secret;
use foundation_netio::DynNetClient;
use foundation_netio::PreparedRequestBuilder;
use serde_json::{json, Value};

use crate::deploy::host::{DeployFuture, DeploymentApi};
use crate::deploy::plan::{Binding, ContainerDeployment, DoMigration, Route, WorkerDeployment};
use crate::deploy::{DeployError, DeploymentId, DeploymentStatus};
use crate::generated::workers::{
    public_create_application_request, public_list_applications_request,
    public_update_application_request, worker_put_script_secret_request,
    worker_versions_upload_version_request, CcCreateApplicationRequest, CcUpdateApplicationRequest,
    PublicCreateApplicationArgs, PublicListApplicationsArgs, PublicUpdateApplicationArgs,
    WorkerPutScriptSecretArgs, WorkerVersionsUploadVersionArgs,
};
use foundation_deployment::providers::common::{MultipartBody, MultipartPart};

/// The generated calls of the Cloudflare API.
pub struct GeneratedCloudflareApi {
    http: DynNetClient,
    account_id: String,
    base_url: String,
    token: Secret,
}

impl GeneratedCloudflareApi {
    /// A host over one transport, account and API token.
    #[must_use]
    pub fn new(
        http: DynNetClient,
        account_id: impl Into<String>,
        base_url: impl Into<String>,
        token: Secret,
    ) -> Self {
        Self {
            http,
            account_id: account_id.into(),
            base_url: base_url.into(),
            token,
        }
    }

    /// The `Authorization` header of one request.
    fn auth(&self) -> impl FnOnce(&mut PreparedRequestBuilder) + '_ {
        let token = self.token.expose().to_string();
        move |builder: &mut PreparedRequestBuilder| {
            builder.set_bearer_token(&token);
        }
    }

    /// The JSON metadata of one Worker version upload.
    fn version_metadata(worker: &WorkerDeployment, bindings: &[Binding]) -> Value {
        json!({
            "main_module": main_name(&worker.main),
            "compatibility_date": worker.compatibility_date,
            "bindings": bindings.iter().map(binding_json).collect::<Vec<_>>(),
        })
    }
}

impl DeploymentApi for GeneratedCloudflareApi {
    fn ensure_container(
        &self,
        _worker_name: &str,
        container: &ContainerDeployment,
        tag: &str,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        let container = container.clone();
        let tag = tag.to_string();
        Box::pin(async move {
            let list = public_list_applications_request(
                self.http.clone(),
                &PublicListApplicationsArgs {
                    account_id: self.account_id.clone(),
                    name: Some(container.name.clone()),
                    image: None,
                },
                &self.base_url,
                Some(self.auth()),
            )
            .await
            .map_err(api_error)?;
            let existing = list
                .body
                .result
                .into_iter()
                .find(|application| application.name == container.name);
            let image = tag;
            let instance_type = Some(container.instance_type.clone());
            let max_instances = Some(i64::from(container.max_instances));
            if let Some(application) = existing {
                public_update_application_request(
                    self.http.clone(),
                    &PublicUpdateApplicationArgs {
                        account_id: self.account_id.clone(),
                        application_id: application.id,
                        body: CcUpdateApplicationRequest {
                            image: Some(image),
                            instance_type,
                            max_instances,
                            ..CcUpdateApplicationRequest::default()
                        },
                    },
                    &self.base_url,
                    Some(self.auth()),
                )
                .await
                .map_err(api_error)?;
            } else {
                public_create_application_request(
                    self.http.clone(),
                    &PublicCreateApplicationArgs {
                        account_id: self.account_id.clone(),
                        body: CcCreateApplicationRequest {
                            name: container.name.clone(),
                            image,
                            instance_type,
                            max_instances,
                            ..CcCreateApplicationRequest::default()
                        },
                    },
                    &self.base_url,
                    Some(self.auth()),
                )
                .await
                .map_err(api_error)?;
            }
            Ok(())
        })
    }

    fn upload_version(
        &self,
        worker: &WorkerDeployment,
        bindings: &[Binding],
        _migrations: &[DoMigration],
    ) -> DeployFuture<'_, Result<String, DeployError>> {
        let worker = worker.clone();
        let bindings = bindings.to_vec();
        Box::pin(async move {
            let module = std::fs::read(&worker.main).map_err(|error| DeployError::Io {
                op: "read the Worker module".to_string(),
                message: error.to_string(),
            })?;
            let name = main_name(&worker.main);
            let metadata = Self::version_metadata(&worker, &bindings);
            let metadata_bytes =
                serde_json::to_vec(&metadata).map_err(|error| DeployError::Io {
                    op: "encode the version metadata".to_string(),
                    message: error.to_string(),
                })?;
            let body = MultipartBody::new()
                .with_part(
                    MultipartPart::new("metadata", metadata_bytes)
                        .with_content_type("application/json"),
                )
                .with_part(
                    MultipartPart::new("files", module)
                        .with_filename(name)
                        .with_content_type("application/javascript+module"),
                );
            let response = worker_versions_upload_version_request(
                self.http.clone(),
                &WorkerVersionsUploadVersionArgs {
                    account_id: self.account_id.clone(),
                    script_name: worker.name,
                    bindings_inherit: None,
                },
                &self.base_url,
                &body,
                Some(self.auth()),
            )
            .await
            .map_err(api_error)?;
            response.body.result.id.ok_or_else(|| DeployError::Api {
                status: 0,
                message: "the version upload named no version".to_string(),
            })
        })
    }

    fn put_secret(
        &self,
        worker_name: &str,
        _version: &str,
        name: &str,
        value: &Secret,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        let worker_name = worker_name.to_string();
        let name = name.to_string();
        let text = value.expose().to_string();
        Box::pin(async move {
            let body = json!({ "name": name, "text": text, "type": "secret_text" });
            worker_put_script_secret_request(
                self.http.clone(),
                &WorkerPutScriptSecretArgs {
                    account_id: self.account_id.clone(),
                    script_name: worker_name,
                    body,
                },
                &self.base_url,
                Some(self.auth()),
            )
            .await
            .map_err(api_error)?;
            Ok(())
        })
    }

    fn set_route(
        &self,
        _worker_name: &str,
        _route: &Route,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async {
            Err(DeployError::Unsupported {
                what: "set_route: the generated WorkersRoute type is the response shape"
                    .to_string(),
            })
        })
    }

    fn promote(
        &self,
        _worker_name: &str,
        _version: &str,
    ) -> DeployFuture<'_, Result<String, DeployError>> {
        Box::pin(async {
            Err(DeployError::Unsupported {
                what: "promote: the generated WorkersDeployment type is the response shape"
                    .to_string(),
            })
        })
    }

    fn status(
        &self,
        _id: &DeploymentId,
    ) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>> {
        Box::pin(async {
            Err(DeployError::Unsupported {
                what: "status: the generated WorkersDeployment type is the response shape"
                    .to_string(),
            })
        })
    }

    fn rollback(&self, _id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async {
            Err(DeployError::Unsupported {
                what: "rollback: the generated WorkersDeployment type is the response shape"
                    .to_string(),
            })
        })
    }
}

/// The file name of one Worker module.
fn main_name(main: &Path) -> String {
    main.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("index.js")
        .to_string()
}

/// One binding in the Worker version metadata.
fn binding_json(binding: &Binding) -> Value {
    match binding {
        Binding::DurableObject { name, class_name } => json!({
            "type": "durable_object_namespace",
            "name": name,
            "class_name": class_name,
        }),
        Binding::D1 { name, database_id } => json!({
            "type": "d1",
            "name": name,
            "id": database_id,
        }),
        Binding::Kv { name, namespace_id } => json!({
            "type": "kv_namespace",
            "name": name,
            "namespace_id": namespace_id,
        }),
        Binding::R2 { name, bucket_name } => json!({
            "type": "r2_bucket",
            "name": name,
            "bucket_name": bucket_name,
        }),
        Binding::Ai { name } => json!({ "type": "ai", "name": name }),
        Binding::Service { name, service } => json!({
            "type": "service",
            "name": name,
            "service": service,
        }),
        Binding::PlainText { name, value } => json!({
            "type": "plain_text",
            "name": name,
            "text": value,
        }),
    }
}

/// One generated API error as a deploy error.
fn api_error(error: foundation_deployment::providers::common::ApiError) -> DeployError {
    use foundation_deployment::providers::common::ApiError;
    match error {
        ApiError::HttpStatus { code, body, .. } => DeployError::Api {
            status: code,
            message: body.unwrap_or_default(),
        },
        other => DeployError::Api {
            status: 0,
            message: other.to_string(),
        },
    }
}
