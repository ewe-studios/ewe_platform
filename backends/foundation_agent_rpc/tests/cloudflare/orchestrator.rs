//! The tests of the thin orchestrator and the scope-in-name rule (spec 83
//! feature 06, test cases 1, 2 and 6, and issue #987).
//!
//! The naming rule is tested on every host. The end-to-end cases run against a
//! deployed Worker: they read `CF_AGENT_RPC_URL` and `CF_AGENT_RPC_TOKEN` from
//! the environment, print the reason and return when one is absent. The
//! Miniflare and Cloudflare cases in continuous integration (CI) run them.

use std::sync::Arc;

use foundation_agent_rpc::core::registry::{object_key, Scope};
use foundation_agent_rpc::proto::v1 as pb;
use foundation_agent_rpc::proto::v1::agent_service::AgentServiceClient;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, Transport};
use foundation_context::Ctx;
use foundation_netio::HttpClientBuilder;

/// The URL of the deployed Worker, set in a CI run.
const URL_VAR: &str = "CF_AGENT_RPC_URL";
/// The bearer access secret of the CI run.
const TOKEN_VAR: &str = "CF_AGENT_RPC_TOKEN";
/// The instance name of the streaming case.
const INSTANCE_VAR: &str = "CF_AGENT_RPC_INSTANCE";
/// The model name of the streaming case.
const MODEL_VAR: &str = "CF_AGENT_RPC_MODEL";

#[test]
fn cf_agent_rpc_object_name_includes_the_scope() {
    // Oracle: issue #987. Two owners that choose one instance name reach two
    // Durable Objects, because the object name is `"<scope>/<name>"`.
    let owner_first = Scope::Owner("a".repeat(64));
    let owner_second = Scope::Owner("b".repeat(64));

    let key_first = object_key(&owner_first, "case-one");
    let key_second = object_key(&owner_second, "case-one");

    assert_eq!(key_first, format!("{}/case-one", "a".repeat(64)));
    assert_eq!(key_second, format!("{}/case-one", "b".repeat(64)));
    assert_ne!(
        key_first, key_second,
        "one name under two owners names two Durable Objects"
    );
    assert_eq!(object_key(&Scope::Shared, "case-one"), "shared/case-one");
}

#[test]
fn cf_agent_rpc_object_name_is_unambiguous() {
    // Oracle: a scope key holds no slash and an instance name holds no slash
    // (the registry allows a-z, 0-9 and `._-`), so the first slash separates
    // the scope from the name.
    let scope = Scope::Owner("f".repeat(64));
    let key = object_key(&scope, "my.instance_1");
    let (scope_part, name_part) = key
        .split_once('/')
        .expect("the object name holds one slash");
    assert_eq!(scope_part, "f".repeat(64));
    assert_eq!(name_part, "my.instance_1");
}

/// The URL of the deployed Worker, or `None` with the reason printed.
fn worker_url() -> Option<String> {
    let Ok(url) = std::env::var(URL_VAR) else {
        eprintln!("{URL_VAR} is not set; the Cloudflare end-to-end case is skipped");
        return None;
    };
    Some(url)
}

/// A client of the deployed Worker with `token`, or `None` with the reason
/// printed.
fn client_for(url: &str, token: &str) -> Option<(AgentServiceClient, Ctx)> {
    let options =
        ClientOptions::new().with_header("Authorization".to_string(), format!("Bearer {token}"));
    let transport: Arc<dyn Transport> =
        Arc::new(H1Transport::new(HttpClientBuilder::new().build()));
    match AgentServiceClient::new(transport, url, options) {
        Ok(client) => Some((client, Ctx::host())),
        Err(error) => {
            eprintln!("{URL_VAR} is not a valid Worker URL: {error}");
            None
        }
    }
}

/// A client with the standard bearer token.
fn client() -> Option<(AgentServiceClient, Ctx)> {
    let url = worker_url()?;
    let Ok(token) = std::env::var(TOKEN_VAR) else {
        eprintln!("{TOKEN_VAR} is not set; the Cloudflare end-to-end case is skipped");
        return None;
    };
    client_for(&url, &token)
}

