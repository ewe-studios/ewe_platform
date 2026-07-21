//! Standard file tools (read/write/edit) over an in-memory VFS — spec-60 F05–F07.
//! Offline and deterministic: `MemoryFs` is the swappable FS base.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::tool_impl::{ToolError, ToolImpl};
use foundation_ai::agentic::tools::files::{EditTool, ReadTool, WriteTool};
use foundation_ai::types::ArgType;
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;
use foundation_nativeapis::MemoryFs;

fn args(pairs: &[(&str, &str)]) -> HashMap<String, ArgType> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), ArgType::Text((*v).to_string())))
        .collect()
}

fn text_of(r: &foundation_ai::agentic::tool_impl::ToolCallResult) -> String {
    match &r.content {
        foundation_ai::types::UserModelContent::Text(t) => t.content.clone(),
        foundation_ai::types::UserModelContent::Image(_) => String::new(),
    }
}

/// Seed a MemoryFs with a file at `path`.
async fn fs_with(path: &str, content: &str) -> Arc<MemoryFs> {
    let fs = Arc::new(MemoryFs::new());
    fs.write_file_async(path.to_string(), content.as_bytes().to_vec())
        .await
        .expect("seed write");
    fs
}

// ---------------------------------------------------------------------------
// ReadTool

#[test]
fn read_whole_file() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/a.txt", "hello\nworld\n").await;
        let tool = ReadTool::new(fs);
        let out = tool
            .execute(args(&[("path", "/a.txt")]))
            .await
            .expect("read ok");
        assert_eq!(text_of(&out), "hello\nworld\n");
    });
}

#[test]
fn read_line_range() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/a.txt", "l1\nl2\nl3\nl4\n").await;
        let tool = ReadTool::new(fs);
        let mut a = args(&[("path", "/a.txt")]);
        a.insert("offset".into(), ArgType::Usize(2));
        a.insert("limit".into(), ArgType::Usize(2));
        let out = tool.execute(a).await.expect("read ok");
        assert_eq!(text_of(&out), "l2\nl3");
    });
}

#[test]
fn read_missing_file_errors() {
    futures_lite::future::block_on(async {
        let fs = Arc::new(MemoryFs::new());
        let tool = ReadTool::new(fs);
        let err = tool.execute(args(&[("path", "/nope")])).await.unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn read_missing_path_arg_errors() {
    futures_lite::future::block_on(async {
        let tool = ReadTool::new(Arc::new(MemoryFs::new()));
        let err = tool.execute(HashMap::new()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidArguments { .. }));
    });
}

// ---------------------------------------------------------------------------
// WriteTool

#[test]
fn write_creates_then_read_back() {
    futures_lite::future::block_on(async {
        let fs = Arc::new(MemoryFs::new());
        let write = WriteTool::new(fs.clone());
        let out = write
            .execute(args(&[("path", "/new.txt"), ("content", "data")]))
            .await
            .expect("write ok");
        assert!(text_of(&out).contains("4 bytes"));

        let read = ReadTool::new(fs);
        let back = read.execute(args(&[("path", "/new.txt")])).await.expect("read");
        assert_eq!(text_of(&back), "data");
    });
}

#[test]
fn write_overwrites() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/f.txt", "old").await;
        WriteTool::new(fs.clone())
            .execute(args(&[("path", "/f.txt"), ("content", "new content")]))
            .await
            .expect("overwrite");
        let back = ReadTool::new(fs).execute(args(&[("path", "/f.txt")])).await.unwrap();
        assert_eq!(text_of(&back), "new content");
    });
}

// ---------------------------------------------------------------------------
// EditTool

#[test]
fn edit_unique_replace() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/e.txt", "foo bar baz").await;
        let edit = EditTool::new(fs.clone());
        edit.execute(args(&[
            ("path", "/e.txt"),
            ("old_string", "bar"),
            ("new_string", "QUX"),
        ]))
        .await
        .expect("edit ok");
        let back = ReadTool::new(fs).execute(args(&[("path", "/e.txt")])).await.unwrap();
        assert_eq!(text_of(&back), "foo QUX baz");
    });
}

#[test]
fn edit_absent_string_errors() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/e.txt", "hello").await;
        let err = EditTool::new(fs)
            .execute(args(&[
                ("path", "/e.txt"),
                ("old_string", "missing"),
                ("new_string", "x"),
            ]))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Execution { .. }));
    });
}

#[test]
fn edit_nonunique_without_replace_all_errors() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/e.txt", "a a a").await;
        let err = EditTool::new(fs)
            .execute(args(&[
                ("path", "/e.txt"),
                ("old_string", "a"),
                ("new_string", "b"),
            ]))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Execution { ref reason, .. } if reason.contains("not unique")),
            "non-unique without replace_all must error: {err:?}"
        );
    });
}

