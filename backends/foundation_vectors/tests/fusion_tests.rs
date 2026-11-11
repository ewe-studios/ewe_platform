use std::collections::HashMap;
use std::sync::Mutex;

use foundation_core::valtron::valtron_test;
use foundation_vectors::bm25::{Bm25Index, SimpleTokenizer};
use foundation_vectors::fusion::{fuse, hybrid_search, FusionStrategy, Reranker, TextLookup};
use foundation_vectors::metric::DistanceMetric;
use foundation_vectors::store::{
    InMemoryVectorStore, VectorEntry, VectorMatch, VectorMetadata, VectorStore, VectorStoreConfig,
};
use foundation_vectors::vector::Vector;

fn vm(id: &str, score: f32) -> VectorMatch {
    VectorMatch {
        id: id.into(),
        score,
    }
}

#[test]
fn rrf_fuses_two_lists() {
    let vector = vec![vm("a", 0.9), vm("b", 0.8), vm("c", 0.7)];
    let keyword = vec![vm("b", 5.0), vm("d", 4.0), vm("a", 3.0)];

    let fused = fuse(&vector, &keyword, 4, FusionStrategy::Rrf { k: 60.0 });

    assert!(fused.len() <= 4);
    let ids: Vec<&str> = fused.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
}

#[test]
fn rrf_ranks_shared_items_higher() {
    let vector = vec![vm("shared", 0.9), vm("vec_only", 0.8)];
    let keyword = vec![vm("shared", 5.0), vm("kw_only", 4.0)];

    let fused = fuse(&vector, &keyword, 3, FusionStrategy::Rrf { k: 60.0 });

    assert_eq!(fused[0].id, "shared");
}

#[test]
fn alpha_pure_vector() {
    let vector = vec![vm("a", 0.9), vm("b", 0.7), vm("e", 0.5), vm("h", 0.3)];
    let keyword = vec![vm("c", 5.0), vm("d", 4.0)];

    let fused = fuse(&vector, &keyword, 6, FusionStrategy::Alpha { alpha: 1.0 });

    assert_eq!(fused[0].id, "a");
    assert_eq!(fused[1].id, "b");
    assert_eq!(fused[2].id, "e");
    assert!(fused[0].score > fused[1].score);
    assert!(fused[1].score > fused[2].score);
}

#[test]
fn alpha_pure_keyword() {
    let vector = vec![vm("a", 0.9), vm("b", 0.5)];
    let keyword = vec![vm("c", 5.0), vm("d", 4.0), vm("f", 3.0), vm("g", 2.0)];

    let fused = fuse(&vector, &keyword, 6, FusionStrategy::Alpha { alpha: 0.0 });

    assert_eq!(fused[0].id, "c");
    assert_eq!(fused[1].id, "d");
    assert_eq!(fused[2].id, "f");
    assert!(fused[0].score > fused[1].score);
    assert!(fused[1].score > fused[2].score);
}

#[test]
fn empty_lists_produce_empty() {
    let fused = fuse(&[], &[], 10, FusionStrategy::default());
    assert_eq!(fused, [] as [foundation_vectors::VectorMatch; 0]);
}

#[test]
fn one_empty_list_still_works() {
    let vector = vec![vm("a", 0.9), vm("b", 0.5)];
    let fused = fuse(&vector, &[], 10, FusionStrategy::Rrf { k: 60.0 });
    assert_eq!(fused.len(), 2);
}

struct RecordingLookup {
    texts: HashMap<String, String>,
}

#[async_trait::async_trait]
impl TextLookup for RecordingLookup {
    async fn text(&self, id: &str) -> Option<String> {
        self.texts.get(id).cloned()
    }
}

struct RecordingReranker {
    seen: Mutex<Vec<Vec<String>>>,
}

impl Reranker for RecordingReranker {
    fn rerank(
        &self,
        _query: &str,
        candidates: Vec<VectorMatch>,
        texts: &[&str],
    ) -> Vec<VectorMatch> {
        self.seen
            .lock()
            .unwrap()
            .push(texts.iter().map(|t| (*t).to_string()).collect());
        candidates
    }
}

fn entry(id: &str, v: [f32; 3]) -> VectorEntry {
    VectorEntry {
        id: id.into(),
        vector: Vector::new(v.to_vec()),
        metadata: VectorMetadata::default(),
    }
}

/// The reranker gets one text per fused candidate, in fused order. The vector
/// list ranks `a, b, c`; the keyword list matches only `b`, so RRF fuses to
/// `b, a, c`.
#[valtron_test]
async fn hybrid_search_passes_texts_to_reranker() {
    let store = InMemoryVectorStore::new(VectorStoreConfig::new(3, DistanceMetric::L2));
    for (id, v) in [
        ("a", [1.0, 0.0, 0.0]),
        ("b", [2.0, 0.0, 0.0]),
        ("c", [3.0, 0.0, 0.0]),
    ] {
        store.insert("ns", entry(id, v)).await.unwrap();
    }

    let bm25 = Bm25Index::new(Box::new(SimpleTokenizer));
    bm25.insert("a", "the first note");
    bm25.insert("b", "the matching note");
    bm25.insert("c", "the third note");

    let lookup = RecordingLookup {
        texts: [
            ("a".to_string(), "text a".to_string()),
            ("b".to_string(), "text b".to_string()),
            ("c".to_string(), "text c".to_string()),
        ]
        .into_iter()
        .collect(),
    };
    let reranker = RecordingReranker {
        seen: Mutex::new(Vec::new()),
    };

    let out = hybrid_search(
        "matching",
        &[1.0, 0.0, 0.0],
        "ns",
        &store,
        &bm25,
        &lookup,
        3,
        FusionStrategy::Rrf { k: 60.0 },
        Some(&reranker),
    )
    .await
    .unwrap();

    let ids: Vec<&str> = out.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "a", "c"], "RRF fused order");

    let seen = reranker.seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one rerank call");
    assert_eq!(
        seen[0],
        vec!["text b", "text a", "text c"],
        "one text per candidate, in order"
    );
}
