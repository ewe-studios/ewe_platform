use foundation_auth::{BackupCodeSet, TOTPSecret, TwoFactorChallenge};

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
