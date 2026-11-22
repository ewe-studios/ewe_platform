//! Tests for the auth primitives of specification 86.
//!
//! WHY: The workspace needs one secret type, one bearer parser, one
//! constant-time comparison, and one client-authentication trait. A second copy
//! of a security primitive is a defect that a build cannot see, so some tests
//! scan the source tree.
//!
//! WHAT: Tests for `Secret`, the bearer parser, `constant_time_eq`, `AuthInfo`,
//! `ClientAuth` and `StaticToken`.
//!
//! HOW: Plain `#[test]` functions for the synchronous values and
//! `#[valtron_test]` for the async `ClientAuth::authenticate` call.
// Tests measure real time on purpose.
#![allow(clippy::disallowed_methods)]


use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use foundation_auth::server_credential::extract_bearer_token as server_extract_bearer_token;
use foundation_auth::{
    bearer_header, extract_bearer_token, AuthError, AuthInfo, AuthMethod, CallCredentials,
    ClientAuth, Principal, Secret, StaticToken,
};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeader, SimpleHeaders};

/// A 32-byte token, the shape that a shared secret usually has.
const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// A wrong token of the same length.
const WRONG: &str = "fedcba9876543210fedcba9876543210";

// ---------------------------------------------------------------------------
// Source scans
// ---------------------------------------------------------------------------

/// The workspace `backends` directory, when the tests run inside the workspace.
///
/// `CARGO_MANIFEST_DIR` is `<workspace>/backends/foundation_auth`. A packaged
/// crate has no `backends` directory, so the source scans return `None` and
/// skip.
fn workspace_backends_dir() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent()?.parent()?;
    let backends = workspace.join("backends");
    backends.is_dir().then_some(backends)
}

/// Collect every Rust source file under `dir`, without following symlinks.
///
/// Build outputs and vendored dependency trees are skipped. `DirEntry::file_type`
/// does not follow a symlink, so a symlinked directory (for example the
/// `whisper.cpp` checkout) is not walked.
fn rust_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(name.as_ref(), "target" | ".git" | "node_modules") {
                continue;
            }
            rust_source_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// S7 — the workspace has exactly one constant-time comparison function.
///
/// The needle is built at run time with `concat!`, so this test file does not
/// match itself.
#[test]
fn one_constant_time_eq() {
    let Some(backends) = workspace_backends_dir() else {
        return;
    };

    let needle = concat!("fn constant", "_time_eq");
    let mut files = Vec::new();
    rust_source_files(&backends, &mut files);

    let mut hits = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if line.contains(needle) {
                hits.push((file.clone(), index + 1));
            }
        }
    }

    assert_eq!(
        hits.len(),
        1,
        "exactly one constant-time comparison function must exist in the workspace; \
         found {hits:#?}"
    );

    // Compare path components, so the check passes on Windows too.
    let expected = Path::new("foundation_auth")
        .join("src")
        .join("shared")
        .join("mod.rs");
    assert!(
        hits[0].0.ends_with(&expected),
        "the one comparison must live in foundation_auth::shared, found {hits:#?}"
    );
}

// ---------------------------------------------------------------------------
// Secret
// ---------------------------------------------------------------------------

/// S1 — a secret never prints.
#[test]
fn secret_is_redacted() {
    let secret = Secret::new("hunter2");
    let debug = format!("{secret:?}");
    let display = format!("{secret}");

    assert_eq!(debug, "Secret(<redacted>)");
    assert_eq!(display, "<redacted>");
    assert!(!debug.contains("hunter2"));
    assert!(!display.contains("hunter2"));

    // The output does not depend on the text or its length.
    assert_eq!(format!("{}", Secret::new("a")), display);
    assert_eq!(format!("{:?}", Secret::new("a much longer value")), debug);
}

/// A secret equals itself and a one-character change is unequal.
#[test]
fn secret_eq() {
    let values = [
        "",
        "a",
        "password",
        "Bearer token",
        "0123456789abcdef0123456789abcdef",
    ];
    for value in values {
        let secret = Secret::new(value);
        assert!(secret.ct_eq(&Secret::new(value)));
        assert_eq!(secret, Secret::new(value));

        for (index, character) in value.char_indices() {
            let replacement = if character == 'X' { "Y" } else { "X" };
            let mut changed = value.to_string();
            changed.replace_range(index..index + character.len_utf8(), replacement);
            assert_ne!(
                secret,
                Secret::new(changed.clone()),
                "a change at byte {index} must not compare equal"
            );
            assert!(!secret.ct_eq(&Secret::new(changed)));
        }

        // A different length is unequal.
        assert_ne!(secret, Secret::new(format!("{value}!")));
    }
}

