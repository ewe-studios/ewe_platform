#![cfg(target_arch = "wasm32")]

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

mod body_stream;
mod exchange_client;
mod http_exchange_task;
mod sse;
mod websocket;
