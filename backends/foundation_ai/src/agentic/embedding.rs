use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::errors::GenerationError;
use crate::types::{
    BoxModel, CostStatus, MessageRole, Messages, ModelId, ModelInteraction, ModelOutput,
    ModelProviders, ProviderRouter, RouterError, StopReason, TextContent, ToolShed,
    UserModelContent, UsageCosting, UsageReport,
};

fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting::zero(CostStatus::Unknown),
    }
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVector {
    pub dimensions: u16,
    pub data: Vec<f32>,
    pub model_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size: usize,
}

#[derive(Debug)]
pub enum EmbeddingError {
    DimensionMismatch {
        expected: u16,
        got: u16,
        model_id: String,
    },
    DimensionOverflow {
        value: usize,
    },
    ModelNotRegistered(String),
    ModelNotFound(String),
    Routing(RouterError),
    Generation(GenerationError),
    NoEmbeddingInOutput,
}

impl core::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DimensionMismatch {
                expected,
                got,
                model_id,
            } => write!(
                f,
                "dimension mismatch for model {model_id}: expected {expected}, got {got}"
            ),
            Self::DimensionOverflow { value } => {
                write!(f, "dimension overflow: {value} > u16::MAX")
            }
            Self::ModelNotRegistered(id) => write!(f, "model not registered: {id}"),
            Self::ModelNotFound(id) => write!(f, "model not found in router: {id}"),
            Self::Routing(e) => write!(f, "routing error: {e}"),
            Self::Generation(e) => write!(f, "generation error: {e}"),
            Self::NoEmbeddingInOutput => write!(f, "model output contained no embedding"),
        }
    }
}

impl std::error::Error for EmbeddingError {}

impl From<GenerationError> for EmbeddingError {
    fn from(e: GenerationError) -> Self {
        Self::Generation(e)
    }
}

impl From<RouterError> for EmbeddingError {
    fn from(e: RouterError) -> Self {
        Self::Routing(e)
    }
}

// ---------------------------------------------------------------------------
// TextChunker
// ---------------------------------------------------------------------------

pub trait TextChunker: Send + Sync {
    fn chunk(&self, text: &str) -> Vec<String>;
}

pub struct WholeTextChunker;

impl TextChunker for WholeTextChunker {
    fn chunk(&self, text: &str) -> Vec<String> {
        vec![text.to_string()]
    }
}

pub struct SentenceChunker;

impl TextChunker for SentenceChunker {
    fn chunk(&self, text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            current.push(ch);
            if matches!(ch, '.' | '!' | '?') {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current.clear();
            }
        }

        let remainder = current.trim().to_string();
        if !remainder.is_empty() {
            sentences.push(remainder);
        }

        if sentences.is_empty() {
            sentences.push(text.to_string());
        }

        sentences
    }
}

// ---------------------------------------------------------------------------
// ColdCache trait
// ---------------------------------------------------------------------------

pub trait ColdCache: Send + Sync {
    fn get(&self, key: &str) -> Option<(Vec<f32>, String)>;
    fn put(&self, key: &str, values: &[f32], model_id: &str);
    fn clear(&self);
}

pub struct NoopColdCache;

impl ColdCache for NoopColdCache {
    fn get(&self, _key: &str) -> Option<(Vec<f32>, String)> {
        None
    }
    fn put(&self, _key: &str, _values: &[f32], _model_id: &str) {}
    fn clear(&self) {}
}

// ---------------------------------------------------------------------------
// EmbeddingProvider trait
// ---------------------------------------------------------------------------

pub trait EmbeddingProvider: Send + Sync {
    /// # Errors
    /// Returns [`EmbeddingError`] if the text cannot be embedded.
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError>;

