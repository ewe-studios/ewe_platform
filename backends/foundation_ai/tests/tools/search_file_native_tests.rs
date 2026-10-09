//! F18 — `search_file` routed through the native fff cascade
//! (`native_vfs_searcher`: fff → CLI rg/grep → in-code VFS walk). Native only.
//!
//! Proof strategy: `cargo test` runs with the crate dir as the working
//! directory, so searching `.` for a string that exists in this crate's
//! committed source must return real matches — which only the real-filesystem
//! cascade (fff/CLI) can find (the in-code searcher is backed by an empty
//! `MemoryFs` here).

#![cfg(not(target_family = "wasm"))]

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::tool_impl::ToolImpl;
use foundation_ai::agentic::tools::search::SearchFileTool;
use foundation_ai::types::ArgType;
use foundation_nativeapis::MemoryFs;

fn text_of(r: &foundation_ai::agentic::tool_impl::ToolCallResult) -> String {
    match &r.content {
        foundation_ai::types::UserModelContent::Text(t) => t.content.clone(),
        foundation_ai::types::UserModelContent::Image(_) => String::new(),
    }
}

#[test]
fn native_search_file_finds_repo_content_via_cascade() {
    futures_lite::future::block_on(async {
        // Empty in-memory FS as the fallback tier; the native cascade searches
        // the REAL filesystem rooted at the crate dir (cwd during `cargo test`).
        let tool = SearchFileTool::native(Arc::new(MemoryFs::new()), ".".to_string());

        // A token that exists in this crate's committed source (the F18 method).
        let mut args = HashMap::new();
        args.insert("query".to_string(), ArgType::Text("from_vfs_native".to_string()));
        args.insert("kind".to_string(), ArgType::Text("Grep".to_string()));

        let out = tool.execute(args).await.expect("native search runs");
        let json = text_of(&out);
        assert!(
            json.contains("from_vfs_native") || json.contains("search.rs"),
            "the native cascade must find the committed token in the crate source; got: {json}"
        );
        // A pure in-code searcher over the empty MemoryFs would return "[]".
        assert_ne!(json.trim(), "[]", "native cascade should not be empty over the real repo");
    });
}
