//! Native core construction, delivery and frame validation.
use crate::{host::Adapter, CoreError, EventInput, FrameListener, HostTransport};
use foundation_context::Ctx;
use foundation_ui_runtime::{
    action::{ActionManager, ActionRegistry, Origin, Scope},
    dispatch::{Connection, DispatcherBuilder, LocalDispatcher},
};
use foundation_ui_wire::{
    finish_frame, read_frame,
    writer::{write_messages, ClientCaps, FrameHeader, StreamSequencer, WriterConfig},
    ClientHelloT, FrameKindT, FrameMessage, FrameRequestT, FrameT, MountSurface, Operation,
    OperationT, Platform, WebOperation, SCHEMA_VERSION,
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
    pub fn new(config: CoreConfig, server: Option<Arc<dyn HostTransport>>) -> Arc<Self> {
        let caps = ClientCaps::native_current(config.platform.into());
        let dispatcher = DispatcherBuilder::new(Origin::Host, config.registry.clone())
            .writer_config(config.writer.clone())
            .build();
        let connection = Connection::stateless(caps.clone(), MountSurface::Native, 0);
        let local = LocalDispatcher::new(dispatcher, config.ctx, connection);
        let mut manager = ActionManager::new(
            config.registry,
            MountSurface::Native,
            0,
            *config.writer.wire_limits(),
        )
        .with_host(Arc::new(local));
        if let Some(server) = server {
            manager = manager.with_server(Arc::new(Adapter(server)));
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

    fn message<'a>(&self, bytes: &'a [u8]) -> Result<FrameMessage<'a>, CoreError> {
        let frame = read_frame(bytes, self.limits()).map_err(CoreError::write)?;
        if frame.schema_version() != SCHEMA_VERSION {
            return Err(CoreError::write("Unsupported frame schema"));
        }
        let body = frame
            .kind_as_frame_message()
            .ok_or_else(|| CoreError::write("Expected a message frame"))?;
        for msg in body.messages().into_iter().flatten() {
            let fields = u8::from(msg.operation().is_some())
                + u8::from(msg.control().is_some())
                + u8::from(msg.data().is_some());
            if fields != 1 {
                return Err(CoreError::write("A message must have exactly one body"));
            }
            if let Some(op) = msg.operation() {
                validate_native(op)?;
            }
        }
        Ok(body)
    }
}

fn validate_native(op: Operation<'_>) -> Result<(), CoreError> {
    if op.web_type() != WebOperation::NONE {
        return Err(CoreError::write("Web operation on a native mount"));
    }
    let children = op
        .ui_as_replace_subtree()
        .and_then(|tree| tree.operations())
        .or_else(|| op.ui_as_place().and_then(|tree| tree.operations()));
    for child in children.into_iter().flatten() {
        validate_native(child)?;
    }
    Ok(())
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
        let mut hello = ClientHelloT::default();
        hello.platform = self.caps.platform;
        hello.mount = MountSurface::Native;
        hello.vocabulary_version = self.caps.vocabulary_version;
        let mut request = FrameRequestT::default();
        request.hello = Some(Box::new(hello));
        let mut frame = FrameT::default();
        frame.schema_version = SCHEMA_VERSION;
        frame.sequence_id = 1;
        frame.kind = FrameKindT::FrameRequest(Box::new(request));
        finish_frame(&frame).expect("Fixed-size native hello fits the wire format")
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
        let body = self.message(&frame)?;
        let mut lists = body
            .messages()
            .into_iter()
            .flatten()
            .filter_map(|m| m.control().and_then(|c| c.caps()));
        let caps = lists
            .next()
            .ok_or_else(|| CoreError::write("Missing server capabilities"))?;
        if lists.next().is_some() {
            return Err(CoreError::write("Duplicate server capabilities"));
        }
        match caps.status() {
            foundation_ui_wire::CapsStatus::Ok
                if caps.schema_version() == SCHEMA_VERSION
                    && foundation_ui_wire::vocab::vocab_version(caps.vocabulary_version())
                        .is_some() => {}
            foundation_ui_wire::CapsStatus::UpdateRequired => {}
            _ => return Err(CoreError::write("Invalid server capability versions")),
        }
        let mut names = std::collections::BTreeSet::new();
        for action in caps.actions().into_iter().flatten() {
            let name = action
                .name()
                .ok_or_else(|| CoreError::write("Missing advertised action name"))?;
            let reference = format!("server:{name}");
            foundation_ui_runtime::action::ActionRef::parse(&reference)
                .map_err(CoreError::write)?;
            if !names.insert(name) {
                return Err(CoreError::write("Duplicate advertised action"));
            }
        }
        self.manager.apply_caps(&caps.unpack());
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
        let reply = self
            .manager
            .trigger(&reference, event.into())
            .await
            .map_err(|e| CoreError::manager(e, &reference))?;
        if let Some(bytes) = &reply {
            self.message(bytes)?;
        }
        Ok(reply)
    }
    pub fn set_frame_listener(&self, listener: Arc<dyn FrameListener>) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(listener);
    }
    pub fn clear_frame_listener(&self) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}
