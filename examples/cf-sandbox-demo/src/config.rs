//! The configuration and the limits of the demo (root rule 8).
//!
//! Every value reads a Worker var. The default is documented on the field. A
//! bad value stops the first request with a message that names the var, never
//! a secret value.

use worker::Env;

/// The settings of one demo request.
#[derive(Clone, Debug)]
pub struct DemoConfig {
    /// The most sandboxes that the page and the live test may create at once.
    /// Default 3: the three model paths of the demo.
    pub max_sandboxes: usize,
    /// The time limit of one command. Default 60 s: nginx's default
    /// `proxy_read_timeout`.
    pub exec_timeout_ms: u64,
    /// The most bytes that one file write accepts. Default 64 KiB: one nginx
    /// `large_client_header_buffers` buffer.
    pub file_max_bytes: usize,
    /// The most bytes of one prompt. Default 4 KiB: the gRPC default receive
    /// message size is 4 MiB, but a demo prompt is short.
    pub prompt_max_bytes: usize,
    /// The most arguments in one command. Default 32: the conventional
    /// `argv` bound of a small shell tool.
    pub command_max_argv: usize,
    /// The most bytes of one command. Default 4 KiB.
    pub command_max_bytes: usize,
    /// The most bytes of one control API JSON body. Default 128 KiB.
    pub control_body_max_bytes: usize,
    /// The most bytes of one error or unary answer that the demo reads.
    /// Default 2 KiB: enough for one Connect error message.
    pub error_text_max_bytes: usize,
    /// The port of the model server in the sandbox container. Default 8081.
    pub container_model_port: u16,
    /// The provider name that the container model reports. Default
    /// `container-model`.
    pub container_model_name: String,
    /// The Workers AI model id. Default `@cf/meta/llama-3.1-8b-instruct`.
    pub workers_ai_model: String,
    /// The OpenRouter base URL. Default `https://openrouter.ai/api`: the
    /// provider appends `/v1/chat/completions`.
    pub openrouter_base_url: String,
    /// The OpenRouter model id. Default `google/gemma-4-31b-it:free`: a free
    /// model, so one demo turn costs no money.
    pub openrouter_model: String,
    /// The system prompt of the OpenRouter conversation. Default: one short
    /// sentence.
    pub agent_system_prompt: String,
    /// The Worker secret that holds the OpenRouter key.
    pub openrouter_secret: String,
    /// The `AI` binding name of the Worker. Default `AI`.
    pub ai_binding: String,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            max_sandboxes: 3,
            exec_timeout_ms: 60_000,
            file_max_bytes: 64 * 1024,
            prompt_max_bytes: 4 * 1024,
            command_max_argv: 32,
            command_max_bytes: 4 * 1024,
            control_body_max_bytes: 128 * 1024,
            error_text_max_bytes: 2 * 1024,
            container_model_port: 8081,
            container_model_name: "container-model".to_string(),
            workers_ai_model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
            openrouter_base_url: "https://openrouter.ai/api".to_string(),
            openrouter_model: "google/gemma-4-31b-it:free".to_string(),
            agent_system_prompt:
                "You are a concise assistant. Answer in one short sentence.".to_string(),
            openrouter_secret: "OPENROUTER_API_KEY".to_string(),
            ai_binding: "AI".to_string(),
        }
    }
}

impl DemoConfig {
    /// The configuration from the Worker vars.
    ///
    /// # Errors
    /// A message that names the bad var.
    pub fn from_env(env: &Env) -> Result<Self, String> {
        let defaults = Self::default();
        Ok(Self {
            max_sandboxes: number(env, "MAX_SANDBOXES", defaults.max_sandboxes)?,
            exec_timeout_ms: number(env, "EXEC_TIMEOUT_MS", defaults.exec_timeout_ms as usize)?
                as u64,
            file_max_bytes: number(env, "FILE_MAX_BYTES", defaults.file_max_bytes)?,
            prompt_max_bytes: number(env, "PROMPT_MAX_BYTES", defaults.prompt_max_bytes)?,
            command_max_argv: number(env, "COMMAND_MAX_ARGV", defaults.command_max_argv)?,
            command_max_bytes: number(env, "COMMAND_MAX_BYTES", defaults.command_max_bytes)?,
            control_body_max_bytes: number(
                env,
                "CONTROL_BODY_MAX_BYTES",
                defaults.control_body_max_bytes,
            )?,
            error_text_max_bytes: number(
                env,
                "ERROR_TEXT_MAX_BYTES",
                defaults.error_text_max_bytes,
            )?,
            container_model_port: number(
                env,
                "CONTAINER_MODEL_PORT",
                usize::from(defaults.container_model_port),
            )? as u16,
            container_model_name: text(env, "CONTAINER_MODEL_NAME", defaults.container_model_name),
            workers_ai_model: text(env, "WORKERS_AI_MODEL", defaults.workers_ai_model),
            openrouter_base_url: text(env, "OPENROUTER_BASE_URL", defaults.openrouter_base_url),
            openrouter_model: text(env, "OPENROUTER_MODEL", defaults.openrouter_model),
            agent_system_prompt: text(env, "AGENT_SYSTEM_PROMPT", defaults.agent_system_prompt),
            openrouter_secret: text(env, "OPENROUTER_SECRET", defaults.openrouter_secret),
            ai_binding: text(env, "AI_BINDING", defaults.ai_binding),
        })
    }
}

/// A text var, or `default` when it is absent or empty.
fn text(env: &Env, name: &str, default: String) -> String {
    env.var(name)
        .map(|value| value.to_string())
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
}

/// A number var, or `default` when it is absent.
fn number(env: &Env, name: &str, default: usize) -> Result<usize, String> {
    let Some(value) = env.var(name).map(|value| value.to_string()).ok() else {
        return Ok(default);
    };
    value
        .trim()
        .parse::<usize>()
        .map_err(|error| format!("the Worker var {name} is not a number: {error}"))
}