#[test]
fn cf_agent_rpc_reports_health_over_connect() {
    // Oracle: test case 1. A typed unary request reaches the Worker through
    // `dispatch_cf` and the shared Connect router, and a typed response comes
    // back.
    let Some((client, ctx)) = client() else {
        return;
    };
    futures_lite::future::block_on(async {
        let response = client
            .health(
                RpcCtx::client(ctx),
                Request::new(pb::HealthRequest::default()),
            )
            .await
            .expect("the Worker answers Health");
        assert!(
            !response.msg.status.is_empty(),
            "the Worker reports a status"
        );
    });
}

#[test]
fn cf_agent_rpc_two_owners_one_name_see_two_instances() {
    // Oracle: issue #987. Two owners create one instance name. Each call
    // reaches its own Durable Object, so both creates succeed and each list
    // holds only that owner's instance. A shared object would refuse the
    // second create.
    let Some(url) = worker_url() else {
        return;
    };
    let (Ok(token_first), Ok(token_second)) = (
        std::env::var("CF_AGENT_RPC_TOKEN_A"),
        std::env::var("CF_AGENT_RPC_TOKEN_B"),
    ) else {
        eprintln!(
            "CF_AGENT_RPC_TOKEN_A or CF_AGENT_RPC_TOKEN_B is not set; the two-owner case is skipped"
        );
        return;
    };
    let Ok(model) = std::env::var(MODEL_VAR) else {
        eprintln!("{MODEL_VAR} is not set; the two-owner case is skipped");
        return;
    };
    let Some((owner_first, ctx_first)) = client_for(&url, &token_first) else {
        return;
    };
    let Some((owner_second, ctx_second)) = client_for(&url, &token_second) else {
        return;
    };
    futures_lite::future::block_on(async {
        let name = "case-shared";
        let create = pb::CreateInstanceRequest {
            name: name.to_string(),
            model,
            ..Default::default()
        };
        // An earlier run may have made the instance; only a shared object
        // would make the second create fail after a reset.
        let _ = owner_first
            .create_instance(
                RpcCtx::client(ctx_first.clone()),
                Request::new(create.clone()),
            )
            .await;
        owner_second
            .create_instance(RpcCtx::client(ctx_second.clone()), Request::new(create))
            .await
            .expect("the second owner reaches its own object");

        let first = list_names(owner_first, ctx_first).await;
        let second = list_names(owner_second, ctx_second).await;
        assert!(
            first.contains(&name.to_string()),
            "owner A sees its instance"
        );
        assert!(
            second.contains(&name.to_string()),
            "owner B sees its instance"
        );
    });
}

/// The instance names that one owner lists.
async fn list_names(client: AgentServiceClient, ctx: Ctx) -> Vec<String> {
    client
        .list_instances(
            RpcCtx::client(ctx),
            Request::new(pb::ListInstancesRequest::default()),
        )
        .await
        .expect("the owner lists its instances")
        .msg
        .instances
        .iter()
        .map(|instance| instance.name.clone())
        .collect()
}

#[test]
fn cf_agent_rpc_streams_a_workers_ai_answer() {
    // Oracle: test case 6. A client creates an instance and streams a chat
    // turn; at least one typed event arrives from the Workers AI provider.
    let Ok(instance) = std::env::var(INSTANCE_VAR) else {
        eprintln!("{INSTANCE_VAR} is not set; the streaming case is skipped");
        return;
    };
    let Ok(model) = std::env::var(MODEL_VAR) else {
        eprintln!("{MODEL_VAR} is not set; the streaming case is skipped");
        return;
    };
    let Some((client, ctx)) = client() else {
        return;
    };
    futures_lite::future::block_on(async {
        let create = pb::CreateInstanceRequest {
            name: instance.clone(),
            model,
            ..Default::default()
        };
        // A second run may find the instance from the first run.
        let _ = client
            .create_instance(RpcCtx::client(ctx.clone()), Request::new(create))
            .await;

        let chat = pb::ChatRequest {
            instance,
            prompt: "Say one word.".to_string(),
            ..Default::default()
        };
        let mut stream = client
            .chat_stream(RpcCtx::client(ctx), Request::new(chat))
            .await
            .expect("the Worker starts the turn");
        let mut events = 0_usize;
        while let Ok(Some(_event)) = stream.receive().await {
            events += 1;
        }
        assert!(events > 0, "tokens arrive as streamed events");
    });
}
