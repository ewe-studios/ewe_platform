//! Wasm HTTP exchange task construction smoke test (moved from
//! `src/wasm/client/tasks/http_exchange_task.rs`).

use std::sync::Arc;

use foundation_context::Ctx;
use foundation_core::url::Uri;
use foundation_core::valtron::TaskIterator;
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::{Extensions, SendSafeBody, SimpleHeaders, SimpleMethod};
use foundation_netio::wasm::client::tasks::new_http_exchange_task;
use foundation_netio::wasm::client::FetchHttpClient;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn wasm_http_exchange_task_constructs() {
    let request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com/").expect("uri"),
        headers: SimpleHeaders::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    };
    let client: Arc<dyn foundation_netio::shared::client::HttpClient> =
        Arc::new(FetchHttpClient::new());
    let mut task = new_http_exchange_task(client, request);
    let _status = task.next_status(Ctx::host());
}