/// The serde form is the text, so a JSON manifest can hold a secret.
#[test]
fn secret_serde_round_trip() {
    let secret = Secret::new("json-secret");
    let json = serde_json::to_string(&secret).expect("serialize secret");
    assert_eq!(json, "\"json-secret\"");

    let parsed: Secret = serde_json::from_str(&json).expect("deserialize secret");
    assert_eq!(parsed, secret);
    assert_eq!(parsed.expose(), "json-secret");
}

// ---------------------------------------------------------------------------
// Bearer parsing
// ---------------------------------------------------------------------------

/// S5 — the scheme is case-insensitive.
#[test]
fn bearer_case_insensitive() {
    assert_eq!(
        extract_bearer_token(Some("bearer abc")),
        Some("abc".to_string())
    );
    assert_eq!(
        extract_bearer_token(Some("BEARER abc")),
        Some("abc".to_string())
    );
    assert_eq!(
        server_extract_bearer_token(Some("BeArEr abc")),
        Some("abc".to_string())
    );
    assert_eq!(extract_bearer_token(Some("Basic abc")), None);
    assert_eq!(extract_bearer_token(None), None);
}

/// S4 — a header with a control byte is refused.
#[test]
fn bearer_control_byte() {
    let header = "Bearer tok\0en";
    assert_eq!(Secret::from_bearer(Some(header)), None);
    assert_eq!(extract_bearer_token(Some(header)), None);
    assert_eq!(server_extract_bearer_token(Some(header)), None);
}

/// A token with a space is not a token.
#[test]
fn bearer_no_space() {
    assert_eq!(extract_bearer_token(Some("Bearer abc def")), None);
    assert_eq!(Secret::from_bearer(Some("Bearer  abc")), None);
    assert_eq!(Secret::from_bearer(Some("Bearer ")), None);
    assert_eq!(Secret::from_bearer(Some("Bearer")), None);
}

/// `bearer_header` round-trips through the parser.
#[test]
fn bearer_round_trip() {
    let tokens = ["abc", "a-b_c.d~e", TOKEN, "XYZ!", "~"];
    for token in tokens {
        let secret = Secret::new(token);
        let header = bearer_header(&secret);
        assert_eq!(header, format!("Bearer {token}"));
        assert_eq!(extract_bearer_token(Some(&header)), Some(token.to_string()));
        assert_eq!(
            server_extract_bearer_token(Some(&header)),
            Some(token.to_string())
        );
    }
}

// ---------------------------------------------------------------------------
// StaticToken
// ---------------------------------------------------------------------------

/// The facts of one call, with a lifetime that outlives the future.
struct Fixture {
    headers: SimpleHeaders,
    connection: ConnectionContext,
    extensions: Extensions,
}

impl Fixture {
    fn new(header: Option<&str>) -> Self {
        let mut headers = SimpleHeaders::new();
        if let Some(value) = header {
            headers.insert(SimpleHeader::AUTHORIZATION, vec![value.to_string()]);
        }
        Self {
            headers,
            connection: ConnectionContext::default(),
            extensions: Extensions::new(),
        }
    }

    fn call(&self) -> CallCredentials<'_> {
        CallCredentials {
            procedure: "/ewe.test.v1.TestService/Check",
            headers: &self.headers,
            connection: &self.connection,
            extensions: &self.extensions,
        }
    }
}

/// S3 — a missing header is `Ok(None)`.
#[valtron_test]
async fn static_token_no_header_is_none() {
    let provider = StaticToken::new(Secret::new(TOKEN));
    let fixture = Fixture::new(None);

    let result = provider
        .authenticate(fixture.call())
        .await
        .expect("a missing header is not an error");
    assert!(result.is_none());
}

/// A header of another scheme is no credential of this kind.
#[valtron_test]
async fn static_token_missing() {
    let provider = StaticToken::new(Secret::new(TOKEN));
    let fixture = Fixture::new(Some("Basic dXNlcjpwYXNz"));

    let result = provider
        .authenticate(fixture.call())
        .await
        .expect("another scheme is not an error");
    assert!(result.is_none());
}

