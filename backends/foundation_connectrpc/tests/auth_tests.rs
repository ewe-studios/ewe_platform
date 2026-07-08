//! Auth module integration tests.
//!
//! WHY: Authentication and authorization need thorough coverage for the
//! middlewares, authenticators, and helpers.
//!
//! WHAT: Tests for AuthInfo construction, bearer_token extraction,
//! protocol/procedure inference, JwtAuthenticator, CompositeAuthenticator,
//! PerProcedureAuth, AuthzInterceptor, and authenticate_request.
//!
//! HOW: Feature-gated on `auth + multi`.

#![cfg(all(test, feature = "auth", feature = "multi"))]

use foundation_auth::AuthContext;
use foundation_connectrpc::{AuthInfo, AuthFunc, bearer_token, Interceptor};
use foundation_connectrpc::CompositeAuthenticator;
use foundation_connectrpc::PerProcedureAuth;
use foundation_netio::simple_http::shared::{SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod, SimpleUrl};

// ---------------------------------------------------------------------------
// Test 1: AuthInfo construction
// ---------------------------------------------------------------------------

/// WHY: AuthInfo is the fundamental auth result type. It must carry the auth
/// context and support artifact attachments.
/// WHAT: Creating AuthInfo from an AuthContext and adding a typed artifact.
#[test]
fn test_auth_info_construction() {
    let ctx = AuthContext::new("/test/path".to_string(), None, None);
    let info = AuthInfo::new(ctx.clone())
        .with_artifact(42u32);

    assert_eq!(info.context.path, "/test/path");
    assert!(info.context.token.is_none());
    assert!(info.context.sub.is_none());
    assert!(info.context.roles.is_empty());

    // Verify artifact is stored
    let artifact: Option<&u32> = info.artifacts.get::<u32>();
    assert_eq!(artifact, Some(&42));
}

// ---------------------------------------------------------------------------
// Test 2: bearer_token extraction (RFC 9110 case-insensitive)
// ---------------------------------------------------------------------------

/// WHY: bearer_token must handle all ASCII case variants per RFC 9110.
/// WHAT: Extracting tokens with "Bearer ", "bearer ", "BEARER " prefixes.
#[test]
fn test_bearer_token_rfc9110_case_insensitive() {
    // Standard "Bearer " prefix
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::AUTHORIZATION, "Bearer my-token-123")
        .build()
        .expect("valid request");
    assert_eq!(bearer_token(&req), Some("my-token-123".to_string()));

    // Lowercase "bearer " prefix
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::AUTHORIZATION, "bearer my-token-123")
        .build()
        .expect("valid request");
    assert_eq!(bearer_token(&req), Some("my-token-123".to_string()));

    // Uppercase "BEARER " prefix
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::AUTHORIZATION, "BEARER my-token-123")
        .build()
        .expect("valid request");
    assert_eq!(bearer_token(&req), Some("my-token-123".to_string()));

    // Mixed case "BeArEr " prefix
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::AUTHORIZATION, "BeArEr my-token-123")
        .build()
        .expect("valid request");
    assert_eq!(bearer_token(&req), Some("my-token-123".to_string()));
}

/// WHY: Requests without an Authorization header must return None.
/// WHAT: No auth header means no bearer token.
#[test]
fn test_bearer_token_no_header() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .build()
        .expect("valid request");
    assert!(bearer_token(&req).is_none());
}

// ---------------------------------------------------------------------------
// Test 3: infer_protocol
// ---------------------------------------------------------------------------

/// WHY: The auth layer needs to detect the protocol to render errors correctly.
/// WHAT: POST requests with recognized content types are detected correctly.
#[test]
fn test_infer_protocol_connect_unary() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .build()
        .expect("valid request");
    assert_eq!(
        foundation_connectrpc::infer_protocol(&req),
        Some("connect")
    );
}

#[test]
fn test_infer_protocol_connect_streaming() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/connect+proto")
        .build()
        .expect("valid request");
    assert_eq!(
        foundation_connectrpc::infer_protocol(&req),
        Some("connect")
    );
}

#[test]
fn test_infer_protocol_grpc_web() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/grpc-web+proto")
        .build()
        .expect("valid request");
    assert_eq!(
        foundation_connectrpc::infer_protocol(&req),
        Some("grpc-web")
    );
}

