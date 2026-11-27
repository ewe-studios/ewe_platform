//! The document-level behaviour suite for `DurableDocumentStore`.
//!
//! WHY: the transport matrix `Driver` trait drives byte-stream operations
//! (create with a content type and a time to live, append raw bytes with a
//! producer and a `Stream-Seq`, read a byte offset, close and fork). A
//! `DurableDocumentStore` is document oriented: one session id is one durable
//! stream and one document is one JavaScript Object Notation (JSON) chunk. A
//! mapping would encode the bytes again, move the chunk boundaries and hide the
//! create status, the time to live and the `Stream-Seq` outcome. That mapping
//! would be a fake. This suite proves the document behaviour on its own terms.
//! See issue #1092.
//!
//! WHAT: every scenario is Given/When/Then. Each scenario keeps an independent
//! document model. It asserts the model first, then the store against it.
//!
//! HOW: the suite builds the store over `MemoryVfs` with `Ctx::host()`. It makes
//! no network call. A write uses an explicit document id, because the model
//! needs a stable id and the id source is not part of the document behaviour.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_ai::agentic::DurableDocumentStore;
use foundation_context::Ctx;
use foundation_core::durable::{DurableStream, DurableStreamVfs, DurableStreams, MemoryVfs};
use foundation_db::traits::{Document, DocumentStore};
use futures_lite::future::block_on;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Document model oracle

/// One document in the oracle history.
#[derive(Debug, Clone, PartialEq)]
struct ModelDocument {
    /// The stable document id.
    id: String,
    /// The document content as JSON.
    content: Value,
}

/// An independent model of the session document log.
///
/// The model holds one ordered history per session. A fork copies the parent
/// history at the fork point. A tail offset is the history length at that
/// moment, so a replay from it returns the later documents.
#[derive(Default)]
struct DocumentModel {
    /// The document history of each session key.
    histories: HashMap<String, Vec<ModelDocument>>,
}

impl DocumentModel {
    /// Add one document to the end of a session history.
    fn write(&mut self, key: &str, id: &str, content: Value) {
        self.histories
            .entry(key.to_string())
            .or_default()
            .push(ModelDocument {
                id: id.to_string(),
                content,
            });
    }

    /// Copy the parent history into a branch, as a fork at the parent tail.
    fn fork(&mut self, parent: &str, branch: &str) {
        let inherited = self.histories.get(parent).cloned().unwrap_or_default();
        self.histories.insert(branch.to_string(), inherited);
    }

    /// The history length, which a tail offset records.
    fn tail(&self, key: &str) -> usize {
        self.histories.get(key).map_or(0, Vec::len)
    }

    /// The documents from a history position to the end, in order.
    fn replay(&self, key: &str, from: usize) -> &[ModelDocument] {
        let history = self.histories.get(key).map_or(&[][..], Vec::as_slice);
        &history[from.min(history.len())..]
    }
}

// ---------------------------------------------------------------------------
// Store setup and checks

/// Build a store over a fresh `MemoryVfs`, as the host builds it.
fn store(ctx: &Ctx) -> DurableDocumentStore {
    let vfs: Arc<dyn DurableStreamVfs> = Arc::new(MemoryVfs::new());
    let stream = DurableStream::new(vfs, ctx);
    DurableDocumentStore::new(DurableStreams::new(stream), ctx.clone())
}

/// Write one document with an explicit id.
async fn write(store: &DurableDocumentStore, key: &str, id: &str, content: Value) {
    store
        .append_with_id(key, id, content)
        .await
        .expect("append the document");
}

/// Convert a stored document to the oracle form.
fn model_document(document: &Document) -> ModelDocument {
    ModelDocument {
        id: document.id.clone(),
        content: serde_json::from_str(&document.content).expect("the stored content is JSON"),
    }
}

/// Assert the store documents equal the oracle, after the model was checked.
fn assert_store_matches(documents: &[Document], expected: &[ModelDocument], scenario: &str) {
    let observed: Vec<ModelDocument> = documents.iter().map(model_document).collect();
    assert_eq!(
        observed, expected,
        "{scenario}: the store must match the document model"
    );
}

