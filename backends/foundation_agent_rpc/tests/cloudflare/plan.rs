//! The tests of the agent-rpc deployment plan (spec 83 feature 06, test case
//! 3).
//!
//! The plan is plain data. The tests prove that it names the instance Durable
//! Object, one container, and one secret reference for each secret, with no
//! secret value.

use foundation_agent_rpc::cloudflare::plan::{
    agent_rpc_plan, AgentRpcConfig, ContainerConfig, ARCHIVE_BINDING, DATABASE_BINDING,
    DEFAULT_INSTANCE_BINDING, DEFAULT_INSTANCE_CLASS, DEFAULT_MAX_INSTANCES,
};
use foundation_agent_rpc::core::model::ApiModelConfig;
use foundation_deployment_cloudflare::{Binding, ContainerImage, SecretSource};

fn model(name: &str, secret: &str) -> ApiModelConfig {
    ApiModelConfig {
        name: name.to_string(),
        provider: "workers-ai".to_string(),
        model_id: "@cf/meta/llama-3.1-8b-instruct".to_string(),
        base_url: None,
        api_key_secret: secret.to_string(),
        account_id: Some("account".to_string()),
    }
}

fn config() -> AgentRpcConfig {
    let mut config = AgentRpcConfig::new("cf-agent-rpc", "build/worker/shim.mjs");
    config.database_id = Some("database-id".to_string());
    config.archive_bucket = Some("agent-rpc-archive".to_string());
    config.routes = vec!["agents.example.com/rpc/*".to_string()];
    config.models = vec![
        model("llama", "WORKERS_AI_API_TOKEN"),
        model("gpt", "OPENAI_API_KEY"),
        // The same secret under another model is one plan secret.
        model("llama-turbo", "WORKERS_AI_API_TOKEN"),
    ];
    config
}

/// The plan of a valid configuration.
fn valid_plan(config: &AgentRpcConfig) -> foundation_deployment_cloudflare::DeploymentPlan {
    agent_rpc_plan(config).expect("the configuration is valid")
}

#[test]
fn cloudflare_plan_refuses_a_malformed_secret_reference() {
    // Oracle: issue #1003. A model that names malformed JSON is a typed
    // configuration error, not a plain secret name.
    let mut config = config();
    config.models = vec![model("gpt", "{\"kind\":")];
    let error = agent_rpc_plan(&config).expect_err("the secret reference is malformed");
    assert!(
        error.to_string().contains("api_key_secret"),
        "the error names the field: {error}"
    );
}

#[test]
fn cloudflare_plan_keeps_the_container_port_in_configuration() {
    // Oracle: issue #1006. The port is a configuration field with the default
    // 8080. A caller that changes it still gets a valid plan, and the same
    // value goes to `AgentContainer::call`.
    let default_config = AgentRpcConfig::default();
    assert_eq!(
        default_config.port,
        foundation_agent_rpc::core::config::DEFAULT_AGENT_RPC_PORT
    );
    assert_eq!(default_config.port, 8_080);

    let mut config = config();
    config.port = 9_090;
    let plan = valid_plan(&config);
    assert_eq!(plan.containers.len(), 1);
}

#[test]
fn cloudflare_plan_passes_the_spec_77_validation() {
    // Oracle: the spec 77 deployer reads this plan. Its validator accepts it:
    // one Worker, unique bindings, and a container class with a DO binding and
    // a migration.
    let plan = valid_plan(&config());
    foundation_deployment_cloudflare::validate(&plan).expect("the plan is valid");
}

