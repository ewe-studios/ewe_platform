//! Build a host action dispatcher and negotiate actions with a remote service.
use crate::{
    action::{
        ActionManager, ActionRef, ActionRegistry, ManagerError, Origin, Transport, TriggerEvent,
    },
    dispatch::{Connection, DispatcherBuilder, LocalDispatcher},
};
use foundation_context::Ctx;
use foundation_ui_wire::{
    finish_frame, read_frame,
    writer::{ClientCaps, WriterConfig},
    ClientHelloT, FrameKindT, FrameMessage, FrameRequestT, FrameT, MountSurface, Operation,
    ServerCapsT, WebOperation, WireLimits, SCHEMA_VERSION,
};
use std::sync::Arc;

/// Owns the action manager and its server transport for one application.
pub struct NetworkApp {
    manager: ActionManager,
    transport: Arc<Transport>,
    caps: ClientCaps,
    writer: WriterConfig,
}

/// Holds the application inputs until a server transport is available.
pub struct NetworkAppBuilder {
    ctx: Ctx,
    actions: ActionRegistry,
    caps: ClientCaps,
}

/// Holds the server transport that the application will use.
pub struct ConnectedAppBuilder {
    inputs: NetworkAppBuilder,
    transport: Arc<Transport>,
}

impl NetworkApp {
    #[must_use]
    pub fn builder(
        ctx: Ctx,
        actions: ActionRegistry,
        mut caps: ClientCaps,
        surface: MountSurface,
    ) -> NetworkAppBuilder {
        caps.mount = surface;
        NetworkAppBuilder { ctx, actions, caps }
    }

    /// Sends the hello. A transport can deliver capabilities later through `receive_capabilities`.
    ///
    /// # Errors
    /// Returns a transport error or an invalid capability reply.
    pub async fn connect(&self) -> Result<(), ManagerError> {
        let request = client_hello(&self.caps);
        if let Some(reply) = self.transport.send(request).await? {
            self.receive_capabilities(&reply)?;
        }
        Ok(())
    }

    /// Replaces the server action list after a complete capability reply arrives.
    ///
    /// # Errors
    /// Returns the frame error or the required version update.
    pub fn receive_capabilities(&self, bytes: &[u8]) -> Result<(), ManagerError> {
        let caps = server_capabilities(bytes, self.writer.wire_limits(), self.caps.mount)?;
        self.manager.apply_caps(&caps);
        if caps.status != foundation_ui_wire::CapsStatus::Ok {
            return Err(invalid("The server requires a client update"));
        }
        Ok(())
    }

    /// Sends the action to its owner and validates any immediate reply.
    ///
    /// # Errors
    /// Returns the lookup, transport or frame error.
    pub async fn trigger(
        &self,
        reference: &str,
        event: TriggerEvent,
    ) -> Result<Option<Vec<u8>>, ManagerError> {
        let reply = self.manager.trigger(reference, event).await?;
        if let Some(bytes) = &reply {
            reply_message(bytes, self.writer.wire_limits(), self.caps.mount)?;
        }
        Ok(reply)
    }
}

impl NetworkAppBuilder {
    #[must_use]
    pub fn with_server_transport(self, transport: Arc<Transport>) -> ConnectedAppBuilder {
        ConnectedAppBuilder {
            inputs: self,
            transport,
        }
    }
}

impl ConnectedAppBuilder {
    #[must_use]
    pub fn build(self) -> NetworkApp {
        let NetworkAppBuilder { ctx, actions, caps } = self.inputs;
        let writer = WriterConfig::default();
        let manager = client_manager(ctx, Arc::new(actions), &caps, &writer)
            .with_server(self.transport.clone());
        manager.apply_caps(&ServerCapsT::default());
        NetworkApp {
            manager,
            transport: self.transport,
            caps,
            writer,
        }
    }
}

/// Builds the local dispatcher and action manager for this screen.
#[must_use]
pub fn client_manager(
    ctx: Ctx,
    registry: Arc<ActionRegistry>,
    caps: &ClientCaps,
    writer: &WriterConfig,
) -> ActionManager {
    let dispatcher = DispatcherBuilder::new(Origin::Host, registry.clone())
        .writer_config(writer.clone())
        .build();
    let connection = Connection::stateless(caps.clone(), caps.mount, 0);
    ActionManager::new(registry, caps.mount, 0, *writer.wire_limits())
        .with_host(Arc::new(LocalDispatcher::new(dispatcher, ctx, connection)))
}

