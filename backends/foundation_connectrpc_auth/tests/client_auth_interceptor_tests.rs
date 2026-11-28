//! Spec 81 feature 00, task T11: the `ConnectRPC` `ClientAuthInterceptor`.
//!
//! `cargo test -p foundation_connectrpc_auth --test client_auth_interceptor_tests`
//! runs them.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use bytes::Bytes;
use foundation_auth::client_auth::{
    AuthError, AuthFuture, AuthInfo, AuthMethod, CallCredentials, ClientAuth, Principal,
};
use foundation_auth::AuthContext;
use foundation_connectrpc_auth::{get_auth_info, ClientAuthInterceptor};
use foundation_connectrpc::shared::context::{
    IdempotencyLevel, RequestContext, RpcCtx, Spec, StreamType,
};
use foundation_connectrpc::shared::error::Code;
use foundation_connectrpc::shared::interceptor::{Interceptor, UnaryCall, UnaryFunc, UnaryReply};
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};

fn block_on<F: Future>(fut: F) -> F::Output {
    struct W(std::thread::Thread);
    impl Wake for W {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(W(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn rpc_ctx(procedure: &str, headers: SimpleHeaders) -> RpcCtx {
    let ctx = foundation_context::Ctx::host();
    let mut request = RequestContext::empty(false, ctx.clone());
    request.spec = Arc::new(Spec {
        stream_type: StreamType::Unary,
        procedure: procedure.to_string(),
        is_client: false,
        idempotency: IdempotencyLevel::Unknown,
    });
    // The interceptor reads the request headers from the call context.
    request.headers = Arc::new(headers);
    RpcCtx { ctx, request }
}

fn unary_call(headers: SimpleHeaders) -> UnaryCall {
    UnaryCall {
        headers,
        codec_name: "proto".to_string(),
        frame: Bytes::from_static(b"x"),
    }
}

/// A provider that accepts a caller that shows an `Authorization` header.
struct HeaderAuth;

impl ClientAuth for HeaderAuth {
    fn mode(&self) -> &'static str {
        "header"
    }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(subject) = call
                .headers
                .get(&SimpleHeader::AUTHORIZATION)
                .and_then(|values| values.first())
            else {
                return Ok(None);
            };
            let mut context = AuthContext::new(call.procedure.to_string(), None, None);
            context.sub = Some(subject.clone());
            let principal = Principal {
                subject: subject.clone(),
                method: AuthMethod::Insecure,
                roles: Vec::new(),
            };
            Ok(Some(AuthInfo::new(context).with_artifact(principal)))
        })
    }
}

/// A provider that refuses every call.
struct RejectAuth;

impl ClientAuth for RejectAuth {
    fn mode(&self) -> &'static str {
        "reject"
    }

    fn authenticate<'a>(&'a self, _call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move { Err(AuthError::Expired) })
    }
}

/// A handler that records that it ran, and the `AuthInfo` it saw.
fn recording_handler(ran: Arc<AtomicBool>, seen: Arc<Mutex<Option<AuthInfo>>>) -> UnaryFunc {
    Arc::new(move |ctx: RpcCtx, call: UnaryCall| {
        let ran = Arc::clone(&ran);
        let seen = Arc::clone(&seen);
        Box::pin(async move {
            ran.store(true, Ordering::SeqCst);
            *seen.lock().expect("lock") = get_auth_info(&ctx).cloned();
            Ok(UnaryReply {
                headers: SimpleHeaders::new(),
                trailers: SimpleHeaders::new(),
                frame: call.frame,
            })
        })
    })
}

fn authorization(value: &str) -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::AUTHORIZATION, vec![value.to_string()]);
    headers
}

#[test]
fn auth_info_reaches_the_handler() {
    let ran = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(None));
    let interceptor = ClientAuthInterceptor::new(Arc::new(HeaderAuth));
    let composed = interceptor.wrap_unary(recording_handler(Arc::clone(&ran), Arc::clone(&seen)));

    assert!(
        block_on(composed(
            rpc_ctx("/ewe.test.v1.Service/Call", authorization("alice")),
            unary_call(SimpleHeaders::new()),
        ))
        .is_ok(),
        "the call runs"
    );

    assert!(ran.load(Ordering::SeqCst));
    let info = seen.lock().expect("lock").clone().expect("AuthInfo");
    assert_eq!(info.context.sub.as_deref(), Some("alice"));
}

#[test]
fn a_failure_never_runs_the_handler() {
    let ran = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(None));
    let interceptor = ClientAuthInterceptor::new(Arc::new(RejectAuth));
    let composed = interceptor.wrap_unary(recording_handler(Arc::clone(&ran), Arc::clone(&seen)));

    let error = block_on(composed(
        rpc_ctx("/ewe.test.v1.Service/Call", SimpleHeaders::new()),
        unary_call(SimpleHeaders::new()),
    ))
    .err()
    .expect("the call is refused");

    assert_eq!(error.current_context().code(), Code::Unauthenticated);
    assert!(!ran.load(Ordering::SeqCst));
}

#[test]
fn missing_credentials_are_unauthenticated() {
    let ran = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(None));
    let interceptor = ClientAuthInterceptor::new(Arc::new(HeaderAuth));
    let composed = interceptor.wrap_unary(recording_handler(Arc::clone(&ran), Arc::clone(&seen)));

    let error = block_on(composed(
        rpc_ctx("/ewe.test.v1.Service/Call", SimpleHeaders::new()),
        unary_call(SimpleHeaders::new()),
    ))
    .err()
    .expect("no credentials");

    assert_eq!(error.current_context().code(), Code::Unauthenticated);
    assert!(!ran.load(Ordering::SeqCst));
}

#[test]
fn a_public_procedure_skips_auth() {
    let ran = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(None));
    let interceptor = ClientAuthInterceptor::new(Arc::new(RejectAuth))
        .allow_unauthenticated("/ewe.test.v1.Service/Enroll");
    let composed = interceptor.wrap_unary(recording_handler(Arc::clone(&ran), Arc::clone(&seen)));

    assert!(
        block_on(composed(
            rpc_ctx("/ewe.test.v1.Service/Enroll", SimpleHeaders::new()),
            unary_call(SimpleHeaders::new()),
        ))
        .is_ok(),
        "the public call runs"
    );

    assert!(ran.load(Ordering::SeqCst));
    assert!(seen.lock().expect("lock").is_none());
}
