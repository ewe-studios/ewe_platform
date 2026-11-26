//! The history archive of a destroyed instance (spec 83 feature 05, T26).
//!
//! WHY: owner decision 5 says `DestroyInstance` archives the session history to
//! the object store before it deletes the Durable Object storage. A failed
//! archive keeps the history; no history is lost.
//!
//! WHAT: [`ArchiveConfig`] (the limits), [`ArchiveReport`] (what one archive
//! produced), [`ArchiveMetadata`] (the facts that the object store records) and
//! [`HistoryArchive`] (the uploader).
//!
//! HOW: [`HistoryArchive::archive`] reads the session documents in pages, writes
//! one JSON object per line, compresses the lines with gzip and pushes the
//! compressed bytes into a bounded stream that a [`BlobStore`] consumes. No
//! step holds the whole history in memory: a part holds at most `part_size`
//! bytes plus one document.
//!
//! The caller uploads first and destroys second. A failed upload returns an
//! error and the caller keeps the storage.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use flate2::write::GzEncoder;
use flate2::Compression;
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use foundation_db::traits::BlobStream;
use foundation_db::{BlobStore, DocumentStore, StorageError};
use foundation_errstacks::ErrorTrace;
use sha2::{Digest, Sha256};

use crate::core::registry::Scope;
use crate::error::{AgentRpcError, AgentRpcResult};

/// Default part size: 8 mebibytes (MiB). It is above the 5 MiB minimum part
/// size of the Cloudflare R2 multipart upload, and it is the default part size
/// of the Amazon Web Services (AWS) command-line tool.
pub const DEFAULT_PART_SIZE: usize = 8 * 1024 * 1024;

/// Default page size: 256 documents. A page is one read of the session
/// document store.
pub const DEFAULT_PAGE_SIZE: usize = 256;

/// The limits of the history archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveConfig {
    /// Archive a destroyed instance. Default `true`: owner decision 5. A host
    /// that has no archive binding sets this to `false`.
    pub enabled: bool,
    /// Largest part of the compressed archive stream. Default
    /// [`DEFAULT_PART_SIZE`].
    pub part_size: usize,
    /// Documents read in one page. Default [`DEFAULT_PAGE_SIZE`].
    pub page_size: usize,
}

impl ArchiveConfig {
    /// Turn the archive on or off.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the part size, in bytes. A value of zero becomes one byte.
    #[must_use]
    pub fn with_part_size(mut self, part_size: usize) -> Self {
        self.part_size = part_size.max(1);
        self
    }

    /// Set the document page size. A value of zero becomes one.
    #[must_use]
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.max(1);
        self
    }
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            part_size: DEFAULT_PART_SIZE,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

/// The facts that the archive records beside the object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveMetadata {
    /// The instance name.
    pub instance: String,
    /// The scope key of the instance (a hash for `PerOwner`).
    pub scope: String,
    /// The session identifier.
    pub session_id: String,
    /// The model of the instance.
    pub model: String,
    /// The number of JSON lines in the archive.
    pub record_count: u64,
    /// Lowercase SHA-256 hex of the uncompressed JSON lines.
    pub sha256: String,
}

/// What one archive produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveReport {
    /// The object key.
    pub key: String,
    /// The recorded facts.
    pub metadata: ArchiveMetadata,
}

/// The uploader of one instance's history.
pub struct HistoryArchive {
    store: Arc<dyn BlobStore>,
    config: ArchiveConfig,
    clock: Arc<dyn foundation_context::Clock>,
}

