// Windows signal handling — SetConsoleCtrlHandler + event object.

use crate::signal::error::SignalError;
use crate::signal::event::SignalKind;
use crate::signal::handle::SignalHandle;
use crate::signal::note_signal;

/// Register console control handler.
///
/// # Errors
/// Returns an error if event creation or handler registration fails.
pub fn register_signals() -> Result<SignalHandle, SignalError> {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
    use windows_sys::Win32::System::Threading::CreateEventW;

    // Create auto-reset event
    let event = unsafe {
        CreateEventW(
            std::ptr::null_mut(),
            0, // auto-reset
            0, // initial state nonsignaled
            std::ptr::null(),
        )
    };
    if event.is_null() {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

    // Store the event handle so the console handler can signal it.
    GLOBAL_EVENT.store(event as u64, Ordering::Relaxed);

    // Install console handler
    let ok = unsafe { SetConsoleCtrlHandler(Some(console_handler), 1) };
    if ok == 0 {
        return Err(SignalError::RegistrationFailed(format!(
            "SetConsoleCtrlHandler failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    Ok(SignalHandle::Windows { event })
}

use windows_sys::Win32::System::Console::{
    CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
};
use windows_sys::Win32::System::Threading::SetEvent;

use std::sync::atomic::{AtomicU64, Ordering};

static GLOBAL_EVENT: AtomicU64 = AtomicU64::new(0);

/// Console control handler.
///
/// The handler records the signal and wakes the readiness poll. It does not
/// deliver to the bus, so the handler never locks or allocates.
unsafe extern "system" fn console_handler(ctrl_type: u32) -> i32 {
    let kind = match ctrl_type {
        CTRL_C_EVENT => SignalKind::Interrupt,
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => SignalKind::Terminate,
        _ => return 0,
    };

    // Record first: a poll that sees the event must find a set bit.
    note_signal(kind);

    let event = GLOBAL_EVENT.load(Ordering::Relaxed);
    if event != 0 {
        SetEvent(event as _);
    }

    1
}