#[test]
fn test_infer_protocol_grpc() {
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/grpc+proto")
        .build()
        .expect("valid request");
    assert_eq!(
        foundation_connectrpc::infer_protocol(&req),
        Some("grpc")
    );
}

#[test]
fn test_infer_protocol_unknown() {
    // No Content-Type header
    let req = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/test")
        .with_method(SimpleMethod::POST)
        .build()
        .expect("valid request");
    assert!(foundation_connectrpc::infer_protocol(&req).is_none());
}

// ---------------------------------------------------------------------------
// Test 4: infer_procedure
// ---------------------------------------------------------------------------

/// WHY: Per-procedure auth needs to extract the procedure from the request URL.
/// WHAT: URLs with /service/method paths produce correct procedure strings.
#[test]
fn test_infer_procedure_standard_url() {
    let url = foundation_netio::simple_http::shared::SimpleUrl::url_only(
        "http://example.com/connectrpc.greet.v1.GreetService/Greet".to_string()
    );
    assert_eq!(
        foundation_connectrpc::infer_procedure(&url),
        Some("/connectrpc.greet.v1.GreetService/Greet".to_string())
    );
}

#[test]
fn test_infer_procedure_deep_path() {
    let url = foundation_netio::simple_http::shared::SimpleUrl::url_only(
        "http://example.com/v1/packages/pkg.Service/Method".to_string()
    );
    assert_eq!(
        foundation_connectrpc::infer_procedure(&url),
        Some("/pkg.Service/Method".to_string())
    );
}

#[test]
fn test_infer_procedure_root_path() {
    let url = foundation_netio::simple_http::shared::SimpleUrl::url_only(
        "http://example.com".to_string()
    );
    assert!(foundation_connectrpc::infer_procedure(&url).is_none());
}

#[test]
fn test_infer_procedure_single_segment() {
    let url = foundation_netio::simple_http::shared::SimpleUrl::url_only(
        "http://example.com/just-one".to_string()
    );
    assert!(foundation_connectrpc::infer_procedure(&url).is_none());
}

// ---------------------------------------------------------------------------
// Test 5: AuthFunc blanket impl (closures)
// ---------------------------------------------------------------------------

/// WHY: The blanket impl for async closures is the primary way AuthFunc is used.
/// WHAT: A closure returning Ok(Some(info)) works as AuthFunc.
#[test]
fn test_auth_func_closure_success() {
    let auth = |_headers: &SimpleHeaders, _url: &SimpleUrl| {
        let ctx = AuthContext::new("/proc".to_string(), None, None);
        let r: foundation_connectrpc::ConnectResult<Option<AuthInfo>> =
            Ok(Some(AuthInfo::new(ctx)));
        futures::future::ready(r)
    };

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com/proc".to_string());
    let result = futures::executor::block_on(auth.authenticate(&headers, &url));
    let info = result.expect("auth should succeed");
    assert!(info.is_some());
    assert_eq!(info.unwrap().context.path, "/proc");
}

