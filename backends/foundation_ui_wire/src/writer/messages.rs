use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    vocab::check_prop, ActionErrorT, ArrowIpcBlobT, ControlT, DataT, MessagePackBlobT, MessageT,
    OperationT, ServerCapsT, UiOperationT,
};

use super::{
    columnar::decode_batch,
    finish_message_frame,
    gate::{gate_messages, operation_is_empty},
    ClientCaps, FrameHeader, WriteError, WriteReport, WriterConfig,
};

/// Validate, filter and encode the messages for one client.
///
/// # Errors
///
/// Returns an error if validation fails, the vocabulary version is unknown or the frame is too large.
pub fn write_messages(
    messages: Vec<MessageT>,
    caps: &ClientCaps,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<(Vec<u8>, WriteReport), WriteError> {
    for (message_index, message) in messages.iter().enumerate() {
        if let Some(operation) = &message.operation {
            validate_operation(operation, message_index, config)?;
        } else if message.control.is_none() && message.data.is_none() {
            return Err(WriteError::EmptyMessage { message_index });
        }
    }
    let (messages, report) = gate_messages(messages, caps)?;
    let bytes = finish_message_frame(messages, header, config)?;
    Ok((bytes, report))
}

fn validate_operation(
    operation: &OperationT,
    message_index: usize,
    config: &WriterConfig,
) -> Result<(), WriteError> {
    if operation_is_empty(operation) {
        return Err(WriteError::EmptyMessage { message_index });
    }
    validate_ui(&operation.ui, message_index, config)?;
    if let Some(batch) = &operation.columnar {
        for row in decode_batch(batch, message_index)? {
            validate_ui(&row, message_index, config)?;
        }
    }
    Ok(())
}

fn validate_ui(
    operation: &UiOperationT,
    message_index: usize,
    config: &WriterConfig,
) -> Result<(), WriteError> {
    match operation {
        UiOperationT::SetProp(prop) => {
            check_prop(prop.prop, &prop.value).map_err(|error| WriteError::Vocab {
                message_index,
                error,
            })?;
        }
        UiOperationT::AttachState(attach) => {
            let state = attach
                .state
                .as_ref()
                .ok_or(WriteError::MissingState { message_index })?;
            let limit = config.wire_limits().action_request_bytes();
            if state.len() > limit {
                return Err(WriteError::StateTooLarge {
                    message_index,
                    limit,
                    actual: state.len(),
                });
            }
        }
        UiOperationT::ReplaceSubtree(subtree) => {
            if let Some(nested) = &subtree.operations {
                for operation in nested {
                    validate_operation(operation, message_index, config)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Encode an action error control message.
///
/// # Errors
///
/// Returns an error if the frame exceeds the configured size limit.
pub fn write_error(
    error: &ActionErrorT,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    write_control(
        ControlT {
            error: Some(Box::new(error.clone())),
            ..ControlT::default()
        },
        header,
        config,
    )
}

/// Encode a server capabilities control message.
///
/// # Errors
///
/// Returns an error if the frame exceeds the configured size limit.
pub fn write_caps(
    caps: &ServerCapsT,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    write_control(
        ControlT {
            caps: Some(Box::new(caps.clone())),
            ..ControlT::default()
        },
        header,
        config,
    )
}

fn write_control(
    control: ControlT,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    finish_message_frame(
        vec![MessageT {
            control: Some(Box::new(control)),
            ..MessageT::default()
        }],
        header,
        config,
    )
}

/// Encode an Arrow data message.
///
/// # Errors
///
/// Returns an error if the frame exceeds the configured size limit.
pub fn write_arrow_ipc(
    target_node: u32,
    ipc: &[u8],
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    write_data(
        DataT {
            target_node,
            arrow: Some(Box::new(ArrowIpcBlobT {
                target_node,
                bytes: Some(ipc.to_vec()),
            })),
            ..DataT::default()
        },
        header,
        config,
    )
}

/// Encode a `MessagePack` data message.
///
/// # Errors
///
/// Returns an error if the frame exceeds the configured size limit.
pub fn write_messagepack(
    target_node: u32,
    bytes: &[u8],
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    write_data(
        DataT {
            target_node,
            msgpack: Some(Box::new(MessagePackBlobT {
                target_node,
                bytes: Some(bytes.to_vec()),
            })),
            ..DataT::default()
        },
        header,
        config,
    )
}

fn write_data(
    data: DataT,
    header: FrameHeader,
    config: &WriterConfig,
) -> Result<Vec<u8>, WriteError> {
    finish_message_frame(
        vec![MessageT {
            data: Some(Box::new(data)),
            ..MessageT::default()
        }],
        header,
        config,
    )
}