#[test]
fn cloudflare_plan_names_the_agent_instance_do_class() {
    // Oracle: the DO binding, the migration and the container all name the
    // same class, so the spec 77 deployer accepts the plan.
    let plan = valid_plan(&config());
    assert_eq!(plan.worker.name, "cf-agent-rpc");
    assert_eq!(
        plan.worker.main,
        std::path::PathBuf::from("build/worker/shim.mjs")
    );

    let binding = plan
        .bindings
        .iter()
        .find(|binding| matches!(binding, Binding::DurableObject { .. }))
        .expect("a Durable Object binding");
    assert_eq!(binding.name(), DEFAULT_INSTANCE_BINDING);
    assert_eq!(binding.class_name(), Some(DEFAULT_INSTANCE_CLASS));

    assert_eq!(plan.migrations.len(), 1);
    assert_eq!(plan.migrations[0].tag, "v1");
    assert_eq!(
        plan.migrations[0].new_sqlite_classes,
        vec![DEFAULT_INSTANCE_CLASS.to_string()]
    );

    assert!(plan.bindings.iter().any(|binding| matches!(
        binding,
        Binding::D1 { name, .. } if name == DATABASE_BINDING
    )));
    assert!(plan.bindings.iter().any(|binding| matches!(
        binding,
        Binding::R2 { name, .. } if name == ARCHIVE_BINDING
    )));
}

#[test]
fn cloudflare_plan_names_one_container_deployment() {
    // Oracle: the plan holds exactly one container, and it belongs to the
    // instance DO class.
    let plan = valid_plan(&config());
    assert_eq!(plan.containers.len(), 1);
    let container = &plan.containers[0];
    assert_eq!(container.class_name, DEFAULT_INSTANCE_CLASS);
    assert_eq!(container.name, "agent-rpc");
    assert_eq!(container.max_instances, DEFAULT_MAX_INSTANCES);
    assert_eq!(container.instance_type, "lite");
    assert_eq!(
        container.image,
        ContainerImage::Registry {
            tag: "ghcr.io/ewe/agent-rpc:latest".to_string(),
            digest: None,
        }
    );
    assert_eq!(plan.routes.len(), 1);
    assert_eq!(plan.routes[0].pattern, "agents.example.com/rpc/*");
}

#[test]
fn cloudflare_plan_holds_one_secret_ref_for_each_secret() {
    // Oracle: one `SecretRef` for each unique secret name, and the plan holds
    // no secret value: a `SecretRef` is a name and a source only.
    let plan = valid_plan(&config());
    let names: Vec<&str> = plan
        .secrets
        .iter()
        .map(|secret| secret.name.as_str())
        .collect();
    assert_eq!(names, vec!["OPENAI_API_KEY", "WORKERS_AI_API_TOKEN"]);
    assert!(
        plan.secrets
            .iter()
            .all(|secret| secret.source == SecretSource::WorkerSecret),
        "a plain name is a Worker secret"
    );
}

#[test]
fn cloudflare_plan_maps_a_secrets_store_binding() {
    // Oracle: a model that names a Secrets Store binding gives a plan secret
    // with that store, never a value.
    let mut config = config();
    config.models = vec![model(
        "gpt",
        "{\"kind\":\"secrets_store\",\"binding\":\"AI_KEYS\"}",
    )];
    let plan = valid_plan(&config);
    assert_eq!(plan.secrets.len(), 1);
    assert_eq!(plan.secrets[0].name, "AI_KEYS");
    assert_eq!(
        plan.secrets[0].source,
        SecretSource::SecretsStore {
            store: "AI_KEYS".to_string(),
        }
    );
}

#[test]
fn cloudflare_plan_without_a_container_has_no_container_deployment() {
    // Oracle: `container = None` skips the container; the DO stays.
    let mut config = AgentRpcConfig::new("cf-agent-rpc", "build/worker/shim.mjs");
    config.container = None;
    config.models = vec![model("llama", "WORKERS_AI_API_TOKEN")];
    let plan = valid_plan(&config);
    assert_eq!(plan.containers.len(), 0, "no container is deployed");
    assert!(plan
        .bindings
        .iter()
        .any(|binding| matches!(binding, Binding::DurableObject { .. })));
}

#[test]
fn cloudflare_plan_uses_a_dockerfile_when_one_is_configured() {
    // Oracle: a Dockerfile in the container config wins over the registry tag.
    let mut config = config();
    config.container = Some(ContainerConfig {
        dockerfile: Some(std::path::PathBuf::from("Dockerfile.agent-rpc")),
        ..ContainerConfig::default()
    });
    let plan = valid_plan(&config);
    assert_eq!(
        plan.containers[0].image,
        ContainerImage::Dockerfile(std::path::PathBuf::from("Dockerfile.agent-rpc"))
    );
}
