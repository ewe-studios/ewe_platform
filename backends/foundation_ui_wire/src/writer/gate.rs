use alloc::vec::Vec;

use crate::{
    vocab::{vocab_version, VocabVersion},
    ColumnarBatchT, MessageT, MobileOperationT, MountSurface, NodeKind, OperationT, UiOperationT,
    WebOperationT,
};

use super::{
    columnar::{decode_batch, encode_batch},
    ClientCaps, FilterReason, FilteredMessage, FilteredPart, WriteError, WriteReport,
};

/// Apply the platform and vocabulary rules to messages.
///
/// # Errors
///
/// Returns an error for an unknown vocabulary version, an empty payload or an invalid batch.
pub fn gate(
    messages: Vec<MessageT>,
    caps: &ClientCaps,
) -> Result<(Vec<MessageT>, Vec<FilteredMessage>), WriteError> {
    let (messages, report) = gate_messages(messages, caps)?;
    Ok((messages, report.filtered))
}

pub(super) fn gate_messages(
    messages: Vec<MessageT>,
    caps: &ClientCaps,
) -> Result<(Vec<MessageT>, WriteReport), WriteError> {
    let version =
        vocab_version(caps.vocabulary_version).ok_or(WriteError::UnknownVocabularyVersion {
            version: caps.vocabulary_version,
        })?;
    let mut kept = Vec::with_capacity(messages.len());
    let mut report = WriteReport {
        filtered: Vec::new(),
        replaced_kinds: Vec::new(),
    };
    for (message_index, mut message) in messages.into_iter().enumerate() {
        let mut changes = Changes::default();
        if let Some(operation) = &mut message.operation {
            let gate = OperationGate {
                version,
                mount: caps.mount,
                message_index,
            };
            if let Some(reason) = gate.apply(operation, &mut changes)? {
                report.filtered.push(FilteredMessage {
                    message_index,
                    part: FilteredPart::Message,
                    reason,
                });
                continue;
            }
        } else if message.control.is_none() && message.data.is_none() {
            return Err(WriteError::EmptyMessage { message_index });
        }
        if let Some((part, reason)) = changes.first {
            report.filtered.push(FilteredMessage {
                message_index,
                part,
                reason,
            });
        }
        if changes.replaced_kind {
            report.replaced_kinds.push(message_index);
        }
        kept.push(message);
    }
    Ok((kept, report))
}

#[derive(Default)]
struct Changes {
    first: Option<(FilteredPart, FilterReason)>,
    replaced_kind: bool,
}

impl Changes {
    fn record(&mut self, part: FilteredPart, reason: FilterReason) {
        if self.first.is_none() {
            self.first = Some((part, reason));
        }
    }
}

struct OperationGate<'a> {
    version: &'a VocabVersion,
    mount: MountSurface,
    message_index: usize,
}

