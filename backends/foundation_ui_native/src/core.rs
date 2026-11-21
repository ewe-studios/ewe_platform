//! Native core construction, delivery and frame validation.
use crate::{
    action_sender, CoreError, EventInput, FrameListener, HostTransport, HostTransportProvider,
    ServiceError,
};
use foundation_context::Ctx;
use foundation_ui_runtime::{
    action::{ActionManager, ActionRegistry, Origin, Scope},
    network::{client_hello, client_manager, reply_message, server_capabilities},
};
use foundation_ui_wire::{
    read_frame,
    writer::{write_messages, ClientCaps, FrameHeader, StreamSequencer, WriterConfig},
    OperationT, Platform,
};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePlatform {
    Ios,
    Android,
}
impl From<NativePlatform> for Platform {
    fn from(platform: NativePlatform) -> Self {
        match platform {
            NativePlatform::Ios => Self::Ios,
            NativePlatform::Android => Self::Android,
        }
    }
}

/// The application installs its action registry, signer, and render target in Rust.
pub struct CoreConfig {
    pub ctx: Ctx,
    pub registry: Arc<ActionRegistry>,
    pub platform: NativePlatform,
    pub writer: WriterConfig,
}
/// Sets up one mobile core for a screen.
pub struct NativeCoreBuilder {
    config: CoreConfig,
    transport: Option<Arc<dyn HostTransport>>,
}

impl NativeCoreBuilder {
    #[must_use]
    pub fn with_host_transport(mut self, transport: Arc<dyn HostTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Looks up the sender once and keeps it for this screen.
    ///
    /// # Errors
    /// Returns the native service lookup error.
    pub async fn with_service(
        self,
        provider: &dyn HostTransportProvider,
        service: String,
    ) -> Result<Self, ServiceError> {
        Ok(self.with_host_transport(provider.action_transport(service).await?.transport.clone()))
    }

    #[must_use]
    pub fn with_writer(mut self, writer: WriterConfig) -> Self {
        self.config.writer = writer;
        self
    }

    #[must_use]
    pub fn build(self) -> Arc<NativeCore> {
        NativeCore::new(self.config, self.transport)
    }
}

pub(crate) struct Push {
    #[cfg(feature = "app")]
    pub ticket: u64,
    sequence_offset: usize,
    pub listener: Arc<dyn FrameListener>,
    pub bytes: Vec<u8>,
}

struct Delivery {
    tickets: StreamSequencer,
    sequence: StreamSequencer,
    queue: std::collections::VecDeque<Push>,
    draining: bool,
}

#[derive(uniffi::Object)]
pub struct NativeCore {
    manager: ActionManager,
    caps: ClientCaps,
    writer: WriterConfig,
    delivery: Mutex<Delivery>,
    listener: Mutex<Option<Arc<dyn FrameListener>>>,
}
impl NativeCore {
    #[must_use]
    pub fn builder(
        platform: NativePlatform,
        ctx: Ctx,
        actions: ActionRegistry,
    ) -> NativeCoreBuilder {
        NativeCoreBuilder {
            config: CoreConfig {
                platform,
                ctx,
                registry: Arc::new(actions),
                writer: WriterConfig::default(),
            },
            transport: None,
        }
    }

    #[must_use]
    pub fn new(config: CoreConfig, server: Option<Arc<dyn HostTransport>>) -> Arc<Self> {
        let caps = ClientCaps::native_current(config.platform.into());
        let mut manager = client_manager(config.ctx, config.registry, &caps, &config.writer);
        if let Some(server) = server {
            manager = manager.with_server(action_sender(server));
        }
        Arc::new(Self {
            manager,
            caps,
            writer: config.writer,
            delivery: Mutex::new(Delivery {
                tickets: StreamSequencer::new(0),
                sequence: StreamSequencer::new(0),
                queue: std::collections::VecDeque::default(),
                draining: false,
            }),
            listener: Mutex::new(None),
        })
    }

    /// Sends a complete local update to the current listener.
    ///
    /// # Errors
    /// Returns an error if the listener is absent, the sequence is exhausted, or the writer rejects an operation.
    pub fn push_ops(&self, ops: Vec<OperationT>) -> Result<u64, CoreError> {
        let push = self.prepare_push(ops)?;
        Ok(self.publish(push))
    }

    pub(crate) fn limits(&self) -> &foundation_ui_wire::WireLimits {
        self.writer.wire_limits()
    }

