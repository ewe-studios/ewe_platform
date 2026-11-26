//! Host integration of authenticated sessions and signed component state.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use foundation_auth::{
    CredentialStorage, CredentialStore, SessionConfig, SessionError, SessionManager,
};
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, with_state_binding, HmacStateSigner,
    StateConfig, StateError,
};

use crate::support::{test_ctx, WALL};

#[derive(Debug)]
enum RouteError {
    LoginRequired,
    Session(SessionError),
    State(StateError),
}

struct CurrentUser(String);

struct TodoHost {
    base: Ctx,
    sessions: SessionManager<CredentialStorage>,
    reads: AtomicU32,
    renders: AtomicU32,
    actions: AtomicU32,
}

impl TodoHost {
    fn new() -> Self {
        let (base, _) = test_ctx(WALL);
        let sessions = SessionManager::new(
            CredentialStorage::memory(),
            SessionConfig::default(),
            &[7; 32],
            base.clone(),
        )
        .expect("install session manager");
        let signer = HmacStateSigner::new([9; 32], StateConfig::default()).unwrap();
        install_state_signer(&base, Arc::new(signer));
        Self {
            base,
            sessions,
            reads: AtomicU32::new(0),
            renders: AtomicU32::new(0),
            actions: AtomicU32::new(0),
        }
    }

    // The HTTP adapter supplies the configured session cookie value, if present.
    async fn request_context(&self, cookie: Option<&str>) -> Result<Ctx, RouteError> {
        let token = cookie.ok_or(RouteError::LoginRequired)?;
        let session = self
            .sessions
            .get_session(token)
            .await
            .map_err(RouteError::Session)?
            .ok_or(RouteError::LoginRequired)?;
        let ctx = with_state_binding(&self.base, session.user_id.as_bytes())
            .map_err(RouteError::State)?;
        ctx.store(CurrentUser(session.user_id));
        Ok(ctx)
    }

    async fn page(&self, cookie: Option<&str>) -> Result<Vec<u8>, RouteError> {
        let ctx = self.request_context(cookie).await?;
        let user = ctx.get::<CurrentUser>().expect("authenticated user");
        self.reads.fetch_add(1, Ordering::Relaxed);
        let todo = format!("{}'s todo", user.0);
        self.renders.fetch_add(1, Ordering::Relaxed);
        sign_state(&ctx, todo.as_bytes()).map_err(RouteError::State)
    }

    async fn submit(&self, cookie: Option<&str>, state: &[u8]) -> Result<Vec<u8>, RouteError> {
        let ctx = self.request_context(cookie).await?;
        let todo = verify_state(&ctx, state).map_err(RouteError::State)?;
        self.actions.fetch_add(1, Ordering::Relaxed);
        Ok(todo)
    }

    async fn login(&self, user: &str) -> (String, String) {
        let (session, cookies) = self
            .sessions
            .create_session(user, None, None)
            .await
            .unwrap();
        let token = cookies
            .into_iter()
            .find(|cookie| cookie.name == self.sessions.config().token_cookie_name)
            .expect("issued session cookie")
            .value;
        (session.id, token)
    }
}

#[valtron_test]
async fn protected_routes_bind_each_request_to_the_verified_session_user() {
    let host = TodoHost::new();
    let (_, alice) = host.login("Alice").await;
    let (_, bob) = host.login("Bob").await;
    let signed = host.page(Some(&alice)).await.unwrap();
    assert_eq!(
        host.submit(Some(&alice), &signed).await.unwrap(),
        b"Alice's todo"
    );
    assert!(matches!(
        host.submit(Some(&bob), &signed).await,
        Err(RouteError::State(StateError::BadTag))
    ));
    assert_eq!(host.actions.load(Ordering::Relaxed), 1);
}

#[valtron_test]
async fn failed_session_checks_stop_page_reads_rendering_and_actions() {
    let host = TodoHost::new();
    let (session_id, token) = host.login("Alice").await;
    host.sessions.revoke_session(&session_id).await.unwrap();

    for cookie in [None, Some(token.as_str())] {
        assert!(matches!(
            host.page(cookie).await,
            Err(RouteError::LoginRequired)
        ));
        assert!(matches!(
            host.submit(cookie, b"unused").await,
            Err(RouteError::LoginRequired)
        ));
    }

    // A malformed stored row makes the production session lookup return a storage error.
    host.sessions
        .store()
        .set(&session_id, "invalid session row")
        .await
        .unwrap();
    assert!(matches!(
        host.page(Some(&token)).await,
        Err(RouteError::Session(SessionError::Storage(_)))
    ));
    assert!(matches!(
        host.submit(Some(&token), b"unused").await,
        Err(RouteError::Session(SessionError::Storage(_)))
    ));
    assert_eq!(
        (
            host.reads.load(Ordering::Relaxed),
            host.renders.load(Ordering::Relaxed),
            host.actions.load(Ordering::Relaxed)
        ),
        (0, 0, 0)
    );
}