impl OperationGate<'_> {
    fn apply(
        &self,
        operation: &mut OperationT,
        changes: &mut Changes,
    ) -> Result<Option<FilterReason>, WriteError> {
        let removed_platform = self.filter_platform(operation);
        if let Some(part) = removed_platform {
            changes.record(part, FilterReason::OtherPlatform);
        }
        if self.unknown_ui(&operation.ui)
            || self.unknown_web(&operation.web)
            || self.unknown_mobile(&operation.mobile)
        {
            return Ok(Some(FilterReason::UnknownVocabulary));
        }
        changes.replaced_kind |= self.replace_kind(&mut operation.ui);

        if let UiOperationT::ReplaceSubtree(subtree) = &mut operation.ui {
            if let Some(operations) = subtree.operations.take() {
                let mut kept = Vec::with_capacity(operations.len());
                for mut nested in operations {
                    if let Some(reason) = self.apply(&mut nested, changes)? {
                        changes.record(FilteredPart::NestedOperation, reason);
                    } else {
                        kept.push(nested);
                    }
                }
                subtree.operations = Some(kept);
            }
        }

        let removed_columnar = if let Some(batch) = &mut operation.columnar {
            self.filter_columnar(batch, changes)?
        } else {
            None
        };
        if removed_columnar.is_some() {
            operation.columnar = None;
        }
        if operation_is_empty(operation) {
            if let Some(reason) = removed_columnar {
                return Ok(Some(reason));
            }
            if removed_platform.is_some() {
                return Ok(Some(FilterReason::OtherPlatform));
            }
            return Err(WriteError::EmptyMessage {
                message_index: self.message_index,
            });
        }

        Ok(None)
    }

    fn filter_platform(&self, operation: &mut OperationT) -> Option<FilteredPart> {
        if self.mount == MountSurface::Native && !matches!(operation.web, WebOperationT::NONE) {
            operation.web = WebOperationT::NONE;
            Some(FilteredPart::Web)
        } else if self.mount == MountSurface::Web
            && !matches!(operation.mobile, MobileOperationT::NONE)
        {
            operation.mobile = MobileOperationT::NONE;
            Some(FilteredPart::Mobile)
        } else {
            None
        }
    }

    fn replace_kind(&self, operation: &mut UiOperationT) -> bool {
        if let UiOperationT::CreateNode(node) = operation {
            if node.kind.0 > self.version.max_node_kind {
                node.kind = NodeKind::UNKNOWN;
                return true;
            }
        }
        false
    }

    fn unknown_ui(&self, operation: &UiOperationT) -> bool {
        match operation {
            UiOperationT::SetProp(prop) => prop.prop.0 > self.version.max_prop_id,
            UiOperationT::BindAction(action) => action.event.0 > self.version.max_event_id,
            UiOperationT::UnbindAction(action) => action.event.0 > self.version.max_event_id,
            UiOperationT::UiBehaviour(behaviour) => {
                self.version.version == 1 || behaviour.behaviour.0 > self.version.max_behaviour
            }
            _ => false,
        }
    }

    fn unknown_web(&self, operation: &WebOperationT) -> bool {
        let number = match operation {
            WebOperationT::WebElement(element) => {
                return element.tag_id > self.version.max_tag_id;
            }
            WebOperationT::WebSetAttribute(attr) => attr.attr_id,
            WebOperationT::WebRemoveAttribute(attr) => attr.attr_id,
            WebOperationT::WebProperty(prop) => prop.name_id,
            WebOperationT::WebStyle(style) => style.name_id,
            WebOperationT::WebAddListener(listener) => listener.name_id,
            WebOperationT::WebRemoveListener(listener) => listener.name_id,
            WebOperationT::WebBehaviour(behaviour) => {
                return self.version.version == 1
                    || behaviour.behaviour.0 > self.version.max_web_behaviour;
            }
            _ => return false,
        };
        number > self.version.max_attr_id
    }

    fn unknown_mobile(&self, operation: &MobileOperationT) -> bool {
        match operation {
            MobileOperationT::NativeCommand(command) => {
                command.command.0 > self.version.max_native_command
            }
            MobileOperationT::MobileBehaviour(behaviour) => {
                self.version.version == 1
                    || behaviour.behaviour.0 > self.version.max_mobile_behaviour
            }
            _ => false,
        }
    }

    fn filter_columnar(
        &self,
        batch: &mut ColumnarBatchT,
        changes: &mut Changes,
    ) -> Result<Option<FilterReason>, WriteError> {
        let mut rows = decode_batch(batch, self.message_index)?;
        let mut replaced_kind = false;
        let before = rows.len();
        rows.retain_mut(|row| {
            replaced_kind |= self.replace_kind(row);
            !self.unknown_ui(row)
        });
        let removed = rows.len() != before;
        if removed {
            changes.record(
                FilteredPart::NestedOperation,
                FilterReason::UnknownVocabulary,
            );
            if rows.is_empty() {
                return Ok(Some(FilterReason::UnknownVocabulary));
            }
        }
        let last_a = batch.blob_a_off.as_ref().and_then(|offsets| offsets.last());
        let first_b = batch
            .blob_b_off
            .as_ref()
            .and_then(|offsets| offsets.first());
        if removed || replaced_kind || first_b != last_a {
            *batch = encode_batch(&rows, self.message_index)?;
        }
        changes.replaced_kind |= replaced_kind;
        Ok(None)
    }
}

pub(super) fn operation_is_empty(operation: &OperationT) -> bool {
    matches!(operation.ui, UiOperationT::NONE)
        && matches!(operation.web, WebOperationT::NONE)
        && matches!(operation.mobile, MobileOperationT::NONE)
        && operation.columnar.is_none()
}
