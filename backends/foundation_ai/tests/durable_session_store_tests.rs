//! The `foundation_ai` session log over Durable Streams (`session_uses_core`).
//!
//! The tests drive the real protocol core over the in-memory virtual file
//! system (VFS). They make no Hypertext Transfer Protocol (HTTP) call.
//!
//! Tests build the effect sources on purpose (spec 84, section 6, item 2).

#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use foundation_ai::agentic::message_api::MessageApi;
use foundation_ai::agentic::{AgentSession, DurableDocumentStore};
use foundation_ai::types::{
    MessageRole, Messages, ProviderRouter, SessionId, SessionRecord, TextContent, UserModelContent,
};
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::durable::{DurableStream, DurableStreamVfs, DurableStreams, MemoryVfs};
use futures_lite::future::block_on;

// ---------------------------------------------------------------------------
// Helpers

/// Build the context with a fixed clock and a seeded effect environment.
fn test_ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    foundation_core::env::install(&ctx, |ctx| foundation_core::env::simulated(ctx, 7))
        .expect("a seeded effect environment");
    ctx
}

/// Build a store over the given VFS, as the host does at its entry point.
fn store(vfs: &Arc<MemoryVfs>, ctx: &Ctx) -> DurableDocumentStore {
    let concrete: Arc<MemoryVfs> = Arc::clone(vfs);
    let vfs: Arc<dyn DurableStreamVfs> = concrete;
    let stream = DurableStream::new(vfs, ctx);
    DurableDocumentStore::new(DurableStreams::new(stream), ctx.clone())
}

/// One user message record.
fn user_record(text: &str) -> SessionRecord {
    SessionRecord::Conversation {
        message: Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            signature: None,
        },
    }
}

/// The stable id of a conversation record.
fn record_id(record: &SessionRecord) -> String {
    match record {
        SessionRecord::Conversation { message } => message.id().to_string(),
        other => panic!("expected a conversation record, got {other:?}"),
    }
}

/// Append `count` conversation records and flush them.
fn write_records(api: &MessageApi<DurableDocumentStore>, records: &[SessionRecord]) {
    for record in records {
        let _ = api.append(record.clone());
    }
    block_on(api.flush()).expect("flush to the durable stream");
}

// ---------------------------------------------------------------------------
// Tests

/// Records append to a durable stream and a replay returns the same records.
#[test]
fn append_and_replay_round_trip() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let store = Arc::new(store(&vfs, &ctx));
        let session = SessionId::from_name("durable-round-trip");
        let api = MessageApi::from_shared(session, Arc::clone(&store));

        let records = vec![user_record("one"), user_record("two"), user_record("three")];
        for record in &records {
            let _ = api.append(record.clone());
        }
        assert_eq!(api.flush().await.expect("flush"), 3);

        let replayed = api.all().await.expect("replay");
        assert_eq!(replayed, records);

        let newest = api.recent(2).await.expect("recent");
        assert_eq!(newest, vec![records[2].clone(), records[1].clone()]);
    });
}

/// The stream order is the append order.
#[test]
fn order_is_preserved() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let store = Arc::new(store(&vfs, &ctx));
        let session = SessionId::from_name("durable-order");
        let api = MessageApi::from_shared(session, store);

        let records: Vec<SessionRecord> = (0..20)
            .map(|index| user_record(&format!("message-{index}")))
            .collect();
        for record in &records {
            let _ = api.append(record.clone());
        }
        api.flush().await.expect("flush");

        let replayed = api.all().await.expect("replay");
        assert_eq!(replayed, records);
        for (index, record) in replayed.iter().enumerate() {
            match record {
                SessionRecord::Conversation {
                    message: Messages::User { content, .. },
                } => {
                    let UserModelContent::Text(text) = content else {
                        panic!("expected text content");
                    };
                    assert_eq!(text.content, format!("message-{index}"));
                }
                other => panic!("unexpected record {other:?}"),
            }
        }
    });
}

/// A replay from a document id returns that record and the records after it.
#[test]
fn scan_from_replays_at_a_document_id() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let store = Arc::new(store(&vfs, &ctx));
        let session = SessionId::from_name("durable-scan-from");
        let api = MessageApi::from_shared(session, store);

        let records: Vec<SessionRecord> = (0..4)
            .map(|index| user_record(&format!("record-{index}")))
            .collect();
        for record in &records {
            let _ = api.append(record.clone());
        }
        api.flush().await.expect("flush");

        let from = record_id(&records[1]);
        let resumed = api.scan_from(&from, 0).await.expect("scan from");
        assert_eq!(resumed, records[1..].to_vec());

        let bounded = api.scan_from(&from, 2).await.expect("bounded scan from");
        assert_eq!(bounded, records[1..3].to_vec());
    });
}

/// A branch reads the parent prefix, then diverges; the parent stays unchanged.
#[test]
fn fork_branch_reads_parent_prefix_and_diverges() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let store = Arc::new(store(&vfs, &ctx));
        let parent = SessionId::from_name("durable-fork-parent");
        let branch = SessionId::from_name("durable-fork-branch");

        let parent_api = MessageApi::from_shared(parent.clone(), Arc::clone(&store));
        let parent_records = vec![user_record("parent-one"), user_record("parent-two")];
        write_records(&parent_api, &parent_records);

        store
            .fork(&parent.to_string(), &branch.to_string(), None)
            .await
            .expect("fork at the parent tail");

        let branch_api = MessageApi::from_shared(branch, Arc::clone(&store));
        let inherited = branch_api.all().await.expect("branch replay");
        assert_eq!(inherited, parent_records);

        let branch_record = user_record("branch-three");
        write_records(&branch_api, std::slice::from_ref(&branch_record));

        let diverged = branch_api.all().await.expect("branch replay after append");
        assert_eq!(diverged.len(), 3);
        assert_eq!(diverged[..2], parent_records);
        assert_eq!(diverged[2], branch_record);

        let parent_after = parent_api.all().await.expect("parent replay");
        assert_eq!(parent_after, parent_records);
    });
}

/// A new store over the same VFS replays the session.
#[test]
fn reopened_store_replays_the_session() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let session = SessionId::from_name("durable-reopen");

        let records = vec![user_record("persisted-one"), user_record("persisted-two")];
        {
            let store = Arc::new(store(&vfs, &ctx));
            let api = MessageApi::from_shared(session.clone(), store);
            write_records(&api, &records);
        }

        // A fresh store over the same VFS, as a process restart builds it.
        let reopened = Arc::new(store(&vfs, &ctx));
        let api = MessageApi::from_shared(session, reopened);
        let replayed = api.all().await.expect("replay after reopen");
        assert_eq!(replayed, records);
    });
}

/// The `foundation_ai` session builds over the durable store and replays its log.
#[test]
fn session_uses_core() {
    block_on(async {
        let ctx = test_ctx();
        let vfs = Arc::new(MemoryVfs::new());
        let session_id = SessionId::from_name("session-uses-core");
        let store = store(&vfs, &ctx);

        let session = AgentSession::builder(ProviderRouter::builder().build(), ctx)
            .with_session_id(session_id)
            .with_model("test-model")
            .with_doc_store(store)
            .build()
            .await
            .expect("build the session over the durable store");

        let records = vec![user_record("hello"), user_record("world")];
        write_records(session.message_api(), &records);

        let replayed = session.message_api().all().await.expect("session replay");
        assert_eq!(replayed, records);
    });
}
