//! Connect each Rust child call to its browser view.
extern crate std;
use super::{take_request, BTreeMap, RefCell, Vec};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use core::cell::Cell;
use foundation_ui_runtime::action::{ActionError, ChildActionCaller};
use foundation_ui_wire::{
    writer::{write_messages, ClientCaps, FrameHeader, WriterConfig},
    ErrorCode, MessageT,
};

#[link(wasm_import_module = "env")]
extern "C" {
    fn ewe_ui_child(
        parent: u64,
        id: u64,
        name: *const u8,
        name_len: usize,
        json: *const u8,
        json_len: usize,
        preview: *const u8,
        preview_len: usize,
    );
}

pub(super) struct BrowserCaller(pub u64);
type Reply = Result<(), ActionError>;

std::thread_local! {
    static NEXT: Cell<u64> = const { Cell::new(0) };
    static WAITING: RefCell<BTreeMap<u64, async_channel::Sender<Reply>>> = const { RefCell::new(BTreeMap::new()) };
}

struct Pending(u64);
impl Drop for Pending {
    fn drop(&mut self) {
        WAITING.with_borrow_mut(|calls| calls.remove(&self.0));
    }
}

fn failure(message: impl Into<String>) -> ActionError {
    ActionError::new(ErrorCode::ActionFailed, message)
}

#[async_trait::async_trait(?Send)]
impl ChildActionCaller for BrowserCaller {
    async fn call(&self, reference: &str, json: Vec<u8>, preview: Vec<MessageT>) -> Reply {
        let bytes = if preview.is_empty() {
            Vec::new()
        } else {
            let header = FrameHeader {
                stream_id: 0,
                sequence_id: 0,
                ack: 0,
            };
            write_messages(
                preview,
                &ClientCaps::web_current(),
                header,
                &WriterConfig::default(),
            )
            .map_err(|error| failure(error.to_string()))?
            .0
        };
        let id = NEXT
            .with(|next| {
                let id = next.get().checked_add(1)?;
                next.set(id);
                Some(id)
            })
            .ok_or_else(|| failure("Child request identifiers exhausted"))?;
        let (send, receive) = async_channel::bounded(1);
        WAITING.with_borrow_mut(|calls| calls.insert(id, send));
        let _pending = Pending(id);
        // The browser copies all three slices before the import returns.
        unsafe {
            ewe_ui_child(
                self.0,
                id,
                reference.as_ptr(),
                reference.len(),
                json.as_ptr(),
                json.len(),
                bytes.as_ptr(),
                bytes.len(),
            );
        }
        receive
            .recv()
            .await
            .map_err(|_| failure("The browser action service closed"))?
    }
}

/// Complete one child call. Consume the error slot even when the caller has closed.
#[no_mangle]
pub extern "C" fn ewe_ui_child_done(id: u64, code: i32, slot: u64) {
    let result = if code < 0 {
        Ok(())
    } else {
        let message = take_request(slot)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_else(|| "Browser action failed".into());
        Err(ActionError::new(
            ErrorCode(u16::try_from(code).unwrap_or(ErrorCode::ActionFailed.0)),
            message,
        ))
    };
    if let Some(sender) = WAITING.with_borrow_mut(|calls| calls.remove(&id)) {
        // A cancelled action has already dropped its receiver.
        let _ = sender.try_send(result);
    }
}
