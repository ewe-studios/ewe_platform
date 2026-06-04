/// FFI layer — `extern "C"` bindings for the IPC bus.
///
/// Provides opaque types and functions for C/C++ consumers.
/// All functions return 0 on success, -1 on error.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::time::Duration;

use crate::ipc::message::{BytesMessage, Message, MessageBox};
use crate::ipc::options::Options;
use crate::ipc::label::Label;
use crate::ipc::selector::Selector;
use crate::ipc::label::LabelOp;
use crate::ipc::bus_controller::{EndpointSender, EndpointReceiver};

#[repr(C)]
pub struct ipmb_Sender {
    inner: *mut EndpointSender<BytesMessage>,
}

#[repr(C)]
pub struct ipmb_Receiver {
    inner: *mut EndpointReceiver<BytesMessage>,
}

#[repr(C)]
pub struct ipmb_Message {
    pub format: u16,
    pub data: *mut u8,
    pub data_len: u32,
}

#[repr(C)]
pub struct ipmb_Options {
    pub identifier: *const c_char,
    pub label: *const c_char,
    pub token: *const c_char,
    pub controller_affinity: c_int,
}

unsafe fn parse_options(raw: &ipmb_Options) -> Option<Options> {
    let identifier = CStr::from_ptr(raw.identifier).to_str().ok()?.to_string();
    let label = Label::new(CStr::from_ptr(raw.label).to_str().ok()?);
    let token = if raw.token.is_null() {
        String::new()
    } else {
        CStr::from_ptr(raw.token).to_str().ok()?.to_string()
    };
    let mut opts = Options::new(identifier, label).token(token);
    if raw.controller_affinity != 0 {
        opts = opts.controller_affinity(true);
    }
    Some(opts)
}

/// Join a message bus. Writes sender and receiver handles to out pointers.
///
/// # Safety
/// All pointers must be valid and properly aligned.
#[no_mangle]
pub unsafe extern "C" fn ipmb_join(
    options: ipmb_Options,
    timeout_ms: u32,
    out_sender: *mut *mut ipmb_Sender,
    out_receiver: *mut *mut ipmb_Receiver,
) -> c_int {
    let Some(opts) = parse_options(&options) else { return -1 };

    let timeout = if timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(timeout_ms as u64))
    };

    match crate::ipc::join::<BytesMessage, BytesMessage>(opts, timeout) {
        Ok((sender, receiver)) => {
            *out_sender = Box::into_raw(Box::new(ipmb_Sender {
                inner: Box::into_raw(Box::new(sender)),
            }));
            *out_receiver = Box::into_raw(Box::new(ipmb_Receiver {
                inner: Box::into_raw(Box::new(receiver)),
            }));
            0
        }
        Err(_) => -1,
    }
}

/// Send a message on the bus. Takes ownership of message data.
///
/// # Safety
/// `sender` must be a valid pointer from `ipmb_join`.
/// `data` must point to `data_len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn ipmb_send(
    sender: *mut ipmb_Sender,
    format: u16,
    data: *const u8,
    data_len: u32,
) -> c_int {
    if sender.is_null() { return -1; }
    let sender_ref = &*(*sender).inner;
    let payload = BytesMessage {
        format,
        data: if data.is_null() {
            vec![]
        } else {
            std::slice::from_raw_parts(data, data_len as usize).to_vec()
        },
    };
    let msg = Message::new(Selector::multicast(LabelOp::True), payload);
    match sender_ref.send(msg) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Receive a message from the bus. Writes message handle to out pointer.
///
/// # Safety
/// `receiver` must be a valid pointer from `ipmb_join`.
#[no_mangle]
pub unsafe extern "C" fn ipmb_recv(
    receiver: *mut ipmb_Receiver,
    out_message: *mut *mut ipmb_Message,
    timeout_ms: u32,
) -> c_int {
    if receiver.is_null() || out_message.is_null() { return -1; }
    let receiver_ref = &mut *(*receiver).inner;
    let timeout = if timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(timeout_ms as u64))
    };

    match receiver_ref.recv(timeout) {
        Ok(msg) => {
            let mut data = msg.payload.data;
            let msg_box = Box::new(ipmb_Message {
                format: msg.payload.format,
                data_len: data.len() as u32,
                data: if data.is_empty() {
                    std::ptr::null_mut()
                } else {
                    let ptr = data.as_mut_ptr();
                    std::mem::forget(data);
                    ptr
                },
            });
            *out_message = Box::into_raw(msg_box);
            0
        }
        Err(_) => -1,
    }
}

/// Get the format field from a received message.
#[no_mangle]
pub unsafe extern "C" fn ipmb_message_format(msg: *const ipmb_Message) -> u16 {
    if msg.is_null() { return 0; }
    (*msg).format
}

/// Get a pointer to the message data and its length.
#[no_mangle]
pub unsafe extern "C" fn ipmb_message_data(msg: *const ipmb_Message, out_len: *mut u32) -> *const u8 {
    if msg.is_null() { return std::ptr::null(); }
    if !out_len.is_null() {
        *out_len = (*msg).data_len;
    }
    (*msg).data
}

/// Free a message returned by `ipmb_recv`.
#[no_mangle]
pub unsafe extern "C" fn ipmb_message_free(msg: *mut ipmb_Message) {
    if msg.is_null() { return; }
    let msg = Box::from_raw(msg);
    if !msg.data.is_null() && msg.data_len > 0 {
        drop(Vec::from_raw_parts(msg.data, msg.data_len as usize, msg.data_len as usize));
    }
}

/// Free a sender returned by `ipmb_join`.
#[no_mangle]
pub unsafe extern "C" fn ipmb_sender_free(sender: *mut ipmb_Sender) {
    if sender.is_null() { return; }
    let s = Box::from_raw(sender);
    drop(Box::from_raw(s.inner));
}

/// Free a receiver returned by `ipmb_join`.
#[no_mangle]
pub unsafe extern "C" fn ipmb_receiver_free(receiver: *mut ipmb_Receiver) {
    if receiver.is_null() { return; }
    let r = Box::from_raw(receiver);
    drop(Box::from_raw(r.inner));
}
