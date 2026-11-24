use foundation_netio::wasm::client::FetchHttpClient;
use foundation_netio::websocket::shared::client::{WebSocketConnectConfig, WebSocketEvent};
use foundation_netio::websocket::shared::connector::WebSocketConnector;
use foundation_netio::websocket::shared::message::WebSocketMessage;
use foundation_testbed::bindgen::wasm_bindgen::{JsCast, JsValue};
use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures};
use wasm_bindgen_test::wasm_bindgen_test;

async fn yield_now() {
    // Deno has no `window`; call the global `setTimeout` instead. A microtask
    // yield is not enough for the fetch/WebSocket callbacks.
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("setTimeout")
            .dyn_into::<js_sys::Function>()
            .expect("setTimeout is a function");
        set_timeout
            .call2(&global, &resolve, &JsValue::from_f64(0.0))
            .expect("setTimeout");
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

/// `open_websocket_task` returns a task whose first poll reports a status.
///
/// This drives the task directly instead of running it on the valtron pool.
/// Driving `valtron::execute` from a test body hangs under the Deno runner
/// (#565); a direct `next_status` proves the same contract without the pool.
#[wasm_bindgen_test]
async fn open_websocket_task_yields_pending_connecting_on_wasm() {
    use foundation_context::Ctx;
    use foundation_core::valtron::{TaskIterator, TaskStatus};

    let client = FetchHttpClient::new();
    let (mut task, _delivery) = client
        .open_websocket_task("ws://127.0.0.1:9", WebSocketConnectConfig::new())
        .expect("open_websocket_task");

    assert!(
        task.next_status(Ctx::host()).is_some(),
        "first poll should yield a status"
    );

    // Drain the refused dial, so its browser socket closes before the event
    // closures drop. Deno aborts the whole run when an event calls a dropped
    // closure, and the next test then reports no summary.
    for _ in 0..200 {
        match task.next_status(Ctx::host()) {
            None | Some(TaskStatus::Ready(_)) => return,
            Some(_) => yield_now().await,
        }
    }
    panic!("the refused WebSocket never finished");
}
