//! Proof of Work service — anti-bot rate limiting for auth mutations.
//!
//! HOW: Server generates random challenge + difficulty (leading zero bits).
//! Client hashes `(challenge + nonce)` until hash has enough leading zeros.
//! Server validates by hashing once.

use std::sync::Arc;

use chrono::Utc;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_db::{KeyValueStore, StorageError};
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

pub struct PowService {
    cache: Arc<dyn KeyValueStore>,
    difficulty: u32,
    validity_secs: u64,
    solve_validity_secs: u64,
}

impl PowService {
    #[must_use]
    pub fn new(
        cache: Arc<dyn KeyValueStore>,
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
                tracing::error!(error = %e, "PoW: failed to read challenge from cache");
                return false;
            }
        };

        // Lazy expiry check.
        if Utc::now().timestamp() >= entry.expires_at {
            if let Err(e) = self.cache.delete(&key) {
                tracing::error!(error = %e, "PoW: failed to evict expired challenge");
            }
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
                tracing::error!(error = %e, "PoW: failed to mark challenge solved");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_easy_pow() {
        let service = PowService::new(8, 300, 600); // difficulty 8 bits = 1 zero byte
        let challenge = service.generate_challenge();
        assert_eq!(challenge.difficulty, 8);
        assert_eq!(challenge.expires_in, 300);

        // Find a valid nonce (difficulty is low enough for a test)
        let mut nonce = 0u64;
        let mut solution = None;
        for _ in 0..10000 {
            let input = format!("{}{}", challenge.challenge, nonce);
            let hash = Sha256::digest(input.as_bytes());
            if count_leading_zero_bytes(&hash) >= 1 {
                solution = Some(nonce.to_string());
                break;
            }
            nonce += 1;
        }
        let sol = solution.expect("should find a solution with difficulty 8");
        assert!(service.verify_solution(&PowSolution {
            challenge: challenge.challenge,
            solution: sol,
        }));
    }

    #[test]
    fn test_reuse_solution_fails() {
        let service = PowService::new(8, 300, 600);
        let challenge = service.generate_challenge();

        let input = format!("{}0", challenge.challenge);
        let hash = Sha256::digest(input.as_bytes());
        // Find any nonce
        let mut nonce = 0u64;
        for _ in 0..10000 {
            let input = format!("{}{}", challenge.challenge, nonce);
            let hash = Sha256::digest(input.as_bytes());
            if count_leading_zero_bytes(&hash) >= 1 {
                break;
            }
            nonce += 1;
        }

        let sol = PowSolution {
            challenge: challenge.challenge.clone(),
            solution: nonce.to_string(),
        };
        assert!(service.verify_solution(&sol));
        // Reuse fails
        assert!(!service.verify_solution(&sol));
    }
}