/// WHY: A closure returning Ok(None) should pass through without error.
/// WHAT: No auth means no information, but not an error.
#[test]
fn test_auth_func_closure_none() {
    let auth = |_headers: &SimpleHeaders, _url: &SimpleUrl| {
        let r: foundation_connectrpc::ConnectResult<Option<AuthInfo>> = Ok(None);
        futures::future::ready(r)
    };

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com/proc".to_string());
    let result = futures::executor::block_on(auth.authenticate(&headers, &url));
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Test 6: CompositeAuthenticator
// ---------------------------------------------------------------------------

/// WHY: CompositeAuthenticator chains authenticators and returns the first
/// successful result.
/// WHAT: First authenticator succeeds, second is never reached.
#[test]
fn test_composite_authenticator_first_succeeds() {
    let auth1 = |_: &SimpleHeaders, _: &SimpleUrl| async move {
        let ctx = AuthContext::new("/first".to_string(), None, None);
        Ok(Some(AuthInfo::new(ctx)))
    };
    let auth2 = |_: &SimpleHeaders, _: &SimpleUrl| async move {
        panic!("should not be called");
    };

    let composite = CompositeAuthenticator::new()
        .with(auth1)
        .with(auth2);

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com".to_string());
    let result = futures::executor::block_on(composite.authenticate(&headers, &url));
    let info = result.expect("auth should succeed");
    assert_eq!(info.unwrap().context.path, "/first");
}

/// WHY: If the first returns None, the second should be tried.
/// WHAT: Authenticators fall through in order.
#[test]
fn test_composite_authenticator_fallthrough() {
    let auth1 = |_: &SimpleHeaders, _: &SimpleUrl| async move { Ok(None) };
    let auth2 = |_: &SimpleHeaders, _: &SimpleUrl| async move {
        let ctx = AuthContext::new("/second".to_string(), None, None);
        Ok(Some(AuthInfo::new(ctx)))
    };

    let composite = CompositeAuthenticator::new()
        .with(auth1)
        .with(auth2);

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com".to_string());
    let result = futures::executor::block_on(composite.authenticate(&headers, &url));
    let info = result.expect("auth should succeed");
    assert_eq!(info.unwrap().context.path, "/second");
}

// ---------------------------------------------------------------------------
// Test 7: PerProcedureAuth
// ---------------------------------------------------------------------------

/// WHY: PerProcedureAuth dispatches based on the procedure path.
/// WHAT: A configured procedure uses its own authenticator.
#[test]
fn test_per_procedure_auth_matched_procedure() {
    let proc_auth = |_: &SimpleHeaders, _: &SimpleUrl| async move {
        let ctx = AuthContext::new("/custom/proc".to_string(), None, None);
        Ok(Some(AuthInfo::new(ctx)))
    };

    let per_proc = PerProcedureAuth::new()
        .with_procedure("/svc.Svc/Call", proc_auth);

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com/svc.Svc/Call".to_string());
    let result = futures::executor::block_on(per_proc.authenticate(&headers, &url));
    let info = result.expect("auth should succeed");
    assert!(info.is_some());
}

/// WHY: An unauthenticated procedure bypasses all authenticators.
/// WHAT: Marking a procedure as unauthenticated returns Ok(None).
#[test]
fn test_per_procedure_auth_unauthenticated() {
    // auth is defined but not used because the procedure is unauthenticated
    let _auth = |_: &SimpleHeaders, _: &SimpleUrl| async move {
        panic!("should not be called");
    };

    let per_proc = PerProcedureAuth::new()
        .without_auth("/pub.Public/Info");

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com/pub.Public/Info".to_string());
    let result = futures::executor::block_on(per_proc.authenticate(&headers, &url));
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

/// WHY: A procedure with no configured auth and no default returns None.
/// WHAT: Unknown procedures return Ok(None).
#[test]
fn test_per_procedure_auth_unknown_procedure() {
    let per_proc = PerProcedureAuth::new();

    let headers = SimpleHeaders::new();
    let url = SimpleUrl::url_only("http://example.com/unknown.Svc/Method".to_string());
    let result = futures::executor::block_on(per_proc.authenticate(&headers, &url));
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Test 8: JwtAuthenticator
// ---------------------------------------------------------------------------

/// WHY: JwtAuthenticator must validate bearer tokens against a JWT verifier.
/// WHAT: A valid signed JWT returns AuthInfo with the correct sub claim.
#[test]
fn test_jwt_authenticator_valid_token() {
    use foundation_auth::{
        JwtAlgorithm, JwtSigningKey, JwtVerifierConfig, PublicKeySource,
    };

    // Generate an Ed25519 key pair
    let key_pair = JwtSigningKey::generate_ed25519();
    let public_key_pem = key_pair.public_key_pem().expect("valid PEM");

    // Build verifier config from the public key
    let config = JwtVerifierConfig {
        allowed_algorithms: vec![JwtAlgorithm::EdDSA],
        issuer: Some("test-issuer".to_string()),
        audience: Some("test-audience".to_string()),
        key_id: None,
        public_key: PublicKeySource::Pem(public_key_pem),
    };

    let authenticator = foundation_connectrpc::JwtAuthenticator::new(config)
        .expect("valid authenticator");

    // Create signed claims
    let claims = serde_json::json!({
        "sub": "user-123",
        "iss": "test-issuer",
        "aud": "test-audience",
    });
    let token = key_pair.sign_claims(&claims).expect("valid signature");

    // Build request with bearer token
    let headers = {
        let mut h = SimpleHeaders::new();
        h.insert(
            SimpleHeader::AUTHORIZATION,
            vec![format!("Bearer {token}")],
        );
        h
    };
    let url = SimpleUrl::url_only("http://example.com/test".to_string());

    let result = futures::executor::block_on(
        authenticator.authenticate(&headers, &url),
    );
    let info = result.expect("auth should succeed");
    let info = info.expect("should return Some(AuthInfo)");
    assert_eq!(info.context.sub.as_deref(), Some("user-123"));
    assert!(info.context.token.is_some());

    // Verify the VerifiedClaims artifact is present
    let claims: Option<&foundation_auth::VerifiedClaims> =
        info.artifacts.get::<foundation_auth::VerifiedClaims>();
    assert!(claims.is_some());
    assert_eq!(claims.unwrap().sub, "user-123");
}

/// WHY: An invalid/malformed token should return an Unauthenticated error.
/// WHAT: A garbage token fails verification.
#[test]
fn test_jwt_authenticator_invalid_token() {
    use foundation_auth::{
        JwtAlgorithm, JwtSigningKey, JwtVerifierConfig, PublicKeySource,
    };

    let key_pair = JwtSigningKey::generate_ed25519();
    let public_key_pem = key_pair.public_key_pem().expect("valid PEM");

    let config = JwtVerifierConfig {
        allowed_algorithms: vec![JwtAlgorithm::EdDSA],
        issuer: None,
        audience: None,
        key_id: None,
        public_key: PublicKeySource::Pem(public_key_pem),
    };

    let authenticator = foundation_connectrpc::JwtAuthenticator::new(config)
        .expect("valid authenticator");

    // Garbage token
    let headers = {
        let mut h = SimpleHeaders::new();
        h.insert(
            SimpleHeader::AUTHORIZATION,
            vec!["Bearer garbage-token-that-is-not-a-jwt".to_string()],
        );
        h
    };
    let url = SimpleUrl::url_only("http://example.com/test".to_string());

    let result = futures::executor::block_on(
        authenticator.authenticate(&headers, &url),
    );
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Test 9: AuthzInterceptor
// ---------------------------------------------------------------------------

/// WHY: AuthzInterceptor must check scopes in AuthInfo from the Ctx.
/// WHAT: The interceptor checks scopes before allowing the RPC through.
#[test]
fn test_authz_interceptor_grants_access() {
    use std::sync::Arc;
    use foundation_connectrpc::context::Ctx;
    use foundation_netio::simple_http::shared::Extensions;

    let interceptor = foundation_connectrpc::AuthzInterceptor::new(
        vec!["read".to_string()],
    );

    // Build a Ctx with AuthInfo containing a token with "read" scope
    let mut ext = Extensions::new();
    let mut auth_ctx = AuthContext::new("/test".to_string(), None, None);
    auth_ctx.sub = Some("user-1".to_string());
    auth_ctx.token = Some(foundation_auth::AuthToken::OAuth {
        access_token: foundation_auth::ConfidentialText::new("tok".to_string()),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        expires_at: f64::MAX,
        scope: Some("read write".to_string()),
    });
    ext.insert(AuthInfo::new(auth_ctx));

    let mut ctx = Ctx::background();
    ctx.request.extensions = ext;

    let next_func: foundation_connectrpc::UnaryFunc = Arc::new(|_ctx, _call| {
        Box::pin(async {
            Ok(foundation_connectrpc::UnaryReply {
                headers: foundation_netio::simple_http::shared::SimpleHeaders::new(),
                trailers: foundation_netio::simple_http::shared::SimpleHeaders::new(),
                frame: bytes::Bytes::new(),
            })
        })
    });

    let wrapped = interceptor.wrap_unary(next_func);
    let call = foundation_connectrpc::UnaryCall {
        headers: foundation_netio::simple_http::shared::SimpleHeaders::new(),
        codec_name: "proto".to_string(),
        frame: bytes::Bytes::new(),
    };

    let result = futures::executor::block_on(wrapped(ctx, call));
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.is_err());
}

/// WHY: Missing required scopes should deny access.
/// WHAT: The interceptor returns PermissionDenied when scopes are missing.
#[test]
fn test_authz_interceptor_denies_insufficient_scope() {
    use std::sync::Arc;
    use foundation_connectrpc::context::Ctx;
    use foundation_netio::simple_http::shared::Extensions;

    let interceptor = foundation_connectrpc::AuthzInterceptor::new(
        vec!["admin".to_string()],
    );

    let mut ext = Extensions::new();
    let mut auth_ctx = AuthContext::new("/test".to_string(), None, None);
    auth_ctx.sub = Some("user-1".to_string());
    auth_ctx.token = Some(foundation_auth::AuthToken::OAuth {
        access_token: foundation_auth::ConfidentialText::new("tok".to_string()),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        expires_at: f64::MAX,
        scope: Some("read write".to_string()),
    });
    ext.insert(AuthInfo::new(auth_ctx));

    let mut ctx = Ctx::background();
    ctx.request.extensions = ext;

    let next_func: foundation_connectrpc::UnaryFunc = Arc::new(|_ctx, _call| {
        Box::pin(async {
            panic!("should not be called");
        })
    });

    let wrapped = interceptor.wrap_unary(next_func);
    let call = foundation_connectrpc::UnaryCall {
        headers: foundation_netio::simple_http::shared::SimpleHeaders::new(),
        codec_name: "proto".to_string(),
        frame: bytes::Bytes::new(),
    };

    let result = futures::executor::block_on(wrapped(ctx, call));
    let err = match result {
        Err(err) => err,
        Ok(_) => panic!("expected Err"),
    };
    assert_eq!(
        err.current_context().code(),
        foundation_connectrpc::Code::PermissionDenied
    );
}

// ---------------------------------------------------------------------------
// Test 10: authenticate_request
// ---------------------------------------------------------------------------

/// WHY: authenticate_request must call AuthFunc, insert AuthInfo on success,
/// and render errors via ErrorWriter on failure.
/// WHAT: Successful authentication inserts AuthInfo into request extensions.
#[test]
fn test_authenticate_request_success() {
    let auth = |_: &SimpleHeaders, _url: &SimpleUrl| {
        let ctx = AuthContext::new("/proc".to_string(), None, None);
        let r: foundation_connectrpc::ConnectResult<Option<AuthInfo>> =
            Ok(Some(AuthInfo::new(ctx)));
        futures::future::ready(r)
    };

    let mut request = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/proc")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .build()
        .expect("valid request");

    let error_writer = foundation_connectrpc::ErrorWriter::new();

    let result = futures::executor::block_on(
        foundation_connectrpc::authenticate_request(&auth, &mut request, &error_writer),
    );
    let info = match result {
        Ok(info) => info,
        Err(_) => panic!("expected Ok"),
    };
    assert!(info.is_some());

    // Verify AuthInfo is in request extensions
    let exts = request.extensions.as_ref().expect("extensions should be Some");
    let stored: Option<&AuthInfo> = exts.get::<AuthInfo>();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().context.path, "/proc");
}

/// WHY: Authentication failure renders an error response with the correct status.
/// WHAT: Failed auth returns an error response with Unauthorized status.
#[test]
fn test_authenticate_request_failure() {
    use foundation_errstacks::ErrorTrace;

    let auth = |_: &SimpleHeaders, _url: &SimpleUrl| {
        let r: foundation_connectrpc::ConnectResult<Option<AuthInfo>> = Err(
            ErrorTrace::from(foundation_connectrpc::ConnectError::new(
                foundation_connectrpc::Code::Unauthenticated,
                "bad token",
            ))
        );
        futures::future::ready(r)
    };

    let mut request = SimpleIncomingRequest::builder()
        .with_plain_url("http://example.com/proc")
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .build()
        .expect("valid request");

    let error_writer = foundation_connectrpc::ErrorWriter::new();

    let result = futures::executor::block_on(
        foundation_connectrpc::authenticate_request(&auth, &mut request, &error_writer),
    );
    let resp = match result {
        Err(resp) => resp,
        Ok(_) => panic!("expected Err"),
    };
    assert_eq!(
        resp.status,
        foundation_netio::simple_http::shared::Status::Unauthorized
    );
}
