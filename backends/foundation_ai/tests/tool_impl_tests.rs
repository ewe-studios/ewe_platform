use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::*;
use foundation_ai::types::{ArgType, Args, ExecutionHint, SessionId, TextContent, UserModelContent};

struct EchoTool;

#[async_trait]
impl ToolImpl for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "Echoes the message argument".into(),
            arguments: Args::from_value(serde_json::json!({})),
            category: "shell".into(),
        }
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let msg = arguments
            .get("message")
            .and_then(|v| match v {
                ArgType::Text(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("echo: {msg}"),
                signature: None,
            }),
            error_detail: None,
        })
    }
}

#[test]
fn register_and_execute() {
    futures_lite::future::block_on(async {
        let mgr = ToolCallManager::new(SessionId::new());
        mgr.register(Arc::new(EchoTool));

        assert_eq!(mgr.names(), vec!["echo"]);
        assert!(mgr.get("echo").is_some());
        assert!(mgr.get("nonexistent").is_none());

        let request = ToolCallRequest {
            id: "call-1".into(),
            name: "echo".into(),
            arguments: HashMap::from([("message".into(), ArgType::Text("hello".into()))]),
            depends_on: vec![],
            execution_hint: ExecutionHint::default(),
        };
        let result = mgr.execute_one(&request).await.unwrap();
        if let UserModelContent::Text(tc) = result.content {
            assert_eq!(tc.content, "echo: hello");
        }

        mgr.deregister("echo");
        assert!(mgr.get("echo").is_none());
        assert_eq!(mgr.names().len(), 0);
    })
}

#[test]
fn unknown_tool_returns_error() {
    futures_lite::future::block_on(async {
        let mgr = ToolCallManager::new(SessionId::new());
        let request = ToolCallRequest {
            id: "call-1".into(),
            name: "nonexistent".into(),
            arguments: HashMap::new(),
            depends_on: vec![],
            execution_hint: ExecutionHint::default(),
        };
        let err = mgr.execute_one(&request).await.unwrap_err();
        assert_eq!(err, ToolError::UnknownTool("nonexistent".into()));
    })
}

#[test]
fn build_toolshed_populates_by_category() {
    futures_lite::future::block_on(async {
        let mgr = ToolCallManager::new(SessionId::new());
        mgr.register(Arc::new(EchoTool));

        let shed = mgr.build_toolshed();
        assert_eq!(shed.shell.as_ref().unwrap().name, "echo");
        assert_eq!(
            shed.shell.as_ref().unwrap().description,
            "Echoes the message argument"
        );
        assert!(shed.read.is_none());
    })
}

#[test]
fn build_toolshed_empty_has_only_shed() {
    futures_lite::future::block_on(async {
        let mgr = ToolCallManager::new(SessionId::new());
        let shed = mgr.build_toolshed();
        assert_eq!(shed.shed.as_ref().unwrap().name, "shed");
        assert!(shed.shell.is_none());
        assert!(shed.read.is_none());
    })
}