    /// # Errors
    /// Returns [`EmbeddingError`] if any text cannot be embedded.
    fn embed_batch(
        &self,
        texts: &[String],
        model_id: &str,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError>;

    fn register_model(&self, model_id: &str, dimensions: u16);

    fn cache_stats(&self) -> CacheStats;

    fn clear_cache(&self);
}

// ---------------------------------------------------------------------------
// LRU cache internals
// ---------------------------------------------------------------------------

#[derive(Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    text_hash: u64,
    model_id: String,
    epoch: u64,
}

struct CacheEntry {
    text: String,
    embedding: EmbeddingVector,
}

struct LruCache {
    map: HashMap<CacheKey, CacheEntry>,
    order: VecDeque<CacheKey>,
    max_entries: usize,
    evictions: u64,
}

impl LruCache {
    fn new(max_entries: usize) -> Self {
        Self {
            map: HashMap::with_capacity(max_entries),
            order: VecDeque::with_capacity(max_entries),
            max_entries,
            evictions: 0,
        }
    }

    fn get(&mut self, key: &CacheKey, text: &str) -> Option<&EmbeddingVector> {
        if let Some(entry) = self.map.get(key) {
            if entry.text == text {
                if let Some(pos) = self.order.iter().position(|k| k == key) {
                    let k = self.order.remove(pos).unwrap();
                    self.order.push_back(k);
                }
                return Some(&entry.embedding);
            }
        }
        None
    }

    fn insert(&mut self, key: CacheKey, text: String, embedding: EmbeddingVector) {
        if self.map.contains_key(&key) {
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        } else if self.map.len() >= self.max_entries {
            if let Some(evicted) = self.order.pop_front() {
                self.map.remove(&evicted);
                self.evictions += 1;
            }
        }

        self.order.push_back(key.clone());
        self.map.insert(key, CacheEntry { text, embedding });
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

// ---------------------------------------------------------------------------
// CachedEmbeddingProvider
// ---------------------------------------------------------------------------

pub struct CachedEmbeddingProvider {
    router: ProviderRouter,
    chunker: Box<dyn TextChunker>,
    cache: Mutex<LruCache>,
    cold_cache: Box<dyn ColdCache>,
    registry: RwLock<HashMap<String, u16>>,
    hits: AtomicU64,
    misses: AtomicU64,
    epoch: u64,
}

fn hash_text(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = ahash::AHasher::default();
    text.hash(&mut hasher);
    hasher.finish()
}

fn checked_dim(dim: usize) -> Result<u16, EmbeddingError> {
    u16::try_from(dim).map_err(|_| EmbeddingError::DimensionOverflow { value: dim })
}

fn make_embedding_interaction(text: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
        messages: vec![
            Messages::Assistant {
                id: foundation_compact::ids::new_scru128(),
                model: ModelId::Name("embedding".to_string(), None),
                timestamp: foundation_compact::SystemTime::now(),
                usage: zero_usage(),
                content: ModelOutput::Embedding {
                    dimensions: 0,
                    values: vec![],
                },
                stop_reason: StopReason::Stop,
                provider: ModelProviders::Custom("embedding".into()),
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: MessageRole::User,
                content: UserModelContent::Text(TextContent {
                    content: text.to_string(),
                    signature: None,
                }),
                signature: None,
            },
        ],
    }
}

impl CachedEmbeddingProvider {
    #[must_use]
    pub fn new(
        router: ProviderRouter,
        chunker: Box<dyn TextChunker>,
        cold_cache: Box<dyn ColdCache>,
        max_cache_entries: usize,
    ) -> Self {
        Self {
            router,
            chunker,
            cache: Mutex::new(LruCache::new(max_cache_entries)),
            cold_cache,
            registry: RwLock::new(HashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            epoch: 0,
        }
    }

    #[must_use]
    pub fn with_defaults(router: ProviderRouter) -> Self {
        Self::new(router, Box::new(SentenceChunker), Box::new(NoopColdCache), 1000)
    }

    fn embed_single(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        let text_hash = hash_text(text);
        let key = CacheKey {
            text_hash,
            model_id: model_id.to_string(),
            epoch: self.epoch,
        };

        // 1. LRU cache hit
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(vec) = cache.get(&key, text) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(vec.clone());
            }
        }

