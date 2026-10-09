//! Proof of Work service — anti-bot rate limiting for auth mutations.
//!
//! HOW: Server generates random challenge + difficulty (leading zero bits).
//! Client hashes `(challenge + nonce)` until hash has enough leading zeros.
//! Server validates by hashing once.

use std::sync::Arc;

use chrono::Utc;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_db::{AsyncKeyValueStore, KeyValueStore, StorageError};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowChallenge {
    pub difficulty: u32,
    pub challenge: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct PowSolution {
    pub challenge: String,
    pub solution: String,
}

/// Persisted challenge state. Stored in the shared cache (not a process-local
/// map) so the issuing endpoint (`GET /pow`) and the verifying endpoint
/// (`POST /pow`) — which are served by separate handler instances, possibly on
/// separate nodes — see the same challenge. Expiry is embedded in the value and
/// checked lazily on read, mirroring `OAuthState`/`Session`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChallengeEntry {
    difficulty: u32,
    /// Unix timestamp after which the challenge is no longer valid.
    expires_at: i64,
    solved: bool,
}

/// Cache key for a PoW challenge.
fn challenge_key(challenge: &str) -> String {
    format!("pow:challenge:{challenge}")
}

pub struct PowService<KV: KeyValueStore> {
    cache: KV,
    difficulty: u32,
    validity_secs: u64,
    solve_validity_secs: u64,
}

impl<KV: KeyValueStore> PowService<KV> {
    #[must_use]
    pub fn new(
        cache: KV,
        difficulty: u32,
        challenge_validity_secs: u64,
        solve_validity_secs: u64,
    ) -> Self {
        Self {
            cache,
            difficulty,
            validity_secs: challenge_validity_secs,
            solve_validity_secs,
        }
    }

    /// Generate a new PoW challenge and persist it in the shared cache.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the cache write fails.
    pub fn generate_challenge(&self) -> Result<PowChallenge, StorageError> {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        let challenge = URL_SAFE_NO_PAD.encode(bytes);

        let entry = ChallengeEntry {
            difficulty: self.difficulty,
            expires_at: Utc::now().timestamp() + self.validity_secs as i64,
            solved: false,
        };
        self.cache.set(&challenge_key(&challenge), entry)?;

        Ok(PowChallenge {
            difficulty: self.difficulty,
            challenge,
            expires_in: self.validity_secs,
        })
    }

    /// Verify a PoW solution. Returns true if valid.
    ///
    /// Single-use: a solved challenge is marked solved so it can't be replayed.
    /// Cache errors are logged and treated as a verification failure.
    pub fn verify_solution(&self, solution: &PowSolution) -> bool {
        let key = challenge_key(&solution.challenge);
        let mut entry: ChallengeEntry = match self.cache.get(&key) {
            Ok(Some(e)) => e,
            Ok(None) => return false,
            Err(e) => {
                eprintln!("PoW: failed to read challenge from cache: {e}");
                return false;
            }
        };

        // Lazy expiry check.
        if Utc::now().timestamp() >= entry.expires_at {
            let _ = self.cache.delete(&key); // best-effort eviction
            return false;
        }

        // Already solved — reject replay.
        if entry.solved {
            return false;
        }

        // Hash (challenge + solution) and check leading zeros.
        let input = format!("{}{}", solution.challenge, solution.solution);
        let hash = Sha256::digest(input.as_bytes());
        let valid = count_leading_zero_bytes(&hash) >= entry.difficulty / 8;

        if valid {
            entry.solved = true;
            if let Err(e) = self.cache.set(&key, entry) {
                eprintln!("cache error: {e}");
                return false;
            }
        }
        valid
    }
}

// ─── Async PoW service (over `AsyncKeyValueStore`) ──────────────────────────

/// Async counterpart to [`PowService`], backed by an `AsyncKeyValueStore`.
/// Used on wasm32/CF Workers where KV APIs (D1) are Promise-based.
pub struct AsyncPowService<AKV: AsyncKeyValueStore> {
    cache: AKV,
    difficulty: u32,
    validity_secs: u64,
    solve_validity_secs: u64,
}

impl<AKV: AsyncKeyValueStore> AsyncPowService<AKV> {
    #[must_use]
    pub fn new(
        cache: AKV,
        difficulty: u32,
        challenge_validity_secs: u64,
        solve_validity_secs: u64,
    ) -> Self {
        Self {
            cache,
            difficulty,
            validity_secs: challenge_validity_secs,
            solve_validity_secs,
        }
    }

    /// Generate a new PoW challenge and persist it in the shared cache.
    pub async fn generate_challenge(&self) -> Result<PowChallenge, StorageError> {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        let challenge = URL_SAFE_NO_PAD.encode(bytes);

        let entry = ChallengeEntry {
            difficulty: self.difficulty,
            expires_at: Utc::now().timestamp() + self.validity_secs as i64,
            solved: false,
        };
        self.cache.set_async(&challenge_key(&challenge), entry).await?;

        Ok(PowChallenge {
            difficulty: self.difficulty,
            challenge,
            expires_in: self.validity_secs,
        })
    }

    /// Verify a PoW solution. Returns true if valid.
    pub async fn verify_solution(&self, solution: &PowSolution) -> bool {
        let key = challenge_key(&solution.challenge);
        let mut entry: ChallengeEntry = match self.cache.get_async(&key).await {
            Ok(Some(e)) => e,
            Ok(None) => return false,
            Err(e) => {
                eprintln!("PoW: failed to read challenge from cache: {e}");
                return false;
            }
        };

        // Lazy expiry check.
        if Utc::now().timestamp() >= entry.expires_at {
            let _ = self.cache.delete_async(&key).await; // best-effort eviction
            return false;
        }

        // Already solved — reject replay.
        if entry.solved {
            return false;
        }

        // Hash (challenge + solution) and check leading zeros.
        let input = format!("{}{}", solution.challenge, solution.solution);
        let hash = Sha256::digest(input.as_bytes());
        let valid = count_leading_zero_bytes(&hash) >= entry.difficulty / 8;

        if valid {
            entry.solved = true;
            if let Err(e) = self.cache.set_async(&key, entry).await {
                eprintln!("cache error: {e}");
                return false;
            }
        }
        valid
    }
}

fn count_leading_zero_bytes(hash: &[u8]) -> u32 {
    let mut count = 0;
    for &byte in hash {
        if byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}
