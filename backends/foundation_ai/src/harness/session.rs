//! Session harness - provides pre-configured agent sessions for various models.

use foundation_db::{MemoryDocumentStore, MemoryStorage};

use crate::agentic::memory_store::KvMemoryStore;
use crate::agentic::session::AgentSession;
use crate::types::{ModelId, ProviderRouter, SessionId, ToolShed};

use super::providers::{CloudPresets, Glm52};

type DefaultDocStore = MemoryDocumentStore;
type DefaultMemoryStore = KvMemoryStore<MemoryStorage>;
type DefaultSession = AgentSession<DefaultDocStore, DefaultMemoryStore>;

/// Builder for creating pre-configured agent sessions.
pub struct SessionHarness {
    session_id: SessionId,
    model: Option<ModelId>,
    system_prompt: Option<String>,
}

impl SessionHarness {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session_id: SessionId::new(),
            model: None,
            system_prompt: None,
        }
    }

    #[must_use]
    pub fn claude_opus(api_key: &str) -> Self {
        Self::new()
            .with_model("claude-opus-4-6")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn claude_sonnet(api_key: &str) -> Self {
        Self::new()
            .with_model("claude-sonnet-4-6")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn openai_gpt4(api_key: &str) -> Self {
        Self::new()
            .with_model("gpt-4")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn openai_gpt4o(api_key: &str) -> Self {
        Self::new()
            .with_model("gpt-4o")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn glm5_2() -> Self {
        Self::new()
            .with_model(Glm52::MODEL_ID)
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn qwen3_6() -> Self {
        Self::new()
            .with_model("unsloth/Qwen3.6-35B-A3B-GGUF")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn ornith10() -> Self {
        Self::new()
            .with_model("LordNeel/Ornith-1.0-35B-GGUF-llamacpp-tp1")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn gemma4_e4b() -> Self {
        Self::new()
            .with_model("unsloth/gemma-4-E4B-it-GGUF")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn gemma4_26b() -> Self {
        Self::new()
            .with_model("unsloth/gemma-4-26B-A4B-it-GGUF")
            .with_system_prompt("You are a helpful coding assistant.")
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(ModelId::Name(model.into(), None));
        self
    }

    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn build_router(&self) -> Result<ProviderRouter, String> {
        let model = self.model.as_ref().ok_or("No model specified")?;
        let model_name = model.name();

        if model_name.contains("claude") {
            CloudPresets::claude_sonnet("API_KEY_REQUIRED")
                .map(|p| {
                    let routable = crate::types::RoutableProviderBox::new(p);
                    ProviderRouter::single(Box::new(routable))
                })
                .map_err(|e| format!("Failed to create Anthropic provider: {e}"))
        } else if model_name.contains("gpt") {
            CloudPresets::openai_gpt4o("API_KEY_REQUIRED")
                .map(|p| {
                    let routable = crate::types::RoutableProviderBox::new(p);
                    ProviderRouter::single(Box::new(routable))
                })
                .map_err(|e| format!("Failed to create OpenAI provider: {e}"))
        } else {
            Glm52::q4_k_m(None)
                .map(|p| {
                    let routable = crate::types::RoutableProviderBox::new(p);
                    ProviderRouter::single(Box::new(routable))
                })
                .map_err(|e| format!("Failed to create GGUF provider: {e}"))
        }
    }

    pub fn build(self) -> Result<DefaultSession, String> {
        let router = self.build_router()?;
        let _toolshed = ToolShed::default();

        let mut builder =
            AgentSession::<DefaultDocStore, DefaultMemoryStore>::builder(self.session_id, router);

        if let Some(ref prompt) = self.system_prompt {
            builder = builder.with_system_prompt(prompt.clone());
        }

        builder.build().map_err(|e| format!("Failed to build session: {e}"))
    }
}

impl Default for SessionHarness {
    fn default() -> Self {
        Self::new()
    }
}
