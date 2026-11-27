//! The registry authentication tests (feature 15, section 6.3).
//!
//! The tests use the fake registry and its token service on a local socket.
//! They need no network, no Docker and no virtual machine.

use std::sync::Arc;

use foundation_sandbox::native::artifacts::sha256_hex;
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::registry::{
    parse_www_authenticate, OciDescriptor, OciReference, RegistryAuth, RegistryClient,
    RegistryOptions,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::Secret;
use serde_json::json;

use super::registry_server::{BlobBody, Challenge, FakeRegistry, BASIC_USER_PASS, TEST_TOKEN};
use super::support::TempDir;

/// The media type of an uncompressed layer.
const PLAIN_LAYER: &str = "application/vnd.oci.image.layer.v1.tar";
/// The media type of an image manifest.
const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// The media type of an image configuration.
const OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";

/// A client over a new temporary cache directory.
fn make_client(registry: &FakeRegistry, options: RegistryOptions) -> (RegistryClient, TempDir) {
    let directory = TempDir::new("oci-auth");
    let ctx = crate::control::support::ctx(0x0a1).0;
    let options = RegistryOptions {
        allow_insecure: vec![registry.address().to_string()],
        ..options
    };
    let client = RegistryClient::new(
        ctx,
        Arc::new(NativeFs),
        directory.path().join("cache"),
        options,
    );
    (client, directory)
}

/// The options with the `user:pass` credentials of `registry`.
fn credentials(registry: &FakeRegistry) -> RegistryOptions {
    RegistryOptions {
        auth: vec![(
            registry.address().to_string(),
            RegistryAuth::Basic {
                username: "user".to_string(),
                password: Secret::new("pass"),
            },
        )],
        ..RegistryOptions::default()
    }
}

/// One reference to `/test/image` with `text` as its tag.
fn reference(registry: &FakeRegistry, text: &str) -> OciReference {
    let address = registry.address();
    OciReference::parse(&format!("{address}/test/image:{text}")).expect("the test reference parses")
}

/// One image manifest body with no layers.
fn manifest_body(registry: &FakeRegistry) -> (Vec<u8>, String) {
    let config = registry.add_blob(b"{\"architecture\":\"amd64\",\"os\":\"linux\"}".to_vec());
    let body = serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": OCI_MANIFEST,
        "config": {"mediaType": OCI_CONFIG, "digest": config, "size": 39},
        "layers": [],
    }))
    .expect("the manifest encodes");
    let digest = format!("sha256:{}", sha256_hex(&body));
    (body, digest)
}

#[test]
fn www_authenticate_cases() {
    // The three keys of a bearer challenge, in the usual order.
    let challenge = parse_www_authenticate(
        "Bearer realm=\"https://auth.example/token\",service=\"registry.example\",scope=\"repository:a/b:pull\"",
    )
    .expect("the bearer challenge parses");
    assert_eq!(challenge.scheme, "Bearer");
    assert!(challenge.is_bearer());
    assert_eq!(challenge.param("realm"), Some("https://auth.example/token"));
    assert_eq!(challenge.param("service"), Some("registry.example"));
    assert_eq!(challenge.param("scope"), Some("repository:a/b:pull"));

    // Any order, an extra key and a mixed case.
    let challenge = parse_www_authenticate(
        "Bearer scope=\"repository:a/b:pull\", extra=1, service=\"registry.example\", realm=\"https://auth.example/token\"",
    )
    .expect("the reordered challenge parses");
    assert_eq!(challenge.param("realm"), Some("https://auth.example/token"));
    assert_eq!(challenge.param("service"), Some("registry.example"));
    assert_eq!(challenge.param("scope"), Some("repository:a/b:pull"));

    // A value without quotes.
    let challenge = parse_www_authenticate("Bearer realm=https://auth.example/token")
        .expect("the unquoted realm parses");
    assert_eq!(challenge.param("realm"), Some("https://auth.example/token"));

    // Basic credentials.
    let challenge =
        parse_www_authenticate("Basic realm=\"the registry\"").expect("the basic challenge parses");
    assert_eq!(challenge.scheme, "Basic");
    assert!(challenge.is_basic());
    assert_eq!(challenge.param("realm"), Some("the registry"));

    // A header with both challenges: the bearer one wins.
    let challenge = parse_www_authenticate("Basic realm=\"a\", Bearer realm=\"https://b/token\"")
        .expect("the mixed challenge parses");
    assert!(challenge.is_bearer());
    assert_eq!(challenge.param("realm"), Some("https://b/token"));

    // A header with no known scheme.
    assert!(parse_www_authenticate("Negotiate abc").is_none());
    assert!(parse_www_authenticate("").is_none());
}

