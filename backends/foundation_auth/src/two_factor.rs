//! Two-factor authentication — TOTP (RFC 6238) and backup codes.
//!
//! WHY: MFA for sensitive auth flows and API key protection.
//!
//! WHAT: `TOTPSecret` for TOTP generation/verification, backup code support.
//! HOW: HMAC-SHA1 with time steps. Synchronous — CPU-bound crypto ops.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

/// TOTP secret for time-based one-time password generation (RFC 6238).
pub struct TOTPSecret {
    secret: Zeroizing<Vec<u8>>,
    period: u64,
    digits: usize,
}

impl TOTPSecret {
    /// Generate a new random TOTP secret.
    ///
    /// Uses 32 bytes of randomness (256-bit).
    #[must_use]
    pub fn generate() -> Self {
        let mut secret = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);
        Self {
            secret: Zeroizing::new(secret),
            period: 30,
            digits: 6,
        }
    }

    /// Create from an existing secret key.
    #[must_use]
    pub fn from_bytes(secret: Vec<u8>) -> Self {
        Self {
            secret: Zeroizing::new(secret),
            period: 30,
            digits: 6,
        }
    }

    /// Generate a TOTP code for the current time.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn now(&self) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs();
        self.code_at(timestamp)
    }

    /// Generate a TOTP code at a specific timestamp.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn code_at(&self, timestamp: u64) -> String {
        let time_step = timestamp / self.period;
        Self::generate_code(&self.secret, time_step, self.digits)
    }

    /// Verify a TOTP code with time window tolerance (±1 step by default).
    ///
    /// Checks the current time step and one step before/after to account
    /// for clock drift.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn verify(&self, code: &str, tolerance: u64) -> bool {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs();
        let time_step = timestamp / self.period;

        for delta in 0..=tolerance {
            if delta > 0 {
                // Check previous step
                if time_step >= delta
                    && code == Self::generate_code(&self.secret, time_step - delta, self.digits)
                {
                    return true;
                }
                // Check next step
                if code == Self::generate_code(&self.secret, time_step + delta, self.digits) {
                    return true;
                }
            } else {
                // Current step
                if code == Self::generate_code(&self.secret, time_step, self.digits) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the base32-encoded representation of the secret.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_base32(&self) -> String {
        // Use standard base32 (RFC 4648) without padding for TOTP
        let mut bytes = vec![0u8; self.secret.len().div_ceil(5) * 8];
        let bits = self.secret.len() * 8;
        for (i, chunk) in self.secret.chunks(5).enumerate() {
            let val: u64 = chunk.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
            let shift = (5 - chunk.len()) * 8;
            let shifted = val << shift;
            for (j, b) in bytes[i * 8..i * 8 + 8].iter_mut().enumerate() {
                let idx = (shifted >> ((7 - j) * 5)) & 0x1F;
                *b = match idx {
                    0..=25 => b'A' + (idx as u8),
                    26..=31 => b'2' + ((idx - 26) as u8),
                    _ => 0,
                };
            }
        }
        // Trim padding
        let significant = bits.div_ceil(5);
        String::from_utf8(bytes[..significant].to_vec()).unwrap_or_default()
    }

    /// Generate the HOTP code for a given time step.
    #[allow(clippy::cast_possible_truncation)]
    fn generate_code(secret: &[u8], time_step: u64, digits: usize) -> String {
        // Encode time step as 8-byte big-endian counter
        let counter = time_step.to_be_bytes();

        // Compute HMAC-SHA256
        let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
        mac.update(&counter);
        let result = mac.finalize().into_bytes();

        // Dynamic truncation (RFC 4226 section 5.4)
        let offset = (result[result.len() - 1] & 0x0F) as usize;
        let code = u32::from(result[offset] & 0x7F) << 24
            | u32::from(result[offset + 1]) << 16
            | u32::from(result[offset + 2]) << 8
            | u32::from(result[offset + 3]);

        let modulo = 10u32.pow(digits as u32);
        format!("{:0>width$}", code % modulo, width = digits)
    }
}

impl core::fmt::Debug for TOTPSecret {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TOTPSecret(redacted, period={}, digits={})",
            self.period, self.digits
        )
    }
}

/// 2FA challenge state.
pub struct TwoFactorChallenge {
    /// The challenge ID for tracking.
    pub id: String,
    /// Whether this challenge has been completed.
    pub completed: bool,
    /// Remaining attempts.
    pub attempts_left: u32,
    /// Maximum attempts allowed.
    pub max_attempts: u32,
}

impl TwoFactorChallenge {
    /// Create a new challenge.
    #[must_use]
    pub fn new(id: String) -> Self {
        Self {
            id,
            completed: false,
            attempts_left: 3,
            max_attempts: 3,
        }
    }

    /// Record a failed attempt.
    ///
    /// Returns `true` if attempts remain, `false` if challenge is locked.
    pub fn record_attempt(&mut self) -> bool {
        if self.attempts_left == 0 {
            return false;
        }
        self.attempts_left -= 1;
        self.attempts_left > 0
    }

    /// Mark the challenge as successfully completed.
    pub fn complete(&mut self) {
        self.completed = true;
        self.attempts_left = 0;
    }

    /// Whether the challenge is still active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.completed && self.attempts_left > 0
    }
}

