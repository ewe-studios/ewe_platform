//! The JSON-RPC engine over the `foundation_netio` WebSocket client.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::websocket::{WebSocketClient, WebSocketEvent, WebSocketMessage};
use serde_json::{json, Value};

use crate::error::{BrowserError, Result};
use crate::runtime::ensure_runtime;

/// A subscription to a protocol event method — a receiver of the event `params`.
pub type EventSub = Receiver<Value>;

type Pending = Arc<Mutex<HashMap<u64, SyncSender<Result<Value>>>>>;
type Subs = Arc<Mutex<HashMap<String, Vec<Sender<Value>>>>>;

/// The protocol surface the driver programs against (CDP today; `BiDi` later).
pub trait WireProtocol: Send + Sync {
    /// Send a command and block for its result. `session` is the CDP `sessionId`
    /// (or `None` for browser-level commands).
    ///
    /// # Errors
    /// [`BrowserError::Protocol`] when the browser answers with an error,
    /// [`BrowserError::Connect`] if the command cannot be written, and
    /// [`BrowserError::Timeout`] when no answer arrives before the call timeout.
    fn call(&self, method: &str, params: Value, session: Option<&str>) -> Result<Value>;
    /// Subscribe to a protocol event method (e.g. `Page.loadEventFired`).
    fn subscribe(&self, method: &str) -> EventSub;
}

/// Limits for one protocol connection.
///
/// Every field has a builder method and a documented default (owner rule 8).
#[derive(Clone, Copy, Debug)]
pub struct RpcEngineConfig {
    call_timeout: Duration,
    read_timeout: Duration,
    idle_sleep: Duration,
}

impl Default for RpcEngineConfig {
    fn default() -> Self {
        Self {
            // A local CDP call answers in under a second: `Target.createTarget`
            // took 269 ms on Chrome 157, macOS. The 30 s budget covers a slow
            // first paint and still fails a dead call in bounded time.
            call_timeout: Duration::from_secs(30),
            // The transport drains its outbound queue only between socket
            // reads, so one blocking read must return often, or a queued
            // command waits for the whole deadline. A 30 s read made every
            // call wait 29.7 s (measured, spec 88 T4). 50 ms adds at most that
            // much latency to a call that arrives during a quiet read.
            read_timeout: Duration::from_millis(50),
            // Back-off after the transport reports no message. The same 1 ms
            // the engine used before the value became configuration.
            idle_sleep: Duration::from_millis(1),
        }
    }
}

impl RpcEngineConfig {
    /// Set the budget of one command.
    #[must_use]
    pub fn with_call_timeout(mut self, timeout: Duration) -> Self {
        self.call_timeout = timeout;
        self
    }

    /// Set how long one socket read may block before the reader drains its
    /// outbound queue again. Keep it short: the transport writes queued frames
    /// only between reads.
    #[must_use]
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set the pause after a read that reported no message.
    #[must_use]
    pub fn with_idle_sleep(mut self, sleep: Duration) -> Self {
        self.idle_sleep = sleep;
        self
    }

    /// The budget of one command.
    #[must_use]
    pub fn call_timeout(&self) -> Duration {
        self.call_timeout
    }

    /// How long one socket read may block.
    #[must_use]
    pub fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    /// The pause after a read that reported no message.
    #[must_use]
    pub fn idle_sleep(&self) -> Duration {
        self.idle_sleep
    }
}

/// JSON-RPC engine: one WebSocket connection, one reader thread, `id`→pending
/// correlation, and a `method`→subscribers event bus.
pub struct RpcEngine {
    delivery: foundation_netio::websocket::MessageDelivery,
    next_id: AtomicU64,
    pending: Pending,
    subs: Subs,
    stop: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    last_reader_error: Arc<Mutex<Option<String>>>,
    reader: Option<JoinHandle<()>>,
    call_timeout: Duration,
}

