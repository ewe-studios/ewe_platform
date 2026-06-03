/// FFI layer — `extern "C"` bindings for the IPC bus.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use crate::ipc::message::BytesMessage;
use crate::ipc::options::Options;
use crate::ipc::label::Label;
use crate::ipc::errors::IpcError;

/// Opaque sender handle.
pub struct ipmb_Sender {
    _private: (),
}

/// Opaque receiver handle.
pub struct ipmb_Receiver {
    _private: (),
}

/// Opaque message handle.
pub struct ipmb_Message {
    _private: (),
}

#[repr(C)]
pub struct ipmb_Options {
    pub identifier: *const c_char,
    pub label: *const c_char,
    pub token: *const c_char,
    pub controller_affinity: c_int,
}

/// Join a message bus.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointers must be valid and properly aligned.
#[no_mangle]
pub unsafe extern "C" fn ipmb_join(
    options: ipmb_Options,
    _timeout_ms: u32,
    _out_sender: *mut *mut ipmb_Sender,
    _out_receiver: *mut *mut ipmb_Receiver,
) -> c_int {
    let identifier = match CStr::from_ptr(options.identifier).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let label = match CStr::from_ptr(options.label).to_str() {
        Ok(s) => Label::new(s),
        Err(_) => return -1,
    };
    let token = if options.token.is_null() {
        String::new()
    } else {
        match CStr::from_ptr(options.token).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return -1,
        }
    };

    let mut opts = Options::new(identifier, label).token(token);
    if options.controller_affinity != 0 {
        opts = opts.controller_affinity(true);
    }

    match crate::ipc::bus_controller::join::<BytesMessage, BytesMessage>(opts, None) {
        Ok((_sender, _receiver)) => {
            // TODO: Box the sender and receiver into raw pointers
            -1
        }
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn ipmb_send(
    _sender: *mut ipmb_Sender,
    _message: ipmb_Message,
) -> c_int {
    -1
}

#[no_mangle]
pub unsafe extern "C" fn ipmb_recv(
    _receiver: *mut ipmb_Receiver,
    _out_message: *mut *mut ipmb_Message,
    _timeout_ms: u32,
) -> c_int {
    -1
}

#[no_mangle]
pub unsafe extern "C" fn ipmb_message_free(_msg: *mut ipmb_Message) {}

#[no_mangle]
pub unsafe extern "C" fn ipmb_sender_free(_sender: *mut ipmb_Sender) {}

#[no_mangle]
pub unsafe extern "C" fn ipmb_receiver_free(_receiver: *mut ipmb_Receiver) {}