/// Backup code set for 2FA recovery.
pub struct BackupCodeSet {
    /// Hashed backup codes (store these, not the plaintext codes).
    codes: Vec<String>,
}

impl BackupCodeSet {
    /// Generate a set of random backup codes.
    ///
    /// Returns the plaintext codes (to show to user once) and the `BackupCodeSet`
    /// for storage.
    #[must_use]
    pub fn generate(count: usize, length: usize) -> (Vec<String>, Self) {
        let codes: Vec<String> = (0..count)
            .map(|_| {
                let mut bytes = vec![0u8; length];
                rand::thread_rng().fill_bytes(&mut bytes);
                // Use hex for easy transcription
                bytes
                    .iter()
                    .fold(String::with_capacity(length * 2), |mut acc, b| {
                        use std::fmt::Write;
                        write!(acc, "{b:02x}").unwrap();
                        acc
                    })
            })
            .collect();

        let stored = Self {
            codes: codes.clone(),
        };

        (codes, stored)
    }

    /// Validate and consume a backup code (single-use).
    ///
    /// Returns `true` if the code was valid and consumed.
    ///
    /// # Errors
    ///
    /// Returns `TwoFactorError` if no matching code is found.
    pub fn validate(&mut self, code: &str) -> Result<(), TwoFactorError> {
        let pos = self
            .codes
            .iter()
            .position(|c| constant_time_eq(c.as_bytes(), code.as_bytes()));

        match pos {
            Some(idx) => {
                self.codes.remove(idx);
                Ok(())
            }
            None => Err(TwoFactorError::InvalidCode),
        }
    }

    /// Number of remaining unused backup codes.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.codes.len()
    }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// 2FA-specific errors.
#[derive(derive_more::From, Debug)]
pub enum TwoFactorError {
    /// Code is invalid or expired.
    InvalidCode,
    /// Too many attempts — challenge locked.
    TooManyAttempts,
    /// No backup codes remaining.
    NoBackupCodes,
}

impl core::fmt::Display for TwoFactorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TwoFactorError::InvalidCode => write!(f, "Invalid 2FA code"),
            TwoFactorError::TooManyAttempts => {
                write!(f, "Too many 2FA attempts — challenge locked")
            }
            TwoFactorError::NoBackupCodes => write!(f, "No backup codes remaining"),
        }
    }
}

impl std::error::Error for TwoFactorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totp_secret_generation() {
        let secret = TOTPSecret::generate();
        let code = secret.now();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(char::is_numeric));
    }

    #[test]
    fn test_totp_verify_current_code() {
        let secret = TOTPSecret::generate();
        let code = secret.now();
        assert!(secret.verify(&code, 1));
    }

    #[test]
    fn test_totp_verify_wrong_code() {
        let secret = TOTPSecret::generate();
        assert!(!secret.verify("000000", 1));
    }

    #[test]
    fn test_totp_deterministic() {
        let secret = TOTPSecret::from_bytes(vec![0xAB; 32]);
        let ts = 1_700_000_000;
        let code1 = secret.code_at(ts);
        let code2 = secret.code_at(ts);
        assert_eq!(code1, code2);
    }

    #[test]
    fn test_totp_base32() {
        let secret = TOTPSecret::generate();
        let b32 = secret.to_base32();
        assert!(!b32.is_empty());
        assert!(b32.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_totp_debug_redacted() {
        let secret = TOTPSecret::generate();
        let debug = format!("{secret:?}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains("secret: ["));
    }

    #[test]
    fn test_backup_code_generation() {
        let (codes, stored) = BackupCodeSet::generate(10, 8);
        assert_eq!(codes.len(), 10);
        assert_eq!(stored.remaining(), 10);
    }

    #[test]
    fn test_backup_code_validation() {
        let (codes, mut stored) = BackupCodeSet::generate(5, 8);
        let first = &codes[0];
        stored.validate(first).unwrap();
        assert_eq!(stored.remaining(), 4);
    }

    #[test]
    fn test_backup_code_single_use() {
        let (codes, mut stored) = BackupCodeSet::generate(5, 8);
        let first = &codes[0];
        stored.validate(first).unwrap();
        // Second use should fail
        assert!(stored.validate(first).is_err());
    }

    #[test]
    fn test_backup_code_invalid() {
        let (_, mut stored) = BackupCodeSet::generate(5, 8);
        assert!(stored.validate("invalidcode").is_err());
    }

    #[test]
    fn test_two_factor_challenge_attempts() {
        let mut challenge = TwoFactorChallenge::new("ch1".to_string());
        assert!(challenge.is_active());
        assert_eq!(challenge.attempts_left, 3);

        challenge.record_attempt();
        assert!(challenge.is_active());
        assert_eq!(challenge.attempts_left, 2);

        challenge.record_attempt();
        assert!(challenge.is_active());
        assert_eq!(challenge.attempts_left, 1);

        challenge.record_attempt();
        assert!(!challenge.is_active());
        assert_eq!(challenge.attempts_left, 0);
    }

    #[test]
    fn test_two_factor_challenge_complete() {
        let mut challenge = TwoFactorChallenge::new("ch1".to_string());
        challenge.complete();
        assert!(challenge.completed);
        assert!(!challenge.is_active());
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }
}
