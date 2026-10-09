use foundation_vectors::bm25::{Bm25Index, SimpleTokenizer};

fn make_index() -> Bm25Index {
    Bm25Index::new(Box::new(SimpleTokenizer))
}

#[test]
fn simple_tokenizer_splits_and_lowercases() {
    let tok = SimpleTokenizer;
    let tokens = foundation_vectors::Tokenizer::tokenize(&tok, "Hello World! Foo-Bar");
    assert_eq!(tokens, vec!["hello", "world", "foo", "bar"]);
}

#[test]
fn insert_and_search_basic() {
    let idx = make_index();
    idx.insert("doc1", "the quick brown fox jumps over the lazy dog");
    idx.insert("doc2", "a brown cat sat on the mat");
    idx.insert("doc3", "completely unrelated content about rockets");

    let results = idx.search("brown fox", 5);
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "doc1");
}

#[test]
fn empty_index_returns_empty() {
    let idx = make_index();
    let results = idx.search("anything", 5);
    assert!(results.is_empty());
}

#[test]
fn remove_document() {
    let idx = make_index();
    idx.insert("doc1", "rust programming language");
    idx.insert("doc2", "python programming language");

    assert_eq!(idx.len(), 2);
    idx.remove("doc1");
    assert_eq!(idx.len(), 1);

    let results = idx.search("rust", 5);
    assert!(results.is_empty());
}

#[test]
fn bm25_prefers_more_relevant() {
    let idx = make_index();
    idx.insert("rust_heavy", "rust rust rust programming in rust");
    idx.insert("rust_light", "some programming in rust");
    idx.insert("no_rust", "python programming language");

    let results = idx.search("rust", 5);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "rust_heavy");
    assert_eq!(results[1].id, "rust_light");
}

#[test]
fn update_existing_document() {
    let idx = make_index();
    idx.insert("doc1", "old content about cats");
    idx.insert("doc1", "new content about dogs");

    let cat_results = idx.search("cats", 5);
    assert!(cat_results.is_empty());

    let dog_results = idx.search("dogs", 5);
    assert_eq!(dog_results.len(), 1);
    assert_eq!(dog_results[0].id, "doc1");
}

#[test]
fn k_limits_results() {
    let idx = make_index();
    for i in 0..10 {
        idx.insert(&format!("doc{i}"), &format!("common word {i}"));
    }

    let results = idx.search("common", 3);
    assert_eq!(results.len(), 3);
}
