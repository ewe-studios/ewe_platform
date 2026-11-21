//! Test function and struct registration through the same native router.

#![cfg(not(target_family = "wasm"))]

mod fake_overlay;

use std::sync::Arc;

use foundation_http::{
    shared::{app::HttpApp, router::Router},
    ConnectionResult, Ctx, RawStream, Route, Serve, ServeOutcome, SharedByteBufferStream,
    SimpleIncomingRequest, SimpleMethod,
};

type Connection = SharedByteBufferStream<RawStream>;

#[foundation_http::get("/catalog")]
fn catalog(_ctx: Ctx, req: SimpleIncomingRequest, _conn: Connection) -> ConnectionResult {
    assert_eq!(req.method, SimpleMethod::GET);
    ConnectionResult::Keep
}

#[foundation_http::post("/catalog/new")]
async fn create(_ctx: Ctx, _req: SimpleIncomingRequest, _conn: Connection) -> ServeOutcome {
    ServeOutcome::Take
}

struct Alias;
impl Route for Alias {
    const METHOD: SimpleMethod = SimpleMethod::POST;
    const PATH: &'static str = "/action";
    const ALIASES: &'static [&'static str] = &["/action/*"];
}
impl Serve for Alias {
    fn serve(&self, _: Ctx, _: SimpleIncomingRequest, _: Connection) -> ConnectionResult {
        ConnectionResult::Keep
    }
}

#[test]
fn mount_reuses_one_handler_across_aliases() {
    let mut app = HttpApp::new_serve();
    app.mount(Alias).unwrap();
    let exact = app.router.dispatch(&SimpleMethod::POST, "/action").unwrap();
    let named = app
        .router
        .dispatch(&SimpleMethod::POST, "/action/add")
        .unwrap();
    assert!(Arc::ptr_eq(&exact, &named));
    assert!(app
        .router
        .dispatch(&SimpleMethod::GET, "/action/add")
        .is_none());
}

#[test]
fn function_attributes_forward_requests_and_preserve_method_boundaries() {
    let _pool = foundation_core::valtron::initialize_pool(0, Some(2));
    let mut router = Router::<Arc<dyn Serve>>::new();
    router.mount(catalog).unwrap();
    router.mount(create).unwrap();
    assert!(router.dispatch(&SimpleMethod::POST, "/catalog").is_none());

    for (method, path, asynchronous) in [
        (SimpleMethod::GET, "/catalog", false),
        (SimpleMethod::POST, "/catalog/new", true),
    ] {
        let req = SimpleIncomingRequest::builder()
            .with_method(method.clone())
            .with_plain_url(path)
            .build()
            .unwrap();
        let (conn, _client) = fake_overlay::overlay_pair(256);
        let handler = router.dispatch(&method, path).unwrap();
        let result = handler.serve(Ctx::host(), req, conn);
        if asynchronous {
            let ConnectionResult::Async(future) = result else {
                panic!("Expected an async response")
            };
            assert!(matches!(
                foundation_core::valtron::block_on_future(future),
                ServeOutcome::Take
            ));
        } else {
            assert!(matches!(result, ConnectionResult::Keep));
        }
    }
}

#[test]
fn method_wrappers_accept_existing_synchronous_and_asynchronous_handlers() {
    let mut router = Router::<Arc<dyn Serve>>::new();
    router.at("/file", foundation_http::get(Alias)).unwrap();
    router
        .at(
            "/file",
            foundation_http::put(|_: Ctx, _: SimpleIncomingRequest, _: Connection| {
                ConnectionResult::Keep
            }),
        )
        .unwrap();
    router
        .at(
            "/file",
            foundation_http::delete(|_: Ctx, _: SimpleIncomingRequest, _: Connection| async {
                ServeOutcome::Take
            }),
        )
        .unwrap();
    for method in [SimpleMethod::GET, SimpleMethod::PUT, SimpleMethod::DELETE] {
        assert!(router.dispatch(&method, "/file").is_some());
    }
    assert!(router.dispatch(&SimpleMethod::HEAD, "/file").is_none());
}

#[test]
fn invalid_alias_leaves_the_router_unchanged() {
    struct Invalid;
    impl Route for Invalid {
        const METHOD: SimpleMethod = SimpleMethod::GET;
        const PATH: &'static str = "/valid";
        const ALIASES: &'static [&'static str] = &["/earlier", "/users/:id::unknown"];
    }
    impl Serve for Invalid {
        fn serve(&self, _: Ctx, _: SimpleIncomingRequest, _: Connection) -> ConnectionResult {
            ConnectionResult::Take
        }
    }
    let mut router = Router::<Arc<dyn Serve>>::new();
    let mut app = HttpApp::new_serve();
    assert_eq!(
        router.mount(Invalid).unwrap_err().to_string(),
        app.mount(Invalid).unwrap_err().to_string()
    );
    for path in ["/valid", "/earlier"] {
        assert!(app.router.dispatch(&SimpleMethod::GET, path).is_none());
    }
    assert!(router.dispatch(&SimpleMethod::GET, "/valid").is_none());
    assert!(router.dispatch(&SimpleMethod::GET, "/earlier").is_none());
    router
        .at("/users/:id::numbers", foundation_http::get(Alias))
        .unwrap();
    assert!(router.dispatch(&SimpleMethod::GET, "/users/42").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/users/bad").is_none());
}