impl RpcEngine {
    /// Connect to a WebSocket JSON-RPC endpoint (e.g. a CDP `webSocketDebuggerUrl`).
    ///
    /// # Errors
    /// [`BrowserError::Connect`] if the WebSocket handshake fails.
    pub fn connect(ws_url: &str, config: RpcEngineConfig) -> Result<Self> {
        ensure_runtime(None);
        let sleep_timeout = Duration::from_millis(1);
        let (mut client, delivery) = WebSocketClient::connect(
            SystemDnsResolver,
            ws_url,
            config.read_timeout,
            sleep_timeout,
        )
        .map_err(|e| BrowserError::Connect(e.to_string()))?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let subs: Subs = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let last_reader_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let reader = {
            let pending = pending.clone();
            let subs = subs.clone();
            let stop = stop.clone();
            let alive = alive.clone();
            let last_reader_error = last_reader_error.clone();
            let idle_sleep = config.idle_sleep;
            std::thread::Builder::new()
                .name("cdp-reader".into())
                .spawn(move || {
                    let mut iter = client.messages();
                    let mut reason = String::from("stopped");
                    while !stop.load(Ordering::Relaxed) {
                        match iter.next() {
                            Some(Ok(WebSocketEvent::Message(WebSocketMessage::Text(txt)))) => {
                                dispatch(&txt, &pending, &subs);
                            }
                            Some(Ok(WebSocketEvent::Message(WebSocketMessage::Close(
                                code,
                                why,
                            )))) => {
                                reason = format!("close {code} {why}");
                                break;
                            }
                            Some(Err(e)) => {
                                reason = format!("read error: {e}");
                                break;
                            }
                            None => {
                                reason = "stream ended".to_string();
                                break;
                            }
                            // Skip / ping / pong / binary — keep polling, yield a touch.
                            Some(Ok(_)) => std::thread::sleep(idle_sleep),
                        }
                    }
                    tracing::debug!(%reason, "cdp reader thread exiting");
                    // Mark the connection closed BEFORE failing the waiters, so
                    // a call that arrives now sees the flag and never waits.
                    alive.store(false, Ordering::SeqCst);
                    *last_reader_error
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(reason.clone());
                    let drained: Vec<_> = pending
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .drain()
                        .map(|(_, tx)| tx)
                        .collect();
                    for tx in drained {
                        let _ = tx.send(Err(BrowserError::ConnectionLost {
                            reason: reason.clone(),
                        }));
                    }
                })
                .map_err(|e| BrowserError::Connect(e.to_string()))?
        };

        Ok(Self {
            delivery,
            next_id: AtomicU64::new(1),
            pending,
            subs,
            stop,
            alive,
            last_reader_error,
            reader: Some(reader),
            call_timeout: config.call_timeout,
        })
    }

    /// The reason this connection is closed, or `None` while it is alive.
    fn closed_reason(&self) -> Option<String> {
        if self.alive.load(Ordering::SeqCst) {
            return None;
        }
        Some(
            self.last_reader_error
                .lock()
                .expect("reader error poisoned")
                .clone()
                .unwrap_or_else(|| "reader stopped".to_string()),
        )
    }
}

/// Route one incoming frame: an `id` resolves a pending request; a `method`
/// fans its `params` to subscribers.
fn dispatch(txt: &str, pending: &Pending, subs: &Subs) {
    tracing::trace!(frame = %txt, "cdp recv");
    let Ok(v) = serde_json::from_str::<Value>(txt) else {
        tracing::warn!("dropping non-JSON CDP frame");
        return;
    };
    if let Some(id) = v.get("id").and_then(Value::as_u64) {
        tracing::debug!(
            id,
            session = v
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "cdp recv response"
        );
        let is_error =
            v.get("error").is_some() || v.get("type").and_then(Value::as_str) == Some("error");
        let result = if is_error {
            // CDP: `error: { code, message }`. BiDi: `type:"error"`, `error:"<code>"`
            // (a string) + a top-level `message`. Handle both shapes.
            let err = v.get("error");
            let (code, message) = match err.and_then(Value::as_object) {
                Some(obj) => (
                    obj.get("code").and_then(Value::as_i64).unwrap_or(0),
                    obj.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
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
        tracing::debug!(
            method,
            session = v
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            "cdp recv event"
        );
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
        // A closed connection never answers. Fail now instead of waiting for
        // the call deadline (invariant I6).
        if let Some(reason) = self.closed_reason() {
            return Err(BrowserError::ConnectionLost { reason });
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut frame = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session {
            frame["sessionId"] = json!(s);
        }
        let (tx, rx) = sync_channel(1);
        self.pending
            .lock()
            .expect("pending poisoned")
            .insert(id, tx);
        // The reader marks the flag before it drains the waiters, so one of the
        // two checks sees a close that races this call.
        if let Some(reason) = self.closed_reason() {
            self.pending.lock().expect("pending poisoned").remove(&id);
            return Err(BrowserError::ConnectionLost { reason });
        }
        tracing::debug!(id, method, session = session.unwrap_or(""), "cdp send");
        if let Err(e) = self
            .delivery
            .send(WebSocketMessage::Text(frame.to_string()))
        {
            self.pending.lock().expect("pending poisoned").remove(&id);
            return Err(BrowserError::ConnectionLost {
                reason: format!("write failed: {e}"),
            });
        }
        if let Ok(result) = rx.recv_timeout(self.call_timeout) {
            result
        } else {
            self.pending.lock().expect("pending poisoned").remove(&id);
            if let Some(reason) = self.closed_reason() {
                return Err(BrowserError::ConnectionLost { reason });
            }
            Err(BrowserError::Timeout {
                op: method.to_string(),
                after_ms: u64::try_from(self.call_timeout.as_millis()).unwrap_or(u64::MAX),
            })
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
        let _ = self
            .delivery
            .send(WebSocketMessage::Close(1000, String::new()));
        if let Some(handle) = self.reader.take() {
            // Detach; the named daemon thread winds down on the stop flag.
            drop(handle);
        }
    }
}
