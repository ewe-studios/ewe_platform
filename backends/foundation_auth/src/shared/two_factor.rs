//! Two-factor authentication — TOTP (RFC 6238) and backup codes.
//!
//! WHY: MFA for sensitive auth flows and API key protection.
//!
//! WHAT: `TOTPSecret` for TOTP generation/verification, backup code support.
//! HOW: HMAC (SHA-1 by default, SHA-256 optional) with time steps. Synchronous — CPU-bound crypto ops.

use foundation_compact::rng::Rng;
use foundation_compact::time;
use foundation_core::valtron::Ctx;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::Sha256;
use zeroize::Zeroizing;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;

/// The hash a TOTP secret uses. RFC 6238 defaults to SHA-1, and it is the
/// only one most authenticator apps (Google Authenticator and others)
/// implement. SHA-256 is for apps that support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    /// HMAC-SHA-1, 6 digits, 30 s: the RFC 6238 default.
    #[default]
    Sha1,
    /// HMAC-SHA-256.
    Sha256,
}

impl TotpAlgorithm {
    /// The name in an `otpauth://` URI (`algorithm=`).
    #[must_use]
    pub fn uri_name(self) -> &'static str {
        match self {
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
        }
    }
}

/// TOTP secret for time-based one-time password generation (RFC 6238).
pub struct TOTPSecret {
    secret: Zeroizing<Vec<u8>>,
    period: u64,
    digits: usize,
    algorithm: TotpAlgorithm,
}

impl TOTPSecret {
    /// Generate a new random TOTP secret.
    ///
    /// Uses 32 bytes of randomness (256-bit).
    #[must_use]
    pub fn generate() -> Self {
        let mut secret = vec![0u8; 32];
        foundation_compact::rng::rng().fill_bytes(&mut secret);
        Self {
            secret: Zeroizing::new(secret),
            period: 30,
            digits: 6,
            algorithm: TotpAlgorithm::default(),
        }
    }

    /// Use this hash (default: SHA-1, the RFC 6238 default).
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: TotpAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// The hash this secret uses.
    #[must_use]
    pub fn algorithm(&self) -> TotpAlgorithm {
        self.algorithm
    }

    /// Create from an existing secret key.
    #[must_use]
    pub fn from_bytes(secret: Vec<u8>) -> Self {
        Self {
            secret: Zeroizing::new(secret),
            period: 30,
            digits: 6,
            algorithm: TotpAlgorithm::default(),
        }
    }

    /// Generate a TOTP code for the clock's current wall time.
    #[must_use]
    pub fn now(&self, ctx: &Ctx) -> String {
        let timestamp =
            u64::try_from(time::unix_seconds(ctx.clock().wall_now())).unwrap_or_default();
        self.code_at(timestamp)
    }

    /// Generate a TOTP code at a specific timestamp.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn code_at(&self, timestamp: u64) -> String {
        let time_step = timestamp / self.period;
        Self::generate_code(&self.secret, time_step, self.digits, self.algorithm)
    }

    /// Verify a TOTP code with time window tolerance (±1 step by default).
    ///
    /// Checks the current time step and `tolerance` steps before and after to
    /// account for clock drift.
    #[must_use]
    pub fn verify(&self, code: &str, tolerance: u64, ctx: &Ctx) -> bool {
        self.verify_step(code, tolerance, ctx).is_some()
    }

    /// Like [`verify`](Self::verify), but returns the time step the code
    /// belongs to. A caller that remembers the last accepted step can reject a
    /// code for the same or an earlier step (replay).
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn verify_step(&self, code: &str, tolerance: u64, ctx: &Ctx) -> Option<u64> {
        let timestamp =
            u64::try_from(time::unix_seconds(ctx.clock().wall_now())).unwrap_or_default();
        let time_step = timestamp / self.period;

        // Check every step in the window; do not stop at the first match, so
        // the work does not depend on which step matched.
        let mut matched = None;
        for step in time_step.saturating_sub(tolerance)..=time_step + tolerance {
            if code == Self::generate_code(&self.secret, step, self.digits, self.algorithm) {
                matched = Some(step);
            }
        }
        matched
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

    /// Create from a base32 string (RFC 4648, no padding), the inverse of
    /// [`to_base32`](Self::to_base32). Returns `None` for a character outside
    /// the base32 alphabet or an empty string.
    #[must_use]
    pub fn from_base32(encoded: &str) -> Option<Self> {
        if encoded.is_empty() {
            return None;
        }
        let mut out = Vec::with_capacity(encoded.len() * 5 / 8);
        let mut buffer: u32 = 0;
        let mut bits = 0u32;
        for ch in encoded.bytes() {
            let val = match ch {
                b'A'..=b'Z' => u32::from(ch - b'A'),
                b'2'..=b'7' => u32::from(ch - b'2') + 26,
                _ => return None,
            };
            buffer = (buffer << 5) | val;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                out.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
                buffer &= (1 << bits) - 1;
            }
        }
        Some(Self::from_bytes(out))
    }

    /// Generate the HOTP code for a given time step.
    #[allow(clippy::cast_possible_truncation)]
    fn generate_code(
        secret: &[u8],
        time_step: u64,
        digits: usize,
        algorithm: TotpAlgorithm,
    ) -> String {
        // Encode time step as 8-byte big-endian counter
        let counter = time_step.to_be_bytes();

        // Compute HMAC-SHA256
        let result: Vec<u8> = match algorithm {
            TotpAlgorithm::Sha1 => {
                let mut mac =
                    HmacSha1::new_from_slice(secret).expect("HMAC can take key of any size");
                mac.update(&counter);
                mac.finalize().into_bytes().to_vec()
            }
            TotpAlgorithm::Sha256 => {
                let mut mac =
                    HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
                mac.update(&counter);
                mac.finalize().into_bytes().to_vec()
            }
        };

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
                foundation_compact::rng::rng().fill_bytes(&mut bytes);
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
///
/// Slices of different length compare as not equal.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
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
