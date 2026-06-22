use std::collections::HashMap;
use std::sync::RwLock;

use crate::store::VectorMatch;

pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

pub struct SimpleTokenizer;

impl Tokenizer for SimpleTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }
}

struct Posting {
    doc_id: String,
    tf: u32,
}

struct DocInfo {
    length: u32,
}

struct Bm25Inner {
    postings: HashMap<String, Vec<Posting>>,
    docs: HashMap<String, DocInfo>,
    total_length: u64,
    k1: f32,
    b: f32,
}

impl Bm25Inner {
    fn avgdl(&self) -> f32 {
        if self.docs.is_empty() {
            return 1.0;
        }
        self.total_length as f32 / self.docs.len() as f32
    }

    fn idf(&self, term: &str) -> f32 {
        let n = self.docs.len() as f32;
        let df = self
            .postings
            .get(term)
            .map_or(0, Vec::len) as f32;
        if df == 0.0 {
            return 0.0;
        }
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }
}

pub struct Bm25Index {
    inner: RwLock<Bm25Inner>,
    tokenizer: Box<dyn Tokenizer>,
}

impl Bm25Index {
    #[must_use]
    pub fn new(tokenizer: Box<dyn Tokenizer>) -> Self {
        Self::with_params(tokenizer, 1.2, 0.75)
    }

    #[must_use]
    pub fn with_params(tokenizer: Box<dyn Tokenizer>, k1: f32, b: f32) -> Self {
        Self {
            inner: RwLock::new(Bm25Inner {
                postings: HashMap::new(),
                docs: HashMap::new(),
                total_length: 0,
                k1,
                b,
            }),
            tokenizer,
        }
    }

    pub fn insert(&self, id: &str, text: &str) {
        let tokens = self.tokenizer.tokenize(text);
        let doc_len = tokens.len() as u32;

        let mut tf_map: HashMap<&str, u32> = HashMap::new();
        for t in &tokens {
            *tf_map.entry(t.as_str()).or_insert(0) += 1;
        }

        let mut inner = self.inner.write().unwrap();

        if let Some(old) = inner.docs.get(id) {
            inner.total_length -= u64::from(old.length);
            for postings in inner.postings.values_mut() {
                postings.retain(|p| p.doc_id != id);
            }
        }

        inner.docs.insert(id.to_string(), DocInfo { length: doc_len });
        inner.total_length += u64::from(doc_len);

        for (term, tf) in tf_map {
            inner
                .postings
                .entry(term.to_string())
                .or_default()
                .push(Posting {
                    doc_id: id.to_string(),
                    tf,
                });
        }
    }

    pub fn remove(&self, id: &str) {
        let mut inner = self.inner.write().unwrap();
        if let Some(doc) = inner.docs.remove(id) {
            inner.total_length -= u64::from(doc.length);
            for postings in inner.postings.values_mut() {
                postings.retain(|p| p.doc_id != id);
            }
            inner.postings.retain(|_, v| !v.is_empty());
        }
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<VectorMatch> {
        let tokens = self.tokenizer.tokenize(query);
        let inner = self.inner.read().unwrap();
        let avgdl = inner.avgdl();

        let mut scores: HashMap<&str, f32> = HashMap::new();

        for token in &tokens {
            let idf = inner.idf(token);
            if let Some(postings) = inner.postings.get(token.as_str()) {
                for p in postings {
                    if let Some(doc) = inner.docs.get(&p.doc_id) {
                        let tf = p.tf as f32;
                        let dl = doc.length as f32;
                        let numerator = tf * (inner.k1 + 1.0);
                        let denominator =
                            tf + inner.k1 * (1.0 - inner.b + inner.b * dl / avgdl);
                        let bm25 = idf * numerator / denominator;
                        *scores.entry(&p.doc_id).or_insert(0.0) += bm25;
                    }
                }
            }
        }

        let mut results: Vec<VectorMatch> = scores
            .into_iter()
            .map(|(id, score)| VectorMatch {
                id: id.to_string(),
                score,
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().docs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
