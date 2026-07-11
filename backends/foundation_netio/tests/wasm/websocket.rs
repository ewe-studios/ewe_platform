use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_netio::wasm::client::FetchHttpClient;
use foundation_netio::websocket::shared::client::{WebSocketConnectConfig, WebSocketEvent, WsProgress};
use foundation_netio::websocket::shared::connector::WebSocketConnector;
use foundation_netio::websocket::shared::message::WebSocketMessage;
use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};
use wasm_bindgen_test::wasm_bindgen_test;

async fn yield_now() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        web_sys::window()
            .expect("window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
            .expect("set_timeout");
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[wasm_bindgen_test]
fn fetch_client_is_a_websocket_connector() {
    fn assert_connector<T: WebSocketConnector>(_: &T) {}
    assert_connector(&FetchHttpClient::new());
}

#[wasm_bindgen_test]
async fn open_websocket_refused_surfaces_close() {
    let client = FetchHttpClient::new();
    let mut ws = client
        .open_websocket("ws://127.0.0.1:47913", WebSocketConnectConfig::new())
        .expect("open_websocket");

    for _ in 0..2000 {
        match ws.messages().next() {
            Some(Ok(WebSocketEvent::Message(WebSocketMessage::Close(_, _))))
            | Some(Err(_))
            | None => return,
            Some(Ok(WebSocketEvent::Message(other))) => {
                panic!("unexpected live message before close: {other:?}")
            }
            Some(Ok(WebSocketEvent::Skip)) => yield_now().await,
        }
    }
    panic!("refused WebSocket never surfaced a close/error");
}

/// `open_websocket_task` returns a `Box<dyn TaskIterator>` — the same
/// cross-platform pattern as `HttpClient::open_exchange()`. Before the browser
/// delivers any WebSocket events, the first poll should yield
/// `TaskStatus::Pending(WsProgress::Connecting)`, proving the `Stream → TaskStatus`
/// adapter in `browser.rs` works.
#[wasm_bindgen_test]
fn open_websocket_task_yields_pending_connecting_on_wasm() {
    let client = FetchHttpClient::new();
    let (mut task, _delivery) = client
        .open_websocket_task("ws://127.0.0.1:9", WebSocketConnectConfig::new())
        .expect("open_websocket_task");

    let status = task.next_status();
    assert!(
        matches!(status, Some(TaskStatus::Pending(WsProgress::Connecting))),
        "expected Pending(Connecting) on first poll before browser events, got {status:?}"
    );
}