/// The matching token gives the shared principal.
#[valtron_test]
async fn static_token_accepts_match() {
    let provider = StaticToken::new(Secret::new(TOKEN)).named("control-plane");
    let fixture = Fixture::new(Some(&bearer_header(&Secret::new(TOKEN))));

    let result = provider
        .authenticate(fixture.call())
        .await
        .expect("the token matches")
        .expect("the caller is known");

    assert_eq!(result.context.sub.as_deref(), Some("control-plane"));
    assert_eq!(result.context.path, "/ewe.test.v1.TestService/Check");

    let principal: &Principal = result
        .artifacts
        .get::<Principal>()
        .expect("the provider records the principal");
    assert_eq!(
        principal.method,
        AuthMethod::SharedToken {
            name: "control-plane".to_string()
        }
    );
}

/// S2 — a wrong token is refused, and the comparison is constant-time.
///
/// The time of a comparison whose first byte differs must match the time of a
/// comparison whose last byte differs. An early-return comparison would make
/// the first-byte case much faster, and the ratio would leave the bounded
/// window.
#[valtron_test]
async fn static_token_constant_time() {
    let provider = StaticToken::new(Secret::new(TOKEN));

    // The result for a wrong token of the same length is `Invalid`.
    let fixture = Fixture::new(Some(&bearer_header(&Secret::new(WRONG))));
    let error = provider
        .authenticate(fixture.call())
        .await
        .expect_err("a wrong token is refused");
    assert!(matches!(error, AuthError::Invalid(_)), "got {error:?}");

    // Isolate the comparison itself. Both changes stay in the ASCII range.
    let mut first_difference = TOKEN.as_bytes().to_vec();
    first_difference[0] = b'X';
    let mut last_difference = TOKEN.as_bytes().to_vec();
    last_difference[31] = b'Y';

    let expected = Secret::new(TOKEN);
    let first = Secret::new(String::from_utf8(first_difference).expect("ascii"));
    let last = Secret::new(String::from_utf8(last_difference).expect("ascii"));

    // Warm up the code path and the branch predictor.
    for _ in 0..1_000 {
        black_box(expected.ct_eq(black_box(&first)));
        black_box(expected.ct_eq(black_box(&last)));
    }

    let measure = |candidate: &Secret| {
        let start = Instant::now();
        for _ in 0..1_000 {
            black_box(expected.ct_eq(black_box(candidate)));
        }
        start.elapsed().as_secs_f64()
    };

    let first_time = measure(&first);
    let last_time = measure(&last);

    assert!(!expected.ct_eq(&first));
    assert!(!expected.ct_eq(&last));

    let ratio = first_time / last_time;
    assert!(
        (0.25..4.0).contains(&ratio),
        "constant-time comparison must not depend on the differing byte: \
         first-byte {first_time:.9}s, last-byte {last_time:.9}s, ratio {ratio:.3}"
    );
}

/// The error text holds no credential.
#[valtron_test]
async fn auth_error_no_secret() {
    let provider = StaticToken::new(Secret::new(TOKEN));
    let fixture = Fixture::new(Some(&bearer_header(&Secret::new(WRONG))));
    let error = provider
        .authenticate(fixture.call())
        .await
        .expect_err("a wrong token is refused");

    for haystack in [format!("{error}"), format!("{error:?}")] {
        assert!(
            !haystack.contains(TOKEN),
            "error leaks the token: {haystack}"
        );
        assert!(
            !haystack.contains(WRONG),
            "error leaks the presented token: {haystack}"
        );
    }

    // Every variant formats without a credential.
    let variants = [
        AuthError::Invalid("invalid bearer token".to_string()),
        AuthError::Expired,
        AuthError::NotYetValid,
        AuthError::Revoked,
        AuthError::Spent,
        AuthError::Forbidden("role missing".to_string()),
        AuthError::Unavailable("store offline".to_string()),
        AuthError::Misconfigured("no signing key".to_string()),
    ];
    for variant in &variants {
        for haystack in [format!("{variant}"), format!("{variant:?}")] {
            assert!(!haystack.contains(TOKEN));
            assert!(!haystack.contains(WRONG));
        }
    }
}

/// `AuthInfo` keeps its typed artifacts.
#[test]
fn auth_info_artifacts() {
    let context = foundation_auth::AuthContext::new("/proc".to_string(), None, None);
    let info: AuthInfo = AuthInfo::new(context).with_artifact(7u32);
    assert_eq!(info.artifacts.get::<u32>(), Some(&7));
}

/// The trait is object-safe, so a server can hold `Arc<dyn ClientAuth>`.
#[test]
fn client_auth_is_object_safe() {
    let provider: std::sync::Arc<dyn ClientAuth> =
        std::sync::Arc::new(StaticToken::new(Secret::new(TOKEN)));
    assert_eq!(provider.mode(), "shared-token");
}