#[test]
fn edit_replace_all() {
    futures_lite::future::block_on(async {
        let fs = fs_with("/e.txt", "a a a").await;
        let mut a = args(&[
            ("path", "/e.txt"),
            ("old_string", "a"),
            ("new_string", "b"),
            ("replace_all", "true"),
        ]);
        a.insert("replace_all".into(), ArgType::Text("true".into()));
        EditTool::new(fs.clone()).execute(a).await.expect("replace_all ok");
        let back = ReadTool::new(fs).execute(args(&[("path", "/e.txt")])).await.unwrap();
        assert_eq!(text_of(&back), "b b b");
    });
}

/// The tool `definition()` exposes the right name + category (fills its ToolShed slot).
#[test]
fn tool_definitions_expose_names_and_categories() {
    let fs = Arc::new(MemoryFs::new());
    assert_eq!(ReadTool::new(fs.clone()).definition().name, "read");
    assert_eq!(ReadTool::new(fs.clone()).definition().category, "read");
    assert_eq!(WriteTool::new(fs.clone()).definition().name, "write");
    assert_eq!(WriteTool::new(fs.clone()).definition().category, "write");
    assert_eq!(EditTool::new(fs.clone()).definition().name, "edit");
    assert_eq!(EditTool::new(fs).definition().category, "edit");
}

// ---------------------------------------------------------------------------
// BashTool (F08) — native shell exec

#[cfg(not(target_family = "wasm"))]
mod bash {
    use super::*;
    use foundation_ai::agentic::tools::files::BashTool;

    #[test]
    fn bash_captures_stdout_and_exit() {
        futures_lite::future::block_on(async {
            let tool = BashTool::new();
            let out = tool
                .execute(args(&[("command", "echo hello-from-bash")]))
                .await
                .expect("bash ok");
            let text = text_of(&out);
            assert!(text.contains("hello-from-bash"), "stdout captured: {text}");
            assert!(text.contains("exit_code: 0"), "exit code captured: {text}");
        });
    }

    #[test]
    fn bash_captures_nonzero_exit_and_stderr() {
        futures_lite::future::block_on(async {
            let tool = BashTool::new();
            let out = tool
                .execute(args(&[("command", "echo oops >&2; exit 3")]))
                .await
                .expect("bash runs");
            let text = text_of(&out);
            assert!(text.contains("exit_code: 3"), "nonzero exit: {text}");
            assert!(text.contains("oops"), "stderr captured: {text}");
        });
    }

    #[test]
    fn bash_times_out() {
        futures_lite::future::block_on(async {
            let tool = BashTool::with_timeout(std::time::Duration::from_millis(200));
            let err = tool
                .execute(args(&[("command", "sleep 5")]))
                .await
                .unwrap_err();
            assert!(
                matches!(err, ToolError::Execution { ref reason, .. } if reason.contains("timed out")),
                "a slow command must time out: {err:?}"
            );
        });
    }

    #[test]
    fn bash_missing_command_errors() {
        futures_lite::future::block_on(async {
            let err = BashTool::new().execute(HashMap::new()).await.unwrap_err();
            assert!(matches!(err, ToolError::InvalidArguments { .. }));
        });
    }
}

// ---------------------------------------------------------------------------
// F09 — registering the tools fills the ToolShed slots

#[test]
fn registering_file_tools_fills_shed_slots() {
    use foundation_ai::agentic::tool_impl::ToolCallManager;
    use foundation_ai::agentic::tools::files::{register_file_tools, register_shell_tool};
    use foundation_ai::types::SessionId;

    let mgr = ToolCallManager::new(SessionId::new());
    register_file_tools(&mgr, Arc::new(MemoryFs::new()));
    register_shell_tool(&mgr);

    let shed = mgr.build_toolshed();
    assert!(shed.read.is_some(), "read slot filled");
    assert!(shed.write.is_some(), "write slot filled");
    assert!(shed.edit.is_some(), "edit slot filled");
    assert!(shed.shell.is_some(), "shell slot filled");
    // shed meta-tool appears once real tools exist.
    assert!(shed.shed.is_some(), "shed meta-tool present");
    assert_eq!(shed.read.as_ref().unwrap().name, "read");
    assert_eq!(shed.shell.as_ref().unwrap().name, "bash");
}

#[test]
fn registered_read_tool_executes_via_manager() {
    use foundation_ai::agentic::tool_impl::{ToolCallManager, ToolCallRequest};
    use foundation_ai::agentic::tools::files::register_file_tools;
    use foundation_ai::types::SessionId;

    futures_lite::future::block_on(async {
        let fs = Arc::new(MemoryFs::new());
        fs.write_file_async("/x.txt".to_string(), b"via-manager".to_vec())
            .await
            .unwrap();
        let mgr = ToolCallManager::new(SessionId::new());
        register_file_tools(&mgr, fs);

        let req = ToolCallRequest {
            id: "call-1".to_string(),
            name: "read".to_string(),
            arguments: args(&[("path", "/x.txt")]),
            depends_on: Vec::new(),
            execution_hint: Default::default(),
        };
        let out = mgr.execute_one(&req).await.expect("manager executes read");
        assert_eq!(text_of(&out), "via-manager");
    });
}