// ---------------------------------------------------------------------------
// Scenarios

/// Given a session, when a document is written and read, then the document
/// round trips.
#[test]
fn given_a_session_when_a_document_is_written_and_read_then_the_document_round_trips() {
    block_on(async {
        let ctx = Ctx::host();
        let store = store(&ctx);
        let mut model = DocumentModel::default();
        let session = "session-round-trip";
        let content = json!({"role": "user", "text": "hello"});

        // Model first: the session holds one document with this id and content.
        model.write(session, "doc-1", content.clone());
        let expected = [ModelDocument {
            id: "doc-1".to_string(),
            content: content.clone(),
        }];
        assert_eq!(model.replay(session, 0), &expected[..]);

        // Then the store: the write and the read return the same document.
        let written = store
            .append_with_id(session, "doc-1", content.clone())
            .await
            .expect("append the document");
        assert_eq!(written.id, "doc-1");
        assert_eq!(
            serde_json::from_str::<Value>(&written.content).expect("JSON"),
            content
        );

        let read = store
            .replay(session, None)
            .await
            .expect("replay the session");
        assert_store_matches(&read, model.replay(session, 0), "round trip");
    });
}

/// Given several documents, when the session replays from an offset, then it
/// returns exactly the later documents in order.
#[test]
fn given_several_documents_when_the_session_replays_from_an_offset_then_it_returns_exactly_the_later_documents_in_order(
) {
    block_on(async {
        let ctx = Ctx::host();
        let store = store(&ctx);
        let mut model = DocumentModel::default();
        let session = "session-offset";

        for index in 0..3 {
            let id = format!("doc-{index}");
            let content = json!({"seq": index});
            model.write(session, &id, content.clone());
            write(&store, session, &id, content).await;
        }

        // The tail offset after three documents is model position 3.
        let offset = store
            .tail_offset(session)
            .await
            .expect("read the tail offset")
            .expect("the session has a tail");
        let from = model.tail(session);
        assert_eq!(from, 3);
        // Model first: nothing follows the tail.
        assert_eq!(model.replay(session, from), []);
        let at_tail = store
            .replay(session, Some(offset.clone()))
            .await
            .expect("replay from the tail");
        assert_store_matches(&at_tail, model.replay(session, from), "empty tail replay");

        for index in 3..5 {
            let id = format!("doc-{index}");
            let content = json!({"seq": index});
            model.write(session, &id, content.clone());
            write(&store, session, &id, content).await;
        }

        // Model first: exactly the later documents, in order.
        let expected: Vec<ModelDocument> = (3..5)
            .map(|index| ModelDocument {
                id: format!("doc-{index}"),
                content: json!({"seq": index}),
            })
            .collect();
        assert_eq!(model.replay(session, from), expected.as_slice());
        assert_eq!(expected.len(), 2);

        // Then the store: the same ordered suffix.
        let observed = store
            .replay(session, Some(offset))
            .await
            .expect("replay from the offset");
        assert_store_matches(&observed, expected.as_slice(), "replay from an offset");
    });
}