        // 2. Cold cache hit
        let cold_key = format!("{text_hash:016x}:{model_id}:{}", self.epoch);
        if let Some((values, mid)) = self.cold_cache.get(&cold_key) {
            let dim = checked_dim(values.len())?;
            let vec = EmbeddingVector {
                dimensions: dim,
                data: values,
                model_id: mid,
            };
            let mut cache = self.cache.lock().unwrap();
            cache.insert(key, text.to_string(), vec.clone());
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(vec);
        }

        self.misses.fetch_add(1, Ordering::Relaxed);

        // 3. Generate via model
        let mid = ModelId::Name(model_id.to_string(), None);
        let model: BoxModel = self.router.get_model(&mid)?;

        let interaction = make_embedding_interaction(text);
        let results = model.generate(interaction, None)?;

        let (dimensions_raw, values) = results
            .iter()
            .find_map(|msg| {
                if let Messages::Assistant {
                    content: ModelOutput::Embedding { dimensions, values },
                    ..
                } = msg
                {
                    Some((*dimensions, values.clone()))
                } else {
                    None
                }
            })
            .ok_or(EmbeddingError::NoEmbeddingInOutput)?;

        let dim = checked_dim(dimensions_raw)?;

        // 4. Dimension validation
        {
            let reg = self.registry.read().unwrap();
            if let Some(&expected) = reg.get(model_id) {
                if dim != expected {
                    return Err(EmbeddingError::DimensionMismatch {
                        expected,
                        got: dim,
                        model_id: model_id.to_string(),
                    });
                }
            }
        }

        let vec = EmbeddingVector {
            dimensions: dim,
            data: values,
            model_id: model_id.to_string(),
        };

        // 5. Populate caches
        self.cold_cache.put(&cold_key, &vec.data, model_id);
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(key, text.to_string(), vec.clone());
        }

        Ok(vec)
    }
}

impl EmbeddingProvider for CachedEmbeddingProvider {
    fn embed(&self, text: &str, model_id: &str) -> Result<EmbeddingVector, EmbeddingError> {
        let chunks = self.chunker.chunk(text);
        if chunks.len() == 1 {
            return self.embed_single(&chunks[0], model_id);
        }

        let mut all_values: Vec<f32> = Vec::new();
        let mut dim: Option<u16> = None;
        let mut count = 0u32;

        for chunk in &chunks {
            let vec = self.embed_single(chunk, model_id)?;
            match dim {
                None => {
                    dim = Some(vec.dimensions);
                    all_values.resize(vec.dimensions as usize, 0.0);
                }
                Some(d) if d != vec.dimensions => {
                    return Err(EmbeddingError::DimensionMismatch {
                        expected: d,
                        got: vec.dimensions,
                        model_id: model_id.to_string(),
                    });
                }
                _ => {}
            }
            for (acc, v) in all_values.iter_mut().zip(vec.data.iter()) {
                *acc += v;
            }
            count += 1;
        }

        if count > 1 {
            for v in &mut all_values {
                *v /= count as f32;
            }
        }

        Ok(EmbeddingVector {
            dimensions: dim.unwrap_or(0),
            data: all_values,
            model_id: model_id.to_string(),
        })
    }

    fn embed_batch(
        &self,
        texts: &[String],
        model_id: &str,
    ) -> Result<Vec<EmbeddingVector>, EmbeddingError> {
        texts
            .iter()
            .map(|t| self.embed(t, model_id))
            .collect()
    }

    fn register_model(&self, model_id: &str, dimensions: u16) {
        let mut reg = self.registry.write().unwrap();
        reg.insert(model_id.to_string(), dimensions);
    }

    fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: cache.evictions,
            size: cache.map.len(),
        }
    }

    fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        self.cold_cache.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }
}