impl std::fmt::Debug for HistoryArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryArchive")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl HistoryArchive {
    /// An archive into `store`, with the client clock of `ctx`.
    #[must_use]
    pub fn new(store: Arc<dyn BlobStore>, config: ArchiveConfig, ctx: &Ctx) -> Self {
        Self {
            store,
            config,
            clock: ctx.clock(),
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &ArchiveConfig {
        &self.config
    }

    /// The object key of one instance at one time.
    ///
    /// It is `archives/<scope key>/<instance>/<destroyed at, UTC>.jsonl.gz`.
    #[must_use]
    pub fn object_key(&self, scope: &Scope, instance: &str) -> String {
        format!(
            "archives/{}/{}/{}.jsonl.gz",
            scope.as_key(),
            instance,
            self.destroyed_at()
        )
    }

    /// The destruction time, as an International Organization for
    /// Standardization (ISO) 8601 time in Coordinated Universal Time (UTC).
    fn destroyed_at(&self) -> String {
        let millis = unix_millis(self.clock.wall_now());
        chrono::DateTime::from_timestamp_millis(millis).map_or_else(
            || millis.to_string(),
            |time| time.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        )
    }

    /// Read the session history and upload it as one gzip object.
    ///
    /// The upload finishes before this call returns. The caller destroys the
    /// storage only after it returns `Ok`.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when a read or the upload fails. The object
    /// is then absent or incomplete, and the history is unchanged.
    pub async fn archive<D>(
        &self,
        documents: Arc<D>,
        collection: &str,
        scope: &Scope,
        instance: &str,
        session_id: &str,
        model: &str,
    ) -> AgentRpcResult<ArchiveReport>
    where
        D: DocumentStore + 'static,
    {
        let key = self.object_key(scope, instance);
        let state = ArchiveState::new(documents, collection.to_string(), &self.config);
        let result = Arc::clone(&state.stats);
        let stream = state.into_stream();
        self.store
            .put_blob_stream(&key, None, stream)
            .await
            .map_err(|error| storage_error(&error))?;

        let count = result.record_count.load(Ordering::Acquire);
        let sha256 = result.sha256.lock().map_or_default(|hex| hex.clone());
        Ok(ArchiveReport {
            key,
            metadata: ArchiveMetadata {
                instance: instance.to_string(),
                scope: scope.as_key().to_string(),
                session_id: session_id.to_string(),
                model: model.to_string(),
                record_count: count,
                sha256,
            },
        })
    }
}

fn storage_error(error: &StorageError) -> ErrorTrace<AgentRpcError> {
    AgentRpcError::Storage(error.to_string()).into()
}

/// The shared result of the stream, read by the caller after the upload.
struct ArchiveStats {
    record_count: AtomicU64,
    sha256: Mutex<String>,
}

struct ArchiveState<D> {
    documents: Arc<D>,
    collection: String,
    part_size: usize,
    page_size: usize,
    /// The last document that the archive wrote. The next read starts at it.
    after: Option<String>,
    page: Vec<foundation_db::Document>,
    index: usize,
    encoder: Option<GzEncoder<Vec<u8>>>,
    hasher: Sha256,
    finished: bool,
    stats: Arc<ArchiveStats>,
}

impl<D: DocumentStore + 'static> ArchiveState<D> {
    fn new(documents: Arc<D>, collection: String, config: &ArchiveConfig) -> Self {
        Self {
            documents,
            collection,
            part_size: config.part_size.max(1),
            page_size: config.page_size.max(1),
            after: None,
            page: Vec::new(),
            index: 0,
            encoder: Some(GzEncoder::new(Vec::new(), Compression::default())),
            hasher: Sha256::new(),
            finished: false,
            stats: Arc::new(ArchiveStats {
                record_count: AtomicU64::new(0),
                sha256: Mutex::new(String::new()),
            }),
        }
    }

    /// Read the next page of session documents.
    ///
    /// The read starts at the last written document and includes it, so the
    /// first row is removed when it repeats. `page_size` new rows result.
    async fn load_page(&mut self) -> Result<bool, StorageError> {
        let from = self.after.clone().unwrap_or_default();
        let limit = if self.after.is_some() {
            self.page_size + 1
        } else {
            self.page_size
        };
        let mut page = self
            .documents
            .scan_documents_from(&self.collection, &from, limit)
            .await?;
        if self.after.is_some()
            && page
                .first()
                .is_some_and(|document| Some(document.id.as_str()) == self.after.as_deref())
        {
            page.remove(0);
        }
        if page.is_empty() {
            return Ok(false);
        }
        self.page = page;
        self.index = 0;
        Ok(true)
    }

    fn into_stream(self) -> BlobStream {
        let stream = futures::stream::unfold(self, |mut state| async move {
            if state.finished {
                return None;
            }
            let mut part: Vec<u8> = Vec::new();
            while part.len() < state.part_size {
                if state.index >= state.page.len() {
                    match state.load_page().await {
                        Ok(true) => {}
                        Ok(false) => {
                            // End of the history: finish the gzip stream and
                            // record the count and the SHA-256 of the lines.
                            let encoder = state.encoder.take()?;
                            match encoder.finish() {
                                Ok(tail) => part.extend_from_slice(&tail),
                                Err(error) => {
                                    state.finished = true;
                                    return Some((
                                        Err(StorageError::Backend(format!(
                                            "gzip finish failed: {error}"
                                        ))),
                                        state,
                                    ));
                                }
                            }
                            state.finished = true;
                            if let Ok(mut hex) = state.stats.sha256.lock() {
                                *hex = hex_encode(&state.hasher.finalize_reset());
                            }
                            return if part.is_empty() {
                                None
                            } else {
                                Some((Ok(part), state))
                            };
                        }
                        Err(error) => {
                            state.finished = true;
                            return Some((Err(error), state));
                        }
                    }
                }
                let document = state.page[state.index].clone();
                state.index += 1;
                state.after = Some(document.id.clone());
                let mut line = document.content.into_bytes();
                line.push(b'\n');
                state.hasher.update(&line);
                state.stats.record_count.fetch_add(1, Ordering::AcqRel);
                let encoder = state.encoder.as_mut()?;
                if let Err(error) = encoder.write_all(&line) {
                    state.finished = true;
                    return Some((
                        Err(StorageError::Backend(format!("gzip write failed: {error}"))),
                        state,
                    ));
                }
                // A gzip encoder holds its output in an internal buffer. Move
                // the bytes to the part before the next poll, so a part goes
                // out without waiting for the whole history.
                if let Err(error) = encoder.flush() {
                    state.finished = true;
                    return Some((
                        Err(StorageError::Backend(format!("gzip flush failed: {error}"))),
                        state,
                    ));
                }
                part.extend_from_slice(&std::mem::take(encoder.get_mut()));
            }
            Some((Ok(part), state))
        });
        Box::pin(stream)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// The archive as a plain report, for a host that records the object key only.
impl ArchiveReport {
    /// The object key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}