fn invalid(message: impl std::fmt::Display) -> ManagerError {
    ManagerError::Transport(message.to_string())
}

/// Builds the fixed-size capability request for this screen.
///
/// # Panics
/// Panics if the wire schema cannot encode the fixed-size hello.
#[must_use]
pub fn client_hello(caps: &ClientCaps) -> Vec<u8> {
    tagged_client_hello(caps, 0)
}

/// Identifies one capability exchange on a shared connection.
///
/// # Panics
/// Panics if the wire schema cannot encode the fixed-size hello.
#[must_use]
pub fn tagged_client_hello(caps: &ClientCaps, tag: u64) -> Vec<u8> {
    let mut hello = ClientHelloT::default();
    hello.request_tag = tag;
    hello.subscription_resume =
        caps.vocabulary_version >= foundation_ui_wire::vocab::MANAGED_CONNECTION_VERSION;
    hello.platform = caps.platform;
    hello.mount = caps.mount;
    hello.vocabulary_version = caps.vocabulary_version;
    let mut request = FrameRequestT::default();
    request.hello = Some(Box::new(hello));
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.sequence_id = 1;
    frame.kind = FrameKindT::FrameRequest(Box::new(request));
    finish_frame(&frame).expect("Fixed-size client hello fits the wire format")
}

/// Checks the frame type and all operations for this surface.
///
/// # Errors
/// Returns an error for invalid frame bytes or a web operation on a native surface.
pub fn reply_message<'a>(
    bytes: &'a [u8],
    limits: &WireLimits,
    surface: MountSurface,
) -> Result<FrameMessage<'a>, ManagerError> {
    let frame = read_frame(bytes, limits).map_err(invalid)?;
    if frame.schema_version() != SCHEMA_VERSION {
        return Err(invalid("Unsupported frame schema"));
    }
    let body = frame
        .kind_as_frame_message()
        .ok_or_else(|| invalid("Expected a message frame"))?;
    for msg in body.messages().into_iter().flatten() {
        let fields = u8::from(msg.operation().is_some())
            + u8::from(msg.control().is_some())
            + u8::from(msg.data().is_some());
        if fields != 1 {
            return Err(invalid("A message must have exactly one body"));
        }
        if let Some(op) = msg.operation().filter(|_| surface == MountSurface::Native) {
            validate_native(op)?;
        }
    }
    Ok(body)
}

fn validate_native(op: Operation<'_>) -> Result<(), ManagerError> {
    if op.web_type() != WebOperation::NONE {
        return Err(invalid("Web operation on a native mount"));
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

/// Checks the capability list and returns the advertised actions.
///
/// # Errors
/// Returns an error for invalid versions and missing or duplicate declarations.
pub fn server_capabilities(
    bytes: &[u8],
    limits: &WireLimits,
    surface: MountSurface,
) -> Result<ServerCapsT, ManagerError> {
    let body = reply_message(bytes, limits, surface)?;
    let mut lists = body
        .messages()
        .into_iter()
        .flatten()
        .filter_map(|m| m.control().and_then(|c| c.caps()));
    let caps = lists
        .next()
        .ok_or_else(|| invalid("Missing server capabilities"))?;
    if lists.next().is_some() {
        return Err(invalid("Duplicate server capabilities"));
    }
    match caps.status() {
        foundation_ui_wire::CapsStatus::Ok
            if caps.schema_version() == SCHEMA_VERSION
                && foundation_ui_wire::vocab::vocab_version(caps.vocabulary_version())
                    .is_some() => {}
        foundation_ui_wire::CapsStatus::UpdateRequired => {}
        _ => return Err(invalid("Invalid server capability versions")),
    }
    let mut names = std::collections::BTreeSet::new();
    for action in caps.actions().into_iter().flatten() {
        let name = action
            .name()
            .ok_or_else(|| invalid("Missing advertised action name"))?;
        let reference = format!("server:{name}");
        ActionRef::parse(&reference).map_err(invalid)?;
        if !names.insert(name) {
            return Err(invalid("Duplicate advertised action"));
        }
    }
    Ok(caps.unpack())
}
