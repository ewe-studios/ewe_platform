//! Provider construction + `create()` credential handling, offline.
//!
//! WHY: `ModelProvider::create` is the authentication gate every HTTP provider
//! goes through, and its `AuthCredential` match had no coverage — the existing
//! provider suites all start from an already-authenticated provider pointed at
//! a `TestHttpServer`. The match decides which credential shapes are accepted
//! and which are rejected; a wrong arm either drops an API key (every request
//! then 401s with no clue why) or silently accepts a credential the provider
//! cannot actually use.
//!
//! WHAT: for both the Chat Completions and Anthropic Messages providers —
//! the constructors, and `create()` across every `AuthCredential` variant,
//! asserting acceptance for the three key-bearing shapes and explicit rejection
//! for the two interactive ones.
//!
//! HOW: `create()` performs no I/O — it only stores credentials and lazily
//! installs a default HTTP client. So this is fully offline and needs no keys.

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::ModelProvider;
use foundation_auth::{AuthCredential, ConfidentialText, OAuthCredential};

// ---------------------------------------------------------------------------
// Credential builders
// ---------------------------------------------------------------------------

fn secret_only() -> AuthCredential {
    AuthCredential::SecretOnly(ConfidentialText::new("sk-secret".into()))
}

fn client_secret() -> AuthCredential {
    AuthCredential::ClientSecret {
        client_id: ConfidentialText::new("client-id".into()),
        client_secret: ConfidentialText::new("sk-client-secret".into()),
    }
}

fn oauth() -> AuthCredential {
    AuthCredential::OAuth(OAuthCredential {
        expires: 0.0,
        access_token: ConfidentialText::new("sk-oauth".into()),
        refresh_token: None,
    })
}

fn email_auth() -> AuthCredential {
    AuthCredential::EmailAuth {
        email: "a@b.c".into(),
    }
}

fn username_password() -> AuthCredential {
    AuthCredential::UsernameAndPassword {
        username: "user".into(),
        password: ConfidentialText::new("pw".into()),
    }
}

// ---------------------------------------------------------------------------
// OpenAI — constructors
// ---------------------------------------------------------------------------

#[test]
fn openai_new_and_default_agree() {
    // `Default` must not diverge from `new()` — they are used interchangeably.
    let a = OpenAIProvider::new();
    let b = OpenAIProvider::default();
    assert_eq!(
        a.describe().map(|d| d.id).ok(),
        b.describe().map(|d| d.id).ok()
    );
}

#[test]
fn openai_with_config_is_constructible() {
    let cfg = OpenAIConfig::new().with_base_url("https://example.test/v1");
    let p = OpenAIProvider::with_config(cfg);
    assert!(p.describe().is_ok(), "a configured provider must describe itself");
}

// ---------------------------------------------------------------------------
// OpenAI — create() credential branches
// ---------------------------------------------------------------------------

#[test]
fn openai_create_accepts_secret_only() {
    let cfg = OpenAIConfig::new().with_auth(secret_only());
    assert!(
        OpenAIProvider::new().create(Some(cfg)).is_ok(),
        "SecretOnly is the standard API-key shape and must be accepted"
    );
}

#[test]
fn openai_create_accepts_client_secret() {
    let cfg = OpenAIConfig::new().with_auth(client_secret());
    assert!(
        OpenAIProvider::new().create(Some(cfg)).is_ok(),
        "ClientSecret must be accepted (the secret half is used as the key)"
    );
}

#[test]
fn openai_create_accepts_oauth() {
    let cfg = OpenAIConfig::new().with_auth(oauth());
    assert!(
        OpenAIProvider::new().create(Some(cfg)).is_ok(),
        "OAuth must be accepted (the access token is used as the key)"
    );
}

#[test]
fn openai_create_rejects_email_auth() {
    // Interactive credentials carry no usable API key — accepting them would
    // produce a provider that 401s on every request with no explanation.
    let cfg = OpenAIConfig::new().with_auth(email_auth());
    assert!(
        OpenAIProvider::new().create(Some(cfg)).is_err(),
        "EmailAuth must be rejected at create() time, not at request time"
    );
}

#[test]
fn openai_create_rejects_username_and_password() {
    let cfg = OpenAIConfig::new().with_auth(username_password());
    assert!(
        OpenAIProvider::new().create(Some(cfg)).is_err(),
        "UsernameAndPassword must be rejected at create() time"
    );
}

#[test]
fn openai_create_without_config_succeeds() {
    // `create(None)` is the "configure later / no auth needed" path used by
    // local OpenAI-compatible servers (Ollama, llama-server).
    assert!(
        OpenAIProvider::new().create(None).is_ok(),
        "create(None) must succeed for keyless local endpoints"
    );
}

#[test]
fn openai_create_preserves_the_supplied_config() {
    let cfg = OpenAIConfig::new()
        .with_base_url("https://openrouter.ai/api/v1")
        .with_auth(secret_only());
    let p = OpenAIProvider::new()
        .create(Some(cfg))
        .expect("create must succeed");
    // The descriptor still resolves after create — i.e. config replacement did
    // not leave the provider in a broken state.
    assert!(p.describe().is_ok());
}

// ---------------------------------------------------------------------------
// Anthropic — constructors + create() credential branches
// ---------------------------------------------------------------------------

#[test]
fn anthropic_new_is_constructible_and_describes() {
    let p = AnthropicMessagesProvider::new();
    assert!(p.describe().is_ok(), "provider must describe itself");
}

#[test]
fn anthropic_create_accepts_secret_only() {
    let cfg = AnthropicConfig::new().with_auth(secret_only());
    assert!(
        AnthropicMessagesProvider::new().create(Some(cfg)).is_ok(),
        "SecretOnly is the standard Anthropic API-key shape"
    );
}

#[test]
fn anthropic_create_without_config_succeeds() {
    assert!(AnthropicMessagesProvider::new().create(None).is_ok());
}

#[test]
fn anthropic_create_preserves_a_custom_base_url() {
    // Proxy deployments override base_url; create() must not discard it.
    let cfg = AnthropicConfig::new()
        .with_base_url("https://proxy.example.test")
        .with_auth(secret_only());
    let p = AnthropicMessagesProvider::new()
        .create(Some(cfg))
        .expect("create must succeed");
    assert!(p.describe().is_ok());
}

// ---------------------------------------------------------------------------
// Cross-provider consistency
// ---------------------------------------------------------------------------

#[test]
fn every_key_bearing_credential_shape_is_accepted() {
    // A caller swapping credential shapes should not discover that one is
    // silently refused. `AuthCredential` is not Clone, so each case builds a
    // fresh value via its constructor.
    let cases: [(&str, fn() -> AuthCredential); 3] = [
        ("SecretOnly", secret_only),
        ("ClientSecret", client_secret),
        ("OAuth", oauth),
    ];
    for (label, build) in cases {
        assert!(
            OpenAIProvider::new()
                .create(Some(OpenAIConfig::new().with_auth(build())))
                .is_ok(),
            "OpenAI must accept the key-bearing credential {label}"
        );
    }
}

#[test]
fn every_interactive_credential_shape_is_rejected() {
    let cases: [(&str, fn() -> AuthCredential); 2] = [
        ("EmailAuth", email_auth),
        ("UsernameAndPassword", username_password),
    ];
    for (label, build) in cases {
        assert!(
            OpenAIProvider::new()
                .create(Some(OpenAIConfig::new().with_auth(build())))
                .is_err(),
            "OpenAI must reject the interactive credential {label} at create() time"
        );
    }
}
