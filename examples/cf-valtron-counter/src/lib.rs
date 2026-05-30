#![allow(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

//! Minimal Cloudflare Workers example exercising the valtron executor
//! in wasm-bindgen. Two endpoints:
//! - GET / — serves a static HTML page that fetches /counter
//! - GET /counter — streams valtron counter ticks via ReadableStream

use foundation_core::valtron::{
    Stream, StreamIteratorExt, TaskIterator, TaskStatus, NoAction, execute, initialize_pool, PoolGuard,
};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

static POOL_GUARD: OnceLock<PoolGuard> = OnceLock::new();

use futures::StreamExt;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys;

// ===========================================================================
// CounterTaskIterator
// ===========================================================================

struct CounterTaskIterator {
    current: u64,
    max: u64,
    emit_wait: bool,
}

impl CounterTaskIterator {
    fn new(max: u64) -> Self {
        Self {
            current: 0,
            max,
            emit_wait: false,
        }
    }
}

impl TaskIterator for CounterTaskIterator {
    type Ready = u64;
    type Pending = u64;
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current > self.max {
            web_sys::console::log_1(&"CounterTaskIterator: returning None (task complete)".into());
            return None;
        }

        if self.current == self.max {
            self.current += 1;
            return Some(TaskStatus::Ready(self.current));
        }

        if self.emit_wait {
            self.emit_wait = false;
            Some(TaskStatus::Wait)
        } else {
            let tick = self.current;
            self.current += 1;
            self.emit_wait = true;
            web_sys::console::log_1(&format!("valtron counter tick: {tick}").into());
            Some(TaskStatus::Pending(tick))
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

fn html_response(body: &str, status: u16) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);
    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/html; charset=utf-8").unwrap();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

// ===========================================================================
// HTML Page (GET /)
// ===========================================================================

const COUNTER_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Valtron Counter Demo</title></head>
<body>
<h1>Valtron Counter Demo</h1>
<div id="output">Starting...</div>
<script>
(async () => {
  const res = await fetch('/counter');
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  const output = document.getElementById('output');
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    const text = decoder.decode(value);
    const lines = text.split('\n');
    for (const line of lines) {
      if (line.startsWith('data: ')) {
        const msg = line.slice(6);
        output.textContent += msg + '\n';
      }
    }
  }
})();
</script>
</body>
</html>"#;

fn handle_home() -> web_sys::Response {
    html_response(COUNTER_HTML, 200)
}

// ===========================================================================
// Counter Stream (GET /counter)
// ===========================================================================

async fn handle_counter() -> web_sys::Response {
    let driven_iter = execute(
        CounterTaskIterator::new(10),
        Some(std::time::Duration::from_millis(4)),
    ).expect("should schedule task");

    let future_stream = driven_iter.into_future_stream();

    let future_stream = Rc::new(RefCell::new(Some(future_stream)));

    let start_callback = Closure::wrap(Box::new(move |ctrl: JsValue| {
        let controller = ctrl.unchecked_into::<web_sys::ReadableStreamDefaultController>();
        // Take the stream out of the Rc<RefCell> — only runs once
        let stream = future_stream.borrow_mut().take().unwrap();
        let promise = future_to_promise(async move {
            let mut stream = stream;
            while let Some(item) = stream.next().await {
                match item {
                    Stream::Pending(count) => {
                        let chunk = format!("data: {count}\n\n");
                        let bytes = Uint8Array::from(chunk.as_bytes());
                        let _ = controller.enqueue_with_chunk(&bytes);
                    }
                    Stream::Wait => {
                        // executor yielded to JS event loop via JSThreadYielder
                        // just continue polling via .await
                    }
                    Stream::Next(final_val) => {
                        let chunk = format!("data: done: {final_val}\n\n");
                        let bytes = Uint8Array::from(chunk.as_bytes());
                        let _ = controller.enqueue_with_chunk(&bytes);
                        let _ = controller.close();
                        return Ok(JsValue::NULL);
                    }
                    Stream::Ignore | Stream::Init | Stream::Delayed(_) => {}
                    Stream::Spread(_) => {}
                }
            }
            web_sys::console::log_1(&"handle_counter: stream exhausted, closing response".into());
            let _ = controller.close();
            Ok(JsValue::NULL)
        });
        JsValue::from(promise)
    }) as Box<dyn FnMut(JsValue) -> JsValue>);

    let underlying_source = web_sys::UnderlyingSource::new();
    underlying_source.set_start(start_callback.as_ref().unchecked_ref::<js_sys::Function>());
    start_callback.forget();

    let rs = web_sys::ReadableStream::new_with_underlying_source(&underlying_source).unwrap();

    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "text/event-stream").unwrap();
    let init = web_sys::ResponseInit::new();
    init.set_headers(&headers);
    web_sys::Response::new_with_opt_readable_stream_and_init(Some(&rs), &init).unwrap()
}

// ===========================================================================
// wasm-bindgen entry point
// ===========================================================================

#[wasm_bindgen]
pub async fn fetch(req: web_sys::Request, _env: worker::Env) -> web_sys::Response {
    console_error_panic_hook::set_once();

    static INIT_LOGS: std::sync::Once = std::sync::Once::new();
    INIT_LOGS.call_once(|| {
        let _ = console_log::init_with_level(log::Level::Trace);
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::new("foundation_core=trace");
        let _ = tracing_subscriber::fmt()
            .without_time()
            .with_max_level(tracing::Level::TRACE)
            .with_env_filter(filter)
            .try_init();
    });

    POOL_GUARD.get_or_init(|| initialize_pool(42, None));

    let path = req.url();
    let path = if let Some(pos) = path.find("//") {
        let rest = &path[pos + 2..];
        if let Some(pos2) = rest.find('/') {
            rest[pos2..].to_string()
        } else {
            "/".to_string()
        }
    } else {
        path.to_string()
    };

    match path.as_str() {
        "/" => handle_home(),
        "/counter" => handle_counter().await,
        _ => html_response("Not Found", 404),
    }
}
