//! User management service with Argon2id password hashing.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

#[derive(Debug)]
pub enum UserServiceError {
    PasswordHash(String),
    Storage(String),
    NotFound,
}

impl core::fmt::Display for UserServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PasswordHash(s) => write!(f, "Password hash error: {s}"),
            Self::Storage(s) => write!(f, "Storage error: {s}"),
            Self::NotFound => write!(f, "User not found"),
        }
    }
}

impl std::error::Error for UserServiceError {}

pub fn hash_password(password: &str) -> Result<String, UserServiceError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(65536, 3, 4, None)
            .map_err(|e| UserServiceError::PasswordHash(e.to_string()))?,
    );
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| UserServiceError::PasswordHash(e.to_string()))?;
    Ok(hash.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> Result<bool, UserServiceError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| UserServiceError::PasswordHash(e.to_string()))?;
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(UserServiceError::PasswordHash(e.to_string())),
    }
}

pub fn validate_password(
    password: &str,
    policy: &super::super::config::PasswordPolicy,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if password.len() < policy.min_length {
        errors.push(format!(
            "Password must be at least {} characters",
            policy.min_length
        ));
    }
    if policy.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
        errors.push("Password must contain an uppercase letter".into());
    }
    if policy.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
        errors.push("Password must contain a lowercase letter".into());
    }
    if policy.require_number && !password.chars().any(|c| c.is_ascii_digit()) {
        errors.push("Password must contain a number".into());
    }
    if policy.require_special && !password.chars().any(|c| !c.is_alphanumeric()) {
        errors.push("Password must contain a special character".into());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

