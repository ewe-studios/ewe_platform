//! The JSON-RPC engine over the `foundation_netio` WebSocket client.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_netio::simple_http::client::shared::dns::SystemDnsResolver;
use foundation_netio::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use serde_json::{json, Value};

use crate::error::{BrowserError, Result};
use crate::runtime::ensure_runtime;

/// A subscription to a protocol event method — a receiver of the event `params`.
pub type EventSub = Receiver<Value>;

type Pending = Arc<Mutex<HashMap<u64, SyncSender<Result<Value>>>>>;
type Subs = Arc<Mutex<HashMap<String, Vec<Sender<Value>>>>>;

/// The protocol surface the driver programs against (CDP today; BiDi later).
pub trait WireProtocol: Send + Sync {
    /// Send a command and block for its result. `session` is the CDP `sessionId`
    /// (or `None` for browser-level commands).
    fn call(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value>;
    /// Subscribe to a protocol event method (e.g. `Page.loadEventFired`).
    fn subscribe(&self, method: &str) -> EventSub;
}

/// JSON-RPC engine: one WebSocket connection, one reader thread, `id`→pending
/// correlation, and a `method`→subscribers event bus.
pub struct RpcEngine {
    delivery: foundation_netio::websocket::MessageDelivery,
    next_id: AtomicU64,
    pending: Pending,
    subs: Subs,
    stop: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    call_timeout: Duration,
}

impl RpcEngine {
    /// Connect to a WebSocket JSON-RPC endpoint (e.g. a CDP `webSocketDebuggerUrl`).
    ///
    /// # Errors
    /// [`BrowserError::Connect`] if the WebSocket handshake fails.
    pub fn connect(ws_url: &str, call_timeout: Duration) -> Result<Self> {
        ensure_runtime(None);
        let read_timeout = Duration::from_secs(30);
        let sleep_timeout = Duration::from_millis(1);
        let (mut client, delivery) =
            WebSocketClient::connect(SystemDnsResolver, ws_url, read_timeout, sleep_timeout)
                .map_err(|e| BrowserError::Connect(e.to_string()))?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let subs: Subs = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let reader = {
            let pending = pending.clone();
            let subs = subs.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("cdp-reader".into())
                .spawn(move || {
                    let mut iter = client.messages();
                    while !stop.load(Ordering::Relaxed) {
                        match iter.next() {
                            Some(Ok(WebSocketEvent::Message(WebSocketMessage::Text(txt)))) => {
                                dispatch(&txt, &pending, &subs);
                            }
                            Some(Ok(WebSocketEvent::Message(WebSocketMessage::Close(..)))) => break,
                            // Skip / ping / pong / binary — keep polling, yield a touch.
                            Some(Ok(_)) => std::thread::sleep(Duration::from_millis(1)),
                            Some(Err(_)) | None => break,
                        }
                    }
                    tracing::debug!("cdp reader thread exiting");
                })
                .map_err(|e| BrowserError::Connect(e.to_string()))?
        };

        Ok(Self {
            delivery,
            next_id: AtomicU64::new(1),
            pending,
            subs,
            stop,
            reader: Some(reader),
            call_timeout,
        })
    }
}

/// Route one incoming frame: an `id` resolves a pending request; a `method`
/// fans its `params` to subscribers.
fn dispatch(txt: &str, pending: &Pending, subs: &Subs) {
    let Ok(v) = serde_json::from_str::<Value>(txt) else {
        tracing::warn!("dropping non-JSON CDP frame");
        return;
    };
    if let Some(id) = v.get("id").and_then(Value::as_u64) {
        let is_error = v.get("error").is_some() || v.get("type").and_then(Value::as_str) == Some("error");
        let result = if is_error {
            // CDP: `error: { code, message }`. BiDi: `type:"error"`, `error:"<code>"`
            // (a string) + a top-level `message`. Handle both shapes.
            let err = v.get("error");
            let (code, message) = match err.and_then(Value::as_object) {
                Some(obj) => (
                    obj.get("code").and_then(Value::as_i64).unwrap_or(0),
                    obj.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
                ),
                None => (
                    0,
                    v.get("message")
                        .and_then(Value::as_str)
                        .or_else(|| err.and_then(Value::as_str))
                        .unwrap_or("protocol error")
                        .to_string(),
                ),
            };
            Err(BrowserError::Protocol { code, message })
        } else {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        };
        if let Some(tx) = pending.lock().expect("pending poisoned").remove(&id) {
            let _ = tx.send(result);
        }
    } else if let Some(method) = v.get("method").and_then(Value::as_str) {
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        if let Some(list) = subs.lock().expect("subs poisoned").get(method) {
            for tx in list {
                let _ = tx.send(params.clone());
            }
        }
    }
}

impl WireProtocol for RpcEngine {
    fn call(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut frame = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            frame["sessionId"] = json!(s);
        }
        let (tx, rx) = sync_channel(1);
        self.pending.lock().expect("pending poisoned").insert(id, tx);
        self.delivery
            .send(WebSocketMessage::Text(frame.to_string()))
            .map_err(|e| BrowserError::Connect(e.to_string()))?;
        match rx.recv_timeout(self.call_timeout) {
            Ok(result) => result,
            Err(_) => {
                self.pending.lock().expect("pending poisoned").remove(&id);
                Err(BrowserError::Timeout {
                    op: method.to_string(),
                    after_ms: u64::try_from(self.call_timeout.as_millis()).unwrap_or(u64::MAX),
                })
            }
        }
    }

    fn subscribe(&self, method: &str) -> EventSub {
        let (tx, rx) = channel();
        self.subs
            .lock()
            .expect("subs poisoned")
            .entry(method.to_string())
            .or_default()
            .push(tx);
        rx
    }
}

impl Drop for RpcEngine {
    fn drop(&mut self) {
        // Stop the reader and best-effort close the socket. We do NOT join — the
        // reader exits on the stop flag at its next poll; joining risks a hang if
        // the socket read is mid-flight.
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.delivery.send(WebSocketMessage::Close(1000, String::new()));
        if let Some(handle) = self.reader.take() {
            // Detach; the named daemon thread winds down on the stop flag.
            drop(handle);
        }
    }
}
