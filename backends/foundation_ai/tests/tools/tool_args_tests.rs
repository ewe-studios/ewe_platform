//! Proposal 15, item 12: `ToolArgs` reads a call's arguments the same way
//! whichever backend produced them, and `ToolCallResult::text` builds a plain
//! result.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::tool_impl::{ToolArgs, ToolCallResult, ToolError, ToolImpl};
use foundation_ai::agentic::tools::files::EditTool;
use foundation_ai::types::{json_value_to_arg_type, ArgType, TextContent, UserModelContent};
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;
use foundation_nativeapis::MemoryFs;
use serde::Deserialize;

fn args(pairs: &[(&str, ArgType)]) -> HashMap<String, ArgType> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn reason(err: ToolError) -> String {
    match err {
        ToolError::InvalidArguments { tool, reason } => {
            assert_eq!(tool, "t", "errors name the tool");
            reason
        }
        other => panic!("expected InvalidArguments, got {other:?}"),
    }
}

#[test]
fn strings() {
    let map = args(&[("s", ArgType::Text("hi".into())), ("n", ArgType::I64(1))]);
    let a = ToolArgs::new("t", &map);
    assert_eq!(a.str("s").unwrap(), "hi");
    assert_eq!(a.opt_str("s").unwrap(), Some("hi"));
    assert_eq!(a.opt_str("absent").unwrap(), None);
    assert!(reason(a.str("absent").unwrap_err()).contains("missing required argument `absent`"));
    assert!(reason(a.str("n").unwrap_err()).contains("`n` must be a string"));
}

#[test]
fn integers_in_every_spelling() {
    let map = args(&[
        ("i64", ArgType::I64(-4)),
        ("u64", ArgType::U64(7)),
        ("usize", ArgType::Usize(3)),
        ("text", ArgType::Text(" 12 ".into())),
        ("json", ArgType::JSON("5".into())),
        ("bad", ArgType::Text("lots".into())),
        ("huge", ArgType::U64(u64::MAX)),
    ]);
    let a = ToolArgs::new("t", &map);
    assert_eq!(a.i64("i64").unwrap(), -4);
    assert_eq!(a.i64("u64").unwrap(), 7);
    assert_eq!(a.i64("usize").unwrap(), 3);
    assert_eq!(a.i64("text").unwrap(), 12);
    assert_eq!(a.opt_i64("json").unwrap(), Some(5));
    assert_eq!(a.opt_i64("absent").unwrap(), None);
    assert!(reason(a.i64("bad").unwrap_err()).contains("must be an integer"));
    assert!(reason(a.i64("huge").unwrap_err()).contains("must be an integer"));

    assert_eq!(a.usize("u64").unwrap(), 7);
    assert_eq!(a.opt_usize("absent").unwrap(), None);
    assert!(reason(a.usize("i64").unwrap_err()).contains("non-negative"));
}

#[test]
fn floats() {
    let map = args(&[
        ("f", ArgType::Float64(2.5)),
        ("i", ArgType::I64(2)),
        ("text", ArgType::Text("0.25".into())),
        ("bad", ArgType::Text("x".into())),
    ]);
    let a = ToolArgs::new("t", &map);
    assert!((a.f64("f").unwrap() - 2.5).abs() < f64::EPSILON);
    assert!((a.f64("i").unwrap() - 2.0).abs() < f64::EPSILON);
    assert!((a.f64("text").unwrap() - 0.25).abs() < f64::EPSILON);
    assert!(reason(a.f64("bad").unwrap_err()).contains("must be a number"));
}

#[test]
fn booleans_from_json_or_text() {
    let map = args(&[
        ("json", ArgType::JSON("true".into())),
        ("text", ArgType::Text("false".into())),
        ("bad", ArgType::Text("yes".into())),
        ("num", ArgType::I64(1)),
    ]);
    let a = ToolArgs::new("t", &map);
    assert!(a.bool("json").unwrap());
    assert_eq!(a.opt_bool("text").unwrap(), Some(false));
    assert_eq!(a.opt_bool("absent").unwrap(), None);
    assert!(reason(a.opt_bool("bad").unwrap_err()).contains("must be a boolean"));
    assert!(reason(a.bool("num").unwrap_err()).contains("must be a boolean"));
}

#[test]
fn values_and_whole_struct_parsing() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Call {
        path: String,
        count: i64,
        #[serde(default)]
        flag: bool,
        tags: Vec<String>,
        nested: Nested,
    }
    #[derive(Debug, Deserialize, PartialEq)]
    struct Nested {
        depth: u32,
    }

    // Built the way a backend builds them: from the model's JSON.
    let raw = serde_json::json!({
        "path": "a.txt",
        "count": 3,
        "flag": true,
        "tags": ["x", "y"],
        "nested": {"depth": 2}
    });
    let map: HashMap<String, ArgType> = raw
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), json_value_to_arg_type(v)))
        .collect();
    assert!(matches!(map.get("nested"), Some(ArgType::JSONMap(_))));

    let a = ToolArgs::new("t", &map);
    assert_eq!(a.opt_value("tags"), Some(serde_json::json!(["x", "y"])));
    assert_eq!(
        a.parse::<Call>().unwrap(),
        Call {
            path: "a.txt".into(),
            count: 3,
            flag: true,
            tags: vec!["x".into(), "y".into()],
            nested: Nested { depth: 2 },
        }
    );

    let missing = args(&[("path", ArgType::Text("a".into()))]);
    let err = ToolArgs::new("t", &missing).parse::<Call>().unwrap_err();
    assert!(reason(err).contains("missing field"));
}

