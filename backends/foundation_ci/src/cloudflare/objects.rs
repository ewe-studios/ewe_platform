//! List Cloudflare R2 objects one response page at a time.
use super::Bucket;
use crate::CiEnvironment;
use foundation_cloudflare_storage::{
    shared::ApiClient,
    storage::{r2_list_objects_request, R2ListObjectsArgs},
};
use foundation_netio::{shared::http::SimpleHeader, HttpClientBuilder, PreparedRequestBuilder};
use foundation_tasks::{OutputStream, TaskContext, TaskEnv, TaskError, TaskResult, TaskRunner};
use std::collections::BTreeSet;

/// Transport and endpoint for Cloudflare storage operations.
pub struct R2Endpoint {
    client: Box<dyn Fn() -> ApiClient + Send + Sync>,
    base: String,
}

impl R2Endpoint {
    /// Use the supplied transport and endpoint for object listings.
    #[must_use]
    pub fn new(client: ApiClient, base: String) -> Self {
        Self {
            client: Box::new(move || client.clone()),
            base,
        }
    }
}

pub(crate) fn install(runner: &TaskRunner) {
    if runner.context().get::<R2Endpoint>().is_none() {
        runner.context().store(R2Endpoint {
            client: Box::new(|| ApiClient::new(HttpClientBuilder::new().build())),
            base: "https://api.cloudflare.com/client/v4".to_owned(),
        });
    }
}

pub(super) async fn list(ctx: TaskContext, bucket: Bucket) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let account = host
        .text("CLOUDFLARE_ACCOUNT_ID")
        .ok_or_else(|| TaskError::operation("Set CLOUDFLARE_ACCOUNT_ID to list bucket objects"))?;
    let token = host
        .text("CLOUDFLARE_API_TOKEN")
        .ok_or_else(|| TaskError::operation("Set CLOUDFLARE_API_TOKEN to list bucket objects"))?;
    if !account.bytes().all(|ch| ch.is_ascii_alphanumeric())
        || !bucket
            .name
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
        || bucket.name.is_empty()
    {
        return Err(TaskError::operation(
            "Invalid Cloudflare account or R2 bucket name",
        ));
    }
    let transport = ctx.require::<R2Endpoint>()?;
    let client = (transport.client)();
    let out = &ctx.require::<TaskEnv>()?.output;
    let mut query = R2ListObjectsArgs {
        account_id: account,
        bucket_name: bucket.name,
        prefix: None,
        cursor: None,
    };
    let mut seen = BTreeSet::new();
    loop {
        let request = r2_list_objects_request(
            client.clone(),
            &query,
            &transport.base,
            Some(|builder: &mut PreparedRequestBuilder| {
                builder.set_header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
            }),
        );
        let page = match futures_util::future::select(
            Box::pin(request),
            Box::pin(ctx.cancellation().cancelled()),
        )
        .await
        {
            futures_util::future::Either::Left((result, _)) => {
                result
                    .map_err(|failure| {
                        // A response body can include remote account details.
                        match failure {
                            foundation_cloudflare_storage::shared::ApiError::HttpStatus {
                                code,
                                ..
                            } => TaskError::operation(format!(
                                "Cloudflare object listing returned HTTP {code}"
                            )),
                            _ => TaskError::operation(
                                "Cloudflare object listing failed before a page was available",
                            ),
                        }
                    })?
                    .body
            }
            futures_util::future::Either::Right(_) => return Err(TaskError::Cancelled),
        };
        if !page.success {
            return Err(TaskError::operation(
                "Cloudflare rejected the object listing",
            ));
        }
        for item in page.result {
            let mut line = serde_json::to_vec(&item).map_err(TaskError::operation)?;
            line.push(b'\n');
            out.write(OutputStream::Stdout, &line)
                .await
                .map_err(TaskError::operation)?;
        }
        let Some(info) = page.result_info else {
            return Ok(());
        };
        let next = info.cursor.filter(|cursor| !cursor.is_empty());
        if !info.is_truncated.unwrap_or(next.is_some()) {
            return Ok(());
        }
        let cursor =
            next.ok_or_else(|| TaskError::operation("Cloudflare omitted the next object cursor"))?;
        if !seen.insert(cursor.clone()) {
            return Err(TaskError::operation("Cloudflare repeated an object cursor"));
        }
        query.cursor = Some(cursor);
    }
}
