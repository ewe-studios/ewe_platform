//! Wasm HTTP exchange task construction smoke test (moved from
//! `src/wasm/client/tasks/http_exchange_task.rs`).

use foundation_core::url::Uri;
use foundation_core::valtron::{Ctx, TaskIterator};
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::{Extensions, SendSafeBody, SimpleHeaders, SimpleMethod};
use foundation_netio::wasm::client::tasks::new_http_exchange_task;
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
    let mut task = new_http_exchange_task(request);
    let _status = task.next_status(Ctx::new());
}