/// Given a parent session at its tail, when a branch forks and writes, then the
/// branch reads the prefix and its own document, and the parent does not
/// change.
#[test]
fn given_a_parent_session_at_its_tail_when_a_branch_forks_and_writes_then_the_branch_reads_the_prefix_and_its_own_document_and_the_parent_does_not_change(
) {
    block_on(async {
        let ctx = Ctx::host();
        let store = store(&ctx);
        let mut model = DocumentModel::default();
        let parent = "session-fork-parent";
        let branch = "session-fork-branch";

        let prefix = [
            ModelDocument {
                id: "doc-0".to_string(),
                content: json!({"seq": 0}),
            },
            ModelDocument {
                id: "doc-1".to_string(),
                content: json!({"seq": 1}),
            },
        ];
        for document in &prefix {
            model.write(parent, &document.id, document.content.clone());
            write(&store, parent, &document.id, document.content.clone()).await;
        }
        assert!(store
            .tail_offset(parent)
            .await
            .expect("read the tail offset")
            .is_some());

        // Model first: the branch inherits the parent prefix at the tail.
        model.fork(parent, branch);
        assert_eq!(model.replay(branch, 0), &prefix[..]);

        store
            .fork(parent, branch, None)
            .await
            .expect("fork at the parent tail");
        let branch_prefix = store.replay(branch, None).await.expect("branch replay");
        assert_store_matches(
            &branch_prefix,
            model.replay(branch, 0),
            "branch reads the prefix",
        );

        let own = ModelDocument {
            id: "branch-doc".to_string(),
            content: json!({"role": "assistant", "text": "branch"}),
        };
        model.write(branch, &own.id, own.content.clone());
        write(&store, branch, &own.id, own.content.clone()).await;

        // Model first: the branch is the prefix plus its own document.
        let expected: Vec<ModelDocument> =
            prefix.iter().cloned().chain(std::iter::once(own)).collect();
        assert_eq!(model.replay(branch, 0), expected.as_slice());
        let branch_all = store.replay(branch, None).await.expect("branch replay");
        assert_store_matches(
            &branch_all,
            expected.as_slice(),
            "branch reads the prefix and its own document",
        );

        // Then the store: the parent did not change.
        assert_eq!(model.replay(parent, 0), &prefix[..]);
        let parent_after = store.replay(parent, None).await.expect("parent replay");
        assert_store_matches(
            &parent_after,
            model.replay(parent, 0),
            "the parent does not change",
        );
    });
}

/// Given a session, when the tail offset is read, then a replay from that offset
/// returns nothing and a subsequent write is read from it.
#[test]
fn given_a_session_when_the_tail_offset_is_read_then_a_replay_from_that_offset_returns_nothing_and_a_subsequent_write_is_read_from_it(
) {
    block_on(async {
        let ctx = Ctx::host();
        let store = store(&ctx);
        let mut model = DocumentModel::default();
        let session = "session-tail";

        for index in 0..2 {
            let id = format!("doc-{index}");
            let content = json!({"seq": index});
            model.write(session, &id, content.clone());
            write(&store, session, &id, content).await;
        }

        let offset = store
            .tail_offset(session)
            .await
            .expect("read the tail offset")
            .expect("the session has a tail");
        let from = model.tail(session);
        assert_eq!(from, 2);

        // Model first: no document follows the tail.
        assert_eq!(model.replay(session, from), []);
        let at_tail = store
            .replay(session, Some(offset.clone()))
            .await
            .expect("replay from the tail");
        assert_eq!(at_tail.len(), 0, "a replay from the tail returns nothing");

        let content = json!({"text": "after the tail"});
        model.write(session, "doc-after", content.clone());
        write(&store, session, "doc-after", content).await;

        // Model first: the later write is the only document after the tail.
        let expected = [ModelDocument {
            id: "doc-after".to_string(),
            content: json!({"text": "after the tail"}),
        }];
        assert_eq!(model.replay(session, from), &expected[..]);
        let observed = store
            .replay(session, Some(offset))
            .await
            .expect("replay from the tail after the write");
        assert_store_matches(&observed, &expected, "read from the tail offset");
    });
}

/// Given a missing session, when a document is read, then the store reports no
/// document.
#[test]
fn given_a_missing_session_when_a_document_is_read_then_the_store_reports_no_document() {
    block_on(async {
        let ctx = Ctx::host();
        let store = store(&ctx);
        let model = DocumentModel::default();
        let session = "session-missing";

        // Model first: the session has no document.
        assert_eq!(model.replay(session, 0), []);

        // Then the store: reading a missing session reports no document.
        let observed = store
            .replay(session, None)
            .await
            .expect("a missing session reads as empty");
        assert_eq!(
            observed.len(),
            0,
            "a missing session must report no document"
        );
        assert!(
            store
                .tail_offset(session)
                .await
                .expect("read the tail offset")
                .is_none(),
            "a missing session has no tail offset"
        );
    });
}
