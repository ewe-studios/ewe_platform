//! Collect one render or action call and write its updates in order.

mod host;

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::writer::{write_messages, ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{
    AttachStateT, DataT, ErrorCode, MessagePackBlobT, MessageT, MountSurface, OperationT, PlaceT,
    UiOperation, UiOperationT,
};
use serde::Serialize;

use super::{ActionError, ActionResult};
use crate::state::sign_state;

/// Services, render target and output for one call.
pub struct Context {
    inner: Ctx,
    surface: MountSurface,
    state_root: Option<u32>,
    is_action: bool,
    receiver: ActionReceiver,
}

impl Context {
    /// Starts a web render with the supplied host services.
    #[must_use]
    pub fn for_render(inner: Ctx) -> Self {
        Self::for_render_on(inner, MountSurface::Web)
    }

    /// Starts a render on the selected surface.
    #[must_use]
    pub fn for_render_on(inner: Ctx, surface: MountSurface) -> Self {
        Self {
            inner,
            surface,
            state_root: None,
            is_action: false,
            receiver: ActionReceiver::default(),
        }
    }

    /// Starts an action with the component root supplied by the host.
    #[must_use]
    pub fn for_action(inner: Ctx, surface: MountSurface, state_root: Option<u32>) -> Self {
        Self {
            inner,
            surface,
            state_root,
            is_action: true,
            receiver: ActionReceiver::default(),
        }
    }

    pub(crate) fn is_action(&self) -> bool {
        self.is_action
    }

    /// Returns the host context used to start this call.
    #[must_use]
    pub fn inner(&self) -> &Ctx {
        &self.inner
    }

    #[must_use]
    pub fn surface(&self) -> MountSurface {
        self.surface
    }

    #[must_use]
    pub fn state_root(&self) -> Option<u32> {
        self.state_root
    }

    #[must_use]
    pub fn receiver(&self) -> &ActionReceiver {
        &self.receiver
    }

    #[must_use]
    pub fn into_receiver(self) -> ActionReceiver {
        self.receiver
    }
}

/// One pending update from the receiver.
pub struct ReceiverItem(Output);

enum Output {
    View(OperationT),
    State(PendingState),
    Placement {
        place: PlaceT,
        state: Option<PendingState>,
    },
    Data {
        target: u32,
        bytes: Vec<u8>,
    },
}

struct PendingState {
    targets: Vec<u32>,
    body: Vec<u8>,
}

impl PendingState {
    fn new<T: Serialize + ?Sized>(
        targets: &[u32],
        value: &T,
    ) -> Result<Self, ErrorTrace<ActionError>> {
        if targets.is_empty() {
            return Err(output_error("Component state needs at least one target"));
        }
        let body = serde_json::to_vec(value)
            .map_err(|e| output_error("Cannot encode component state").attach(e))?;
        let mut seen = BTreeSet::new();
        let targets = targets
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect();
        Ok(Self { targets, body })
    }

    fn attachments(self, ctx: &Ctx) -> Result<Vec<OperationT>, ErrorTrace<ActionError>> {
        let signed = sign_state(ctx, &self.body)
            .map_err(|e| output_error("Cannot sign component state").attach(e))?;
        Ok(self
            .targets
            .into_iter()
            .map(|target| {
                let mut attachment = AttachStateT::default();
                attachment.root_node = target;
                attachment.state = Some(signed.clone());
                let mut operation = OperationT::default();
                operation.ui = UiOperationT::AttachState(Box::new(attachment));
                operation
            })
            .collect())
    }
}

/// Stores pending updates in the order of their calls.
#[derive(Default)]
pub struct ActionReceiver {
    items: Mutex<Vec<ReceiverItem>>,
}

impl ActionReceiver {
    /// Queues the next state for one node.
    ///
    /// # Errors
    ///
    /// Returns an action error if serialization fails.
    pub fn state<T: Serialize + ?Sized>(&self, target: u32, value: &T) -> ActionResult {
        self.state_for(&[target], value)
    }

    /// Queues one state value for the root and each marked node in a render.
    /// Each distinct target receives the same signed bytes.
    ///
    /// # Errors
    ///
    /// Returns an action error if the target list is empty or serialization fails.
    pub fn state_for<T: Serialize + ?Sized>(&self, targets: &[u32], value: &T) -> ActionResult {
        self.push(Output::State(PendingState::new(targets, value)?));
        Ok(())
    }

    /// Queues application data encoded in the `MessagePack` format.
    pub fn data(&self, target: u32, bytes: Vec<u8>) {
        self.push(Output::Data { target, bytes });
    }

    /// Send MessagePack data to the view that receives this action or subscription.
    pub fn data_in_view(&self, bytes: Vec<u8>) {
        self.data(0, bytes);
    }

    /// Records an operation from the template renderer.
    #[doc(hidden)]
    pub fn render_op(&self, operation: OperationT) {
        self.push(Output::View(operation));
    }

    pub(crate) fn render_tree<S: Serialize + ?Sized>(
        &self,
        ops: Vec<OperationT>,
        placement: Option<PlaceT>,
        targets: &[u32],
        state: Option<&S>,
    ) -> ActionResult {
        let state = state.map(|s| PendingState::new(targets, s)).transpose()?;
        let mut items = self.items.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(mut place) = placement {
            place.operations = Some(ops);
            items.push(ReceiverItem(Output::Placement { place, state }));
        } else {
            items.extend(ops.into_iter().map(|op| ReceiverItem(Output::View(op))));
            if let Some(s) = state {
                items.push(ReceiverItem(Output::State(s)));
            }
        }
        Ok(())
    }

    fn push(&self, output: Output) {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ReceiverItem(output));
    }

    /// Takes all updates after the call finishes.
    #[must_use]
    pub fn drain(self) -> Vec<ReceiverItem> {
        self.items
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Signs pending state and writes one frame with the supplied client capabilities.
///
/// # Errors
///
/// Returns an action error if the signer or frame writer rejects any item.
pub fn encode_updates(
    ctx: &Ctx,
    items: Vec<ReceiverItem>,
    caps: &ClientCaps,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, ErrorTrace<ActionError>> {
    write_messages(prepare_messages(ctx, items)?, caps, header, config)
        .map(|(bytes, _report)| bytes)
        .map_err(|error| output_error("Cannot write the update frame").attach(error))
}

/// Signs queued state and builds ordered messages for the delivery handler.
///
/// # Errors
///
/// Returns an action error if signing fails or a view operation contains a state attachment.
pub fn prepare_messages(
    ctx: &Ctx,
    items: Vec<ReceiverItem>,
) -> Result<Vec<MessageT>, ErrorTrace<ActionError>> {
    let mut messages = Vec::with_capacity(items.len());
    for ReceiverItem(output) in items {
        match output {
            Output::View(operation) => {
                if contains_state_attachment(&operation) {
                    return Err(output_error("Use the receiver to attach component state."));
                }
                messages.push(operation_message(operation));
            }
            Output::State(state) => {
                messages.extend(state.attachments(ctx)?.into_iter().map(operation_message));
            }
            Output::Placement { mut place, state } => {
                let ops = place.operations.get_or_insert_with(Vec::new);
                if ops.iter().any(contains_state_attachment) {
                    return Err(output_error("Use the receiver to attach component state."));
                }
                if let Some(state) = state {
                    ops.extend(state.attachments(ctx)?);
                }
                let mut operation = OperationT::default();
                operation.ui = UiOperationT::Place(Box::new(place));
                messages.push(operation_message(operation));
            }
            Output::Data { target, bytes } => {
                let mut blob = MessagePackBlobT::default();
                blob.target_node = target;
                blob.bytes = Some(bytes);
                let mut data = DataT::default();
                data.target_node = target;
                data.msgpack = Some(Box::new(blob));
                let mut message = MessageT::default();
                message.data = Some(Box::new(data));
                messages.push(message);
            }
        }
    }
    Ok(messages)
}

fn contains_state_attachment(operation: &OperationT) -> bool {
    if operation
        .columnar
        .as_deref()
        .and_then(|batch| batch.op_codes.as_deref())
        .is_some_and(|codes| codes.contains(&UiOperation::AttachState.0))
    {
        return true;
    }
    match &operation.ui {
        UiOperationT::AttachState(_) => true,
        UiOperationT::Place(place) => place
            .operations
            .as_deref()
            .is_some_and(|ops| ops.iter().any(contains_state_attachment)),
        UiOperationT::ReplaceSubtree(tree) => tree
            .operations
            .as_deref()
            .is_some_and(|children| children.iter().any(contains_state_attachment)),
        _ => false,
    }
}

fn operation_message(operation: OperationT) -> MessageT {
    let mut message = MessageT::default();
    message.operation = Some(Box::new(operation));
    message
}

fn output_error(detail: impl Into<String>) -> ErrorTrace<ActionError> {
    ErrorTrace::new(ActionError::new(ErrorCode::ActionFailed, detail))
}
