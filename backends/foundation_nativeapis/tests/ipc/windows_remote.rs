//! Tests for `Remote::is_dead` on Windows (issue #488).
//!
//! `Remote` is public as `ipc::platform::windows::Remote`, so these tests call it directly.
//! They use a named pipe whose server end is in the test process. Each test opens a message-mode
//! pipe with read and write access on both ends.
//!
//! The suite builds on Windows only. It cannot run on Linux or macOS.

use std::ptr;

use foundation_nativeapis::ipc::platform::windows::{Handle, Remote};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, OPEN_EXISTING, PIPE_ACCESS_DUPLEX},
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
    },
};

/// A connected pair of pipe ends in message mode.
struct PipePair {
    /// The server end. The test closes it to simulate a peer that went away.
    server: HANDLE,
    /// The client end, wrapped as a `Remote`.
    remote: Remote,
    /// The raw client handle. `remote` owns it, so the test only borrows it.
    client_raw: HANDLE,
}

/// Create a pipe with a unique name and connect both ends.
fn connect_pair() -> PipePair {
    let name: Vec<u16> = format!(
        "\\\\.\\pipe\\ewe-nativeapis-is-dead-{}",
        uuid::Uuid::new_v4()
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();

    let server = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            4096,
            4096,
            0,
            ptr::null(),
        )
    };
    assert_ne!(server, INVALID_HANDLE_VALUE, "the test pipe is created");

    let client = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    assert_ne!(client, INVALID_HANDLE_VALUE, "the client opens the pipe");

    // The client is already connected, so this call reports `ERROR_PIPE_CONNECTED`.
    // That result is expected, so the test ignores it.
    unsafe {
        ConnectNamedPipe(server, ptr::null_mut());
    }

    let remote = Remote::new(unsafe { Handle::from_raw(client as isize) });
    PipePair {
        server,
        remote,
        client_raw: client,
    }
}

#[test]
fn is_dead_is_false_while_the_peer_is_open() {
    let pair = connect_pair();

    assert!(
        !pair.remote.is_dead(),
        "an open pipe with a live peer process is not dead"
    );

    unsafe {
        CloseHandle(pair.server);
    }
}

#[test]
fn is_dead_is_true_after_the_peer_closes() {
    let pair = connect_pair();
    assert!(!pair.remote.is_dead(), "the peer is open at the start");

    // Close the peer end. The pipe is now broken.
    unsafe {
        CloseHandle(pair.server);
    }

    assert!(
        pair.remote.is_dead(),
        "a pipe with a closed peer end is dead"
    );
}

#[test]
fn is_dead_does_not_write_to_the_pipe() {
    let pair = connect_pair();

    // The old probe wrote zero bytes, and that put an empty message in the pipe.
    assert!(!pair.remote.is_dead());
    assert!(!pair.remote.is_dead());

    // The first message that the server reads must be the one that the client sends now.
    let payload = b"abc";
    let mut written = 0u32;
    let wrote = unsafe {
        WriteFile(
            pair.client_raw,
            payload.as_ptr().cast(),
            3,
            &raw mut written,
            ptr::null_mut(),
        )
    };
    assert_ne!(wrote, 0, "the client writes the payload");

    let mut buf = [0u8; 16];
    let mut read = 0u32;
    let got = unsafe {
        ReadFile(
            pair.server,
            buf.as_mut_ptr().cast(),
            16,
            &raw mut read,
            ptr::null_mut(),
        )
    };
    assert_ne!(got, 0, "the server reads a message");
    assert_eq!(
        &buf[..read as usize],
        payload,
        "the first message is the payload, with no empty message before it"
    );

    unsafe {
        CloseHandle(pair.server);
    }
}