#[test]
fn tool_call_result_text() {
    let result = ToolCallResult::text("done");
    assert!(matches!(
        result.content,
        UserModelContent::Text(TextContent { ref content, signature: None }) if content == "done"
    ));
    assert!(result.error_detail.is_none());
}

/// `edit` reads `replace_all` the same way from every backend: a JSON boolean
/// (OpenAI / Anthropic) or the text `"true"` (some local models).
#[test]
fn edit_accepts_replace_all_in_either_spelling() {
    futures_lite::future::block_on(async {
        for flag in [ArgType::JSON("true".into()), ArgType::Text("true".into())] {
            let fs = Arc::new(MemoryFs::new());
            fs.write_file_async("f.txt".into(), b"a a a".to_vec())
                .await
                .expect("seed file");
            let tool = EditTool::new(Arc::clone(&fs));
            let result = tool
                .execute(args(&[
                    ("path", ArgType::Text("f.txt".into())),
                    ("old_string", ArgType::Text("a".into())),
                    ("new_string", ArgType::Text("b".into())),
                    ("replace_all", flag.clone()),
                ]))
                .await
                .unwrap_or_else(|e| panic!("replace_all {flag:?} must be accepted: {e}"));
            assert!(matches!(
                result.content,
                UserModelContent::Text(TextContent { ref content, .. })
                    if content.contains("replaced 3")
            ));
            let bytes = fs.read_file_async("f.txt".into()).await.expect("read back");
            assert_eq!(bytes, b"b b b");
        }
    });
}

// ---------------------------------------------------------------------------
// Item 13 — closure tools
// ---------------------------------------------------------------------------

fn greet_tool() -> foundation_ai::agentic::FnTool {
    use foundation_ai::agentic::FnTool;
    use foundation_ai::types::Args;
    use foundation_jsonschema::scheme;

    FnTool::new(
        "greet",
        "Greet someone by name.",
        Args::new(scheme::object().required("name", scheme::string()).build()),
        |args: ToolArgs<'_>| {
            let name = args.str("name").map(str::to_owned);
            async move { Ok(ToolCallResult::text(format!("Hello, {}!", name?))) }
        },
    )
}

#[test]
fn fn_tool_declares_itself_and_runs_the_closure() {
    let tool = greet_tool();
    let def = tool.definition();
    assert_eq!(def.name(), "greet");
    assert_eq!(
        def.category(),
        Some("custom"),
        "category defaults to custom"
    );

    let out = futures_lite::future::block_on(
        tool.execute(args(&[("name", ArgType::Text("Ada".into()))])),
    )
    .expect("greets");
    assert!(matches!(
        out.content,
        UserModelContent::Text(TextContent { ref content, .. }) if content == "Hello, Ada!"
    ));

    // Argument errors from the closure surface as tool errors.
    let err = futures_lite::future::block_on(tool.execute(HashMap::new())).unwrap_err();
    assert!(matches!(
        err,
        ToolError::InvalidArguments { ref tool, ref reason }
            if tool == "greet" && reason.contains("`name`")
    ));

    let renamed = greet_tool().with_category("social");
    assert_eq!(renamed.definition().category(), Some("social"));
}

#[test]
fn fn_tool_goes_into_a_toolshed_and_validates_through_the_manager() {
    use foundation_ai::agentic::{AgentSession, ToolCallRequest, ToolShed};
    use foundation_ai::types::{ExecutionHint, ProviderRouter};

    let session = AgentSession::builder(ProviderRouter::builder().build())
        .with_toolshed(ToolShed::new().tool(greet_tool()))
        .build()
        .expect("session builds");
    let manager = session.tool_manager();
    assert!(manager.names().contains(&"greet".to_string()));

    let request = |arguments| ToolCallRequest {
        id: "c1".into(),
        name: "greet".into(),
        arguments,
        depends_on: Vec::new(),
        execution_hint: ExecutionHint::default(),
    };
    let ok = futures_lite::future::block_on(
        manager.execute_one(&request(args(&[("name", ArgType::Text("Bo".into()))]))),
    )
    .expect("runs");
    assert!(matches!(
        ok.content,
        UserModelContent::Text(TextContent { ref content, .. }) if content == "Hello, Bo!"
    ));
    // The schema is enforced before the closure runs.
    let err = futures_lite::future::block_on(
        manager.execute_one(&request(args(&[("name", ArgType::I64(3))]))),
    )
    .unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments { .. }), "{err:?}");
}
