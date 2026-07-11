//! [`WebSocketConnector`] impl for [`NativeHttpClient`] (F51 Stage 6).
//!
//! WHY: F51 unifies the client — the same concrete type that serves HTTP also
//! opens `WebSocket` connections through the [`WebSocketConnector`] trait, and
//! returns the cross-platform [`WebSocketClient`] or [`WsExchangeTask`] rather than
//! a native-only connection type. That is what lets a caller write one call that
//! compiles and works on both native and wasm.
//!
//! WHAT: Implements `WebSocketConnector::open_websocket` by driving a native
//! WebSocket task over the client's own resolver (DNS → TCP → TLS → HTTP/1.1
//! Upgrade), honouring the shared [`WebSocketConnectConfig`] (subprotocols,
//! headers, timeouts, and — native-only — reconnection).
//!
//! `open_websocket_task` returns the raw task (un-executed) so the caller
//! decides when to `valtron::execute()` it on their own pool — mirrors
//! `HttpClient::open_exchange()`.
//!
//! HOW: Delegates to [`WebSocketClient::connect_with_config`] for the client
//! and [`WebSocketClient::connect_parts`] for the task path.

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskSpread, TaskStatus};

use crate::http::NativeHttpClient;
use crate::shared::client::DnsResolver;
use crate::websocket::native::connection::{native_progress, WsPending};
use crate::websocket::shared::client::{
    MessageDelivery, WebSocketClient, WebSocketConnectConfig, WsProgress,
};
use crate::websocket::shared::connector::{WebSocketConnector, WsExchangeTask};
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::message::WebSocketMessage;

/// `WsPending` (native) → `WsProgress` (shared) adapter.
///
/// Wraps a `TaskIterator<Ready=Result<WebSocketMessage, WebSocketError>, Pending=WsPending>`
/// and maps the `Pending` variant. Implements `TaskIterator` directly via delegation
/// so it works with both the `WsTask` enum and `WasmWebSocketBridge`.
struct PendingMapWsTask<I> {
    inner: I,
}

impl<I> TaskIterator for PendingMapWsTask<I>
where
    I: TaskIterator<
        Ready = Result<WebSocketMessage, WebSocketError>,
        Pending = WsPending,
        Spawner = BoxedSendExecutionAction,
    >,
{
    type Ready = Result<WebSocketMessage, WebSocketError>;
    type Pending = WsProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.inner.next_status()? {
            TaskStatus::Ready(d) => Some(TaskStatus::Ready(d)),
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(native_progress(p))),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Wait => Some(TaskStatus::Wait),
            TaskStatus::Spread(items) => {
                let mapped: Vec<_> = items
                    .into_iter()
                    .map(|spread| match spread {
                        TaskSpread::Ready(d) => TaskSpread::Ready(d),
                        TaskSpread::Pending(p) => {
                            TaskSpread::Pending(native_progress(p))
                        }
                    })
                    .collect();
                Some(TaskStatus::Spread(mapped))
            }
            TaskStatus::Depends(ptr) => Some(TaskStatus::Depends(ptr)),
        }
    }
}

impl<R: DnsResolver + Clone + Send + Sync + 'static> WebSocketConnector for NativeHttpClient<R> {
    fn open_websocket(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        let (client, _delivery) =
            WebSocketClient::connect_with_config(self.resolver().clone(), url, &config)?;
        Ok(client)
    }

    fn open_websocket_task(
        &self,
        url: &str,
        config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError> {
        let (ws_task, delivery) = WebSocketClient::connect_parts(
            self.resolver().clone(),
            url,
            config.reconnect,
            config.read_timeout,
            config.sleep_timeout,
        )?;
        let boxed: WsExchangeTask = Box::new(PendingMapWsTask { inner: ws_task });
        Ok((boxed, delivery))
    }
}