    pub(crate) fn prepare_push(&self, ops: Vec<OperationT>) -> Result<Push, CoreError> {
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(CoreError::NoListener)?;
        let messages = ops
            .into_iter()
            .map(foundation_ui_wire::ops::message)
            .collect();
        // A nonzero placeholder reserves the generated sequence field in this owned buffer.
        let header = FrameHeader {
            stream_id: 0,
            sequence_id: 1,
            ack: 0,
        };
        let (bytes, _) =
            write_messages(messages, &self.caps, header, &self.writer).map_err(CoreError::write)?;
        let frame = read_frame(&bytes, self.limits()).map_err(CoreError::write)?;
        let sequence_offset = frame._tab.loc()
            + usize::from(
                frame
                    ._tab
                    .vtable()
                    .get(foundation_ui_wire::Frame::VT_SEQUENCE_ID),
            );
        let mut delivery = self.delivery.lock().unwrap_or_else(PoisonError::into_inner);
        if delivery.tickets.last() == u64::MAX {
            return Err(CoreError::write("No push identifiers remain"));
        }
        delivery.tickets.next();
        Ok(Push {
            #[cfg(feature = "app")]
            ticket: delivery.tickets.last(),
            sequence_offset,
            listener,
            bytes,
        })
    }

    pub(crate) fn publish(&self, mut push: Push) -> u64 {
        let sequence = {
            let mut delivery = self.delivery.lock().unwrap_or_else(PoisonError::into_inner);
            // Every publication owns one unique ticket. A cancelled ticket consumes no sequence.
            let sequence = delivery.sequence.next();
            let offset = push.sequence_offset;
            push.bytes[offset..offset + 8].copy_from_slice(&sequence.to_le_bytes());
            delivery.queue.push_back(push);
            if delivery.draining {
                return sequence;
            }
            delivery.draining = true;
            sequence
        };
        let mut panic = None;
        loop {
            let next = {
                let mut delivery = self.delivery.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(next) = delivery.queue.pop_front() {
                    next
                } else {
                    delivery.draining = false;
                    break;
                }
            };
            let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                next.listener.on_frame(next.bytes);
            }));
            if let Err(payload) = callback {
                panic.get_or_insert(payload);
            }
        }
        if let Some(payload) = panic {
            std::panic::resume_unwind(payload)
        }
        sequence
    }
}

#[uniffi::export]
impl NativeCore {
    /// Creates a core for a foreign application that calls actions on a server.
    ///
    /// # Errors
    /// Rejects duplicate or invalid server action names.
    #[uniffi::constructor]
    pub fn remote(
        platform: NativePlatform,
        names: Vec<String>,
        transport: Arc<dyn HostTransport>,
    ) -> Result<Arc<Self>, CoreError> {
        let mut registry = ActionRegistry::new();
        for name in names {
            registry
                .declare(Some(Origin::Server), &name, Scope::All)
                .map_err(|e| CoreError::BadReference {
                    detail: e.to_string(),
                })?;
        }
        Ok(Self::new(
            CoreConfig {
                ctx: Ctx::host(),
                registry: Arc::new(registry),
                platform,
                writer: WriterConfig::default(),
            },
            Some(transport),
        ))
    }

    /// Returns the hello for this platform and native mount.
    ///
    /// # Panics
    /// Panics if the generated schema can no longer encode its fixed-size hello.
    #[must_use]
    pub fn hello(&self) -> Vec<u8> {
        client_hello(&self.caps)
    }
    /// Replaces the advertised server actions after validating the complete frame.
    ///
    /// # Errors
    /// Returns an error for an invalid frame, a missing capability list, or two lists.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Foreign callers pass an owned byte buffer through UniFFI."
    )]
    pub fn apply_caps_frame(&self, frame: Vec<u8>) -> Result<(), CoreError> {
        let caps = server_capabilities(&frame, self.limits(), self.caps.mount)
            .map_err(CoreError::write)?;
        self.manager.apply_caps(&caps);
        Ok(())
    }
    /// Runs a local action or sends one request to the server.
    ///
    /// # Errors
    /// Returns the routing, transport or frame validation error.
    pub async fn trigger(
        &self,
        reference: String,
        event: EventInput,
    ) -> Result<Option<Vec<u8>>, CoreError> {
        self.send_event(reference, event.into()).await
    }
    /// Send application JSON bytes with the state and event metadata from the triggering node.
    ///
    /// # Errors
    /// Reports unsupported input, routing and transport failures.
    pub async fn trigger_structured(
        &self,
        reference: String,
        event: EventInput,
        json: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, CoreError> {
        let mut event = foundation_ui_runtime::action::TriggerEvent::from(event);
        event.input.value = None;
        event.input.json = Some(json);
        self.send_event(reference, event).await
    }
    pub fn set_frame_listener(&self, listener: Arc<dyn FrameListener>) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(listener);
    }
    pub fn clear_frame_listener(&self) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

impl NativeCore {
    async fn send_event(
        &self,
        reference: String,
        event: foundation_ui_runtime::action::TriggerEvent,
    ) -> Result<Option<Vec<u8>>, CoreError> {
        let reply = self
            .manager
            .trigger(&reference, event)
            .await
            .map_err(|e| CoreError::manager(e, &reference))?;
        if let Some(bytes) = &reply {
            reply_message(bytes, self.limits(), self.caps.mount).map_err(CoreError::write)?;
        }
        Ok(reply)
    }
}