#[test]
fn bearer_flow() {
    let registry = FakeRegistry::start();
    let (body, digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    {
        let mut state = registry.state();
        state.challenge = Challenge::Bearer {
            token: TEST_TOKEN.to_string(),
        };
        state.token_value = Some(TEST_TOKEN.to_string());
        state.token_basic = Some(BASIC_USER_PASS.to_string());
    }
    let (client, _directory) = make_client(&registry, credentials(&registry));
    let resolved = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect("the public manifest resolves with a token");
    assert_eq!(resolved.digest, digest);

    assert_eq!(
        registry.count_path("/token"),
        1,
        "the token service got one request"
    );
    assert_eq!(
        registry.count_path("/manifests/1"),
        2,
        "the manifest request was sent again with the token, one time"
    );
    let requests = registry.requests();
    let token = requests
        .iter()
        .find(|request| request.path.starts_with("/token"))
        .expect("the token request");
    assert_eq!(
        token.authorization(),
        Some(BASIC_USER_PASS),
        "the token request carries the caller's credentials"
    );
    assert!(
        token.path.contains("scope=repository:test/image:pull"),
        "the token request has the repository pull scope: {}",
        token.path
    );
    let manifest = requests
        .iter()
        .rev()
        .find(|request| request.path.contains("/manifests/1"))
        .expect("the second manifest request");
    assert_eq!(
        manifest.authorization(),
        Some(format!("Bearer {TEST_TOKEN}").as_str()),
        "the second manifest request carries the token"
    );
}

#[test]
fn second_401_is_unauthorized() {
    let registry = FakeRegistry::start();
    let (body, _digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    {
        let mut state = registry.state();
        state.challenge = Challenge::Bearer {
            token: "the good token".to_string(),
        };
        // The token service gives a token that the registry does not accept.
        state.token_value = Some("the bad token".to_string());
    }
    let (client, _directory) = make_client(&registry, RegistryOptions::default());
    let error = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect_err("a second 401 is Unauthorized");
    assert!(
        matches!(error.current_context(), SandboxError::Unauthorized),
        "the error is Unauthorized: {error:?}"
    );
    assert_eq!(
        registry.count_path("/token"),
        1,
        "the client asked for a token one time"
    );
    assert_eq!(
        registry.count_path("/manifests/1"),
        2,
        "the client retried one time and stopped"
    );
}

#[test]
fn basic_flow() {
    let registry = FakeRegistry::start();
    let (body, digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    registry.state().challenge = Challenge::Basic {
        authorization: BASIC_USER_PASS.to_string(),
    };
    let (client, _directory) = make_client(&registry, credentials(&registry));
    let resolved = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect("the manifest resolves with basic credentials");
    assert_eq!(resolved.digest, digest);
    assert_eq!(
        registry.count_path("/manifests/1"),
        2,
        "the client retried one time with the credentials"
    );
}

#[test]
fn basic_flow_without_credentials() {
    let registry = FakeRegistry::start();
    let (body, _digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    registry.state().challenge = Challenge::Basic {
        authorization: BASIC_USER_PASS.to_string(),
    };
    let (client, _directory) = make_client(&registry, RegistryOptions::default());
    let error = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect_err("a basic challenge without credentials is Unauthorized");
    assert!(
        matches!(error.current_context(), SandboxError::Unauthorized),
        "the error is Unauthorized: {error:?}"
    );
    assert_eq!(
        registry.count_path("/manifests/1"),
        1,
        "the client does not retry without credentials"
    );
}

#[test]
fn redirect_drops_authorization() {
    let registry = FakeRegistry::start();
    let storage = FakeRegistry::start_on_host("127.0.0.1", "localhost");
    let bytes = b"a redirected layer".to_vec();
    let digest = format!("sha256:{}", sha256_hex(&bytes));
    registry.add_blob_at(&digest, BlobBody::Bytes(bytes.clone()));
    storage.add_blob_at(&digest, BlobBody::Bytes(bytes.clone()));
    {
        let mut state = registry.state();
        state.challenge = Challenge::Bearer {
            token: TEST_TOKEN.to_string(),
        };
        state.token_value = Some(TEST_TOKEN.to_string());
        state.token_basic = Some(BASIC_USER_PASS.to_string());
        state.redirect_blobs_to = Some(storage.base_url());
    }
    let options = RegistryOptions {
        auth: credentials(&registry).auth,
        ..RegistryOptions::default()
    };
    let (client, _directory) = make_client(&registry, options);
    let descriptor = OciDescriptor {
        media_type: PLAIN_LAYER.to_string(),
        digest: digest.clone(),
        size: u64::try_from(bytes.len()).unwrap(),
    };
    let path = crate::pool::block_on(client.fetch_blob(
        &reference(&registry, "1"),
        &descriptor,
        &mut |_| {},
    ))
    .expect("the redirected blob arrives");
    assert_eq!(
        std::fs::read(&path).expect("the blob file"),
        bytes,
        "the stored blob has the storage host's bytes"
    );
    assert_eq!(
        storage.blob_serves(&digest),
        1,
        "the storage host served the blob"
    );
    let storage_requests = storage.requests();
    let storage_request = storage_requests
        .iter()
        .find(|request| request.path.contains("/storage/"))
        .expect("the storage host got the redirected request");
    assert_eq!(
        storage_request.authorization(),
        None,
        "the storage host got no Authorization header"
    );
    assert!(
        registry
            .requests()
            .iter()
            .any(|request| request.authorization() == Some(format!("Bearer {TEST_TOKEN}").as_str())),
        "the registry got the token"
    );
}

#[test]
fn secrets_not_in_debug_or_errors() {
    let auth = RegistryAuth::Basic {
        username: "alice".to_string(),
        password: Secret::new("hunter2"),
    };
    let debug = format!("{auth:?}");
    assert!(
        !debug.contains("hunter2"),
        "the password is redacted: {debug}"
    );
    assert!(
        !debug.contains("alice"),
        "the user name is redacted: {debug}"
    );

    let options = RegistryOptions {
        auth: vec![("registry.example".to_string(), auth)],
        ..RegistryOptions::default()
    };
    let debug = format!("{options:?}");
    assert!(
        !debug.contains("hunter2"),
        "the password is redacted: {debug}"
    );
    assert!(
        !debug.contains("alice"),
        "the user name is redacted: {debug}"
    );

    // A client that holds a token does not show it.
    let registry = FakeRegistry::start();
    let (body, _digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    {
        let mut state = registry.state();
        state.challenge = Challenge::Bearer {
            token: TEST_TOKEN.to_string(),
        };
        state.token_value = Some(TEST_TOKEN.to_string());
    }
    let (client, _directory) = make_client(&registry, RegistryOptions::default());
    crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect("the manifest resolves");
    let debug = format!("{client:?}");
    assert!(
        !debug.contains(TEST_TOKEN),
        "the client Debug hides the token: {debug}"
    );

    // An Unauthorized error names no secret.
    let registry = FakeRegistry::start();
    let (body, _digest) = manifest_body(&registry);
    registry.add_manifest("1", OCI_MANIFEST, body);
    {
        let mut state = registry.state();
        state.always_unauthorized = true;
    }
    let (client, _directory) = make_client(&registry, credentials(&registry));
    let error = crate::pool::block_on(client.resolve_manifest(&reference(&registry, "1")))
        .expect_err("the request is unauthorized");
    let display = format!("{error}");
    let debug = format!("{error:?}");
    for text in ["user:pass".to_string(), TEST_TOKEN.to_string()] {
        assert!(
            !display.contains(&text),
            "the error shows no secret: {display}"
        );
        assert!(!debug.contains(&text), "the debug shows no secret: {debug}");
    }
    assert!(
        matches!(error.current_context(), SandboxError::Unauthorized),
        "the error is Unauthorized: {error:?}"
    );
}
