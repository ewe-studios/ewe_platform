// Windows signal handling — SetConsoleCtrlHandler + event object.

use crate::signal::error::SignalError;
use crate::signal::event::SignalKind;
use crate::signal::handle::SignalHandle;

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
            0,            // auto-reset
            0,            // initial state nonsignaled
            std::ptr::null(),
        )
    };
    if event.0 == 0 {
        return Err(SignalError::Io(std::io::Error::last_os_error()));
    }

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
    CTRL_C_EVENT, CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
};
use windows_sys::Win32::System::Threading::SetEvent;

use std::sync::atomic::{AtomicU64, Ordering};

static GLOBAL_EVENT: AtomicU64 = AtomicU64::new(0);

/// Console control handler.
unsafe extern "system" fn console_handler(ctrl_type: u32) -> i32 {
    let kind = match ctrl_type {
        CTRL_C_EVENT => SignalKind::Interrupt,
        CTRL_CLOSE_EVENT => SignalKind::Terminate,
        CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => SignalKind::Terminate,
        _ => return 0,
    };

    let event = GLOBAL_EVENT.load(Ordering::Relaxed);
    if event != 0 {
        SetEvent(event as _);
    }

    1
}
