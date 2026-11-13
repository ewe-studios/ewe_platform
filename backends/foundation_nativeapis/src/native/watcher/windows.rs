//! Windows `ReadDirectoryChangesW` file watcher driven by an I/O completion port.
//!
//! # Design
//!
//! - One I/O completion port (IOCP) belongs to each [`WinWatcher`]. The
//!   reactor's Windows selector serves sockets and reports readiness. It does
//!   not return the raw `OVERLAPPED` pointers that this code needs. For that
//!   reason this module calls `CreateIoCompletionPort` directly.
//! - Each watched directory has a heap-pinned `WatchState`. It holds the
//!   `OVERLAPPED` value and the change buffer. The completion key of the
//!   directory handle is the watch identifier.
//! - `poll` and `has_events` call `GetQueuedCompletionStatusEx` with the
//!   timeout the caller gives. A zero timeout does not wait.
//! - `unwatch` and `Drop` cancel the read and wait for its completion packet.
//!   Only then do they close the handle and free the state. The kernel can
//!   not write into freed memory.

use std::collections::{HashMap, VecDeque};
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{io, mem, ptr};

use crate::shared::error::{Result, WatchError};
use crate::shared::event::{WatchEvent, WatchEventKind};
use crate::shared::watcher::NativeWatcher;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_NOTIFY_ENUM_DIR, ERROR_OPERATION_ABORTED, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadDirectoryChangesW, FILE_ACTION_ADDED, FILE_ACTION_REMOVED,
    FILE_ACTION_RENAMED_NEW_NAME, FILE_ACTION_RENAMED_OLD_NAME, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::{
    CancelIoEx, CreateIoCompletionPort, GetOverlappedResult, GetQueuedCompletionStatusEx,
    OVERLAPPED, OVERLAPPED_ENTRY,
};

/// The change kinds that the watcher asks the kernel to report.
const NOTIFY_FILTER: u32 = FILE_NOTIFY_CHANGE_FILE_NAME
    | FILE_NOTIFY_CHANGE_DIR_NAME
    | FILE_NOTIFY_CHANGE_SIZE
    | FILE_NOTIFY_CHANGE_LAST_WRITE;

/// Size of the fixed part of `FILE_NOTIFY_INFORMATION`: three `u32` fields.
const RECORD_HEADER_LEN: usize = 12;

/// The smallest change buffer that the watcher accepts, in bytes.
const MIN_BUFFER_SIZE: u32 = 64;

/// Settings of a [`WinWatcher`].
///
/// The defaults follow the Windows documentation. A read of a network share
/// can not use a buffer larger than 64 KiB.
#[derive(Debug, Clone)]
pub struct WinWatcherConfig {
    /// Size in bytes of the change buffer of each watch. Default: 64 KiB.
    ///
    /// 64 KiB is the largest size that `ReadDirectoryChangesW` allows for a
    /// network share. A small buffer overflows sooner. On overflow the watcher
    /// emits a [`WatchEventKind::Rescan`] event.
    pub buffer_size: u32,
    /// How long `unwatch` and `Drop` wait for a cancelled read to finish.
    /// Default: 5 seconds.
    ///
    /// If the wait ends first, the watcher keeps the memory of that read
    /// allocated (it leaks it) because the kernel can still write into it.
    pub unwatch_timeout: Duration,
    /// How many completion packets one `GetQueuedCompletionStatusEx` call
    /// takes. Default: 64, the batch size that libuv uses for its completion
    /// port loop.
    pub completion_batch: usize,
}

impl Default for WinWatcherConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,
            unwatch_timeout: Duration::from_secs(5),
            completion_batch: 64,
        }
    }
}

impl WinWatcherConfig {
    /// Set the size in bytes of the change buffer of each watch.
    #[must_use]
    pub fn buffer_size(mut self, bytes: u32) -> Self {
        self.buffer_size = bytes;
        self
    }

    /// Set how long `unwatch` waits for a cancelled read to finish.
    #[must_use]
    pub fn unwatch_timeout(mut self, timeout: Duration) -> Self {
        self.unwatch_timeout = timeout;
        self
    }

    /// Set how many completion packets one wait takes (at least 1).
    #[must_use]
    pub fn completion_batch(mut self, packets: usize) -> Self {
        self.completion_batch = packets;
        self
    }
}

/// Per-watch state. It lives in a `Box`, so its address stays fixed while the
/// kernel owns the `OVERLAPPED` value and the buffer. `repr(C)` keeps
/// `overlapped` as the first field, as its comment says.
#[repr(C)]
struct WatchState {
    /// The `OVERLAPPED` value of the pending read. It must stay first so that
    /// its address is also the address of the state.
    overlapped: OVERLAPPED,
    /// Change buffer. `u32` elements give the 4-byte alignment that the kernel needs.
    buffer: Box<[u32]>,
    /// Directory handle.
    dir_handle: HANDLE,
    /// The watched directory.
    root: PathBuf,
    /// Whether the watch covers the sub-tree.
    recursive: bool,
    /// True while the kernel owns `overlapped` and `buffer`.
    pending: bool,
    /// Life cycle of the watch.
    phase: Phase,
}

/// Life cycle of one watch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The watch reads changes.
    Active,
    /// `unwatch` started: completions are dropped.
    Closing,
    /// The read ended or could not be issued again. The watch gives no events.
    Failed,
}

// SAFETY: The directory handle is an owned kernel handle, not tied to a thread.
// The buffer and the OVERLAPPED value are used only through `&mut WinWatcher`.
unsafe impl Send for WatchState {}
unsafe impl Sync for WatchState {}

impl WatchState {
    /// Start a `ReadDirectoryChangesW` call on this watch.
    fn issue_read(&mut self) -> io::Result<()> {
        // SAFETY: A zeroed OVERLAPPED is valid. No read is pending here.
        self.overlapped = unsafe { mem::zeroed() };
        let buf = self.buffer.as_mut_ptr().cast::<c_void>();
        // The size comes from a `u32` config field, so it always fits.
        let len = u32::try_from(self.buffer.len() * mem::size_of::<u32>()).unwrap_or(u32::MAX);
        let overlapped = ptr::addr_of_mut!(self.overlapped);
        // SAFETY: `buf` and `overlapped` point into this boxed state. They stay
        // valid until the completion packet is dequeued (see `shutdown_watch`).
        let ok = unsafe {
            ReadDirectoryChangesW(
                self.dir_handle,
                buf,
                len,
                i32::from(self.recursive),
                NOTIFY_FILTER,
                ptr::null_mut(),
                overlapped,
                None,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        self.pending = true;
        Ok(())
    }
}

/// Windows IOCP-based file watcher.
pub struct WinWatcher {
    /// The completion port that all directory handles use.
    iocp: HANDLE,
    /// Watches by identifier (the completion key).
    watches: HashMap<usize, Box<WatchState>>,
    /// Watch identifier by path.
    by_path: HashMap<PathBuf, usize>,
    /// The next watch identifier.
    next_key: usize,
    /// Events that `has_events` read and `poll` has not returned yet.
    event_cache: VecDeque<WatchEvent>,
    /// Settings.
    config: WinWatcherConfig,
}

// SAFETY: The completion port handle is an owned kernel handle. All other
// state is plain data behind `&mut self`.
unsafe impl Send for WinWatcher {}
unsafe impl Sync for WinWatcher {}

/// Convert a timeout to whole milliseconds, rounded up, below `INFINITE`.
fn timeout_ms(timeout: Duration) -> u32 {
    let ms = timeout.as_nanos().div_ceil(1_000_000);
    u32::try_from(ms.min(u128::from(u32::MAX - 1))).unwrap_or(u32::MAX - 1)
}

/// Decode `FILE_NOTIFY_INFORMATION` records into `(action, name)` pairs.
///
/// The function reads bytes with `from_ne_bytes`, so alignment does not matter.
/// It checks every offset and length against the buffer. A bad record stops
/// the decode and the function keeps the records read so far.
fn decode_records(buf: &[u8]) -> Vec<(u32, OsString)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(header) = buf.get(offset..offset.saturating_add(RECORD_HEADER_LEN)) {
        let word =
            |i: usize| u32::from_ne_bytes([header[i], header[i + 1], header[i + 2], header[i + 3]]);
        let next = word(0) as usize;
        let action = word(4);
        let name_len = word(8) as usize;
        let name_start = offset + RECORD_HEADER_LEN;
        let Some(name_bytes) = buf.get(name_start..name_start.saturating_add(name_len)) else {
            tracing::warn!("WinWatcher: record name exceeds the buffer");
            break;
        };
        if !name_len.is_multiple_of(2) {
            tracing::warn!("WinWatcher: record name has an odd byte length");
            break;
        }
        let (pairs, _) = name_bytes.as_chunks::<2>();
        let wide: Vec<u16> = pairs.iter().map(|c| u16::from_ne_bytes(*c)).collect();
        out.push((action, OsString::from_wide(&wide)));

        if next == 0 {
            break;
        }
        if next < RECORD_HEADER_LEN || offset.saturating_add(next) >= buf.len() {
            tracing::warn!("WinWatcher: bad NextEntryOffset {next}");
            break;
        }
        offset += next;
    }
    out
}

/// Turn decoded records into events. A rename old/new pair becomes one event.
fn records_to_events(root: &Path, records: Vec<(u32, OsString)>, out: &mut VecDeque<WatchEvent>) {
    let mut old_name: Option<PathBuf> = None;
    let flush_old = |old: &mut Option<PathBuf>, out: &mut VecDeque<WatchEvent>| {
        if let Some(path) = old.take() {
            out.push_back(WatchEvent {
                kind: WatchEventKind::Removed,
                path,
            });
        }
    };
    for (action, name) in records {
        let path = if name.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&name)
        };
        match action {
            FILE_ACTION_RENAMED_OLD_NAME => {
                flush_old(&mut old_name, out);
                old_name = Some(path);
            }
            FILE_ACTION_RENAMED_NEW_NAME => match old_name.take() {
                Some(from) => out.push_back(WatchEvent {
                    kind: WatchEventKind::Renamed {
                        from,
                        to: path.clone(),
                    },
                    path,
                }),
                None => out.push_back(WatchEvent {
                    kind: WatchEventKind::Created,
                    path,
                }),
            },
            other => {
                flush_old(&mut old_name, out);
                let kind = match other {
                    FILE_ACTION_ADDED => WatchEventKind::Created,
                    FILE_ACTION_REMOVED => WatchEventKind::Removed,
                    _ => WatchEventKind::Modified,
                };
                out.push_back(WatchEvent { kind, path });
            }
        }
    }
    // A lone old name has no partner in this buffer: the file left the tree.
    flush_old(&mut old_name, out);
}

impl WinWatcher {
    /// Create a new `WinWatcher` with the default settings.
    ///
    /// # Errors
    /// Returns an error if the completion port cannot be created.
    pub fn new() -> Result<Self> {
        Self::with_config(WinWatcherConfig::default())
    }

    /// Create a new `WinWatcher` with the given settings.
    ///
    /// # Errors
    /// Returns an error if the settings are invalid or the completion port
    /// cannot be created.
    pub fn with_config(config: WinWatcherConfig) -> Result<Self> {
        if config.buffer_size < MIN_BUFFER_SIZE {
            return Err(WatchError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("buffer_size must be at least {MIN_BUFFER_SIZE} bytes"),
            )));
        }
        // SAFETY: Creating a new port with no handle attached takes no pointers.
        let iocp = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, ptr::null_mut(), 0, 0) };
        if iocp.is_null() {
            return Err(WatchError::Io(io::Error::last_os_error()));
        }
        Ok(Self {
            iocp,
            watches: HashMap::new(),
            by_path: HashMap::new(),
            next_key: 1,
            event_cache: VecDeque::new(),
            config,
        })
    }

    /// Open a directory handle for file change notifications.
    fn open_directory(path: &Path) -> io::Result<HANDLE> {
        let path_wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: `path_wide` is NUL-terminated and outlives the call.
        let handle = unsafe {
            CreateFileW(
                path_wide.as_ptr(),
                FILE_LIST_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(handle)
        }
    }

    /// Wait up to `timeout` for completion packets and handle them.
    ///
    /// Returns the number of packets handled. Zero means the wait timed out.
    fn dequeue(&mut self, timeout: Duration) -> io::Result<usize> {
        let batch = self.config.completion_batch.max(1);
        // SAFETY: A zeroed OVERLAPPED_ENTRY is a valid output buffer.
        let mut entries: Vec<OVERLAPPED_ENTRY> = vec![unsafe { mem::zeroed() }; batch];
        let mut removed: u32 = 0;
        // SAFETY: `entries` has `batch` slots and `removed` is a valid output.
        let ok = unsafe {
            GetQueuedCompletionStatusEx(
                self.iocp,
                entries.as_mut_ptr(),
                u32::try_from(batch).unwrap_or(u32::MAX),
                ptr::addr_of_mut!(removed),
                timeout_ms(timeout),
                0,
            )
        };
        if ok == 0 {
            // SAFETY: Reads the thread error code only.
            let code = unsafe { GetLastError() };
            if code == WAIT_TIMEOUT {
                return Ok(0);
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        for entry in &entries[..removed as usize] {
            self.handle_completion(entry.lpCompletionKey, entry.lpOverlapped);
        }
        Ok(removed as usize)
    }

    /// Handle one completion packet: decode it and issue the next read.
    fn handle_completion(&mut self, key: usize, overlapped: *mut OVERLAPPED) {
        let Self {
            watches,
            event_cache,
            ..
        } = self;
        let Some(state) = watches.get_mut(&key) else {
            return;
        };
        if !ptr::eq(overlapped, ptr::addr_of_mut!(state.overlapped)) {
            return;
        }
        state.pending = false;
        if state.phase == Phase::Closing {
            return;
        }

        let mut bytes: u32 = 0;
        // SAFETY: The handle is open and `overlapped` is this state's own
        // OVERLAPPED. The operation is complete, so the call does not wait.
        let ok = unsafe { GetOverlappedResult(state.dir_handle, overlapped, &mut bytes, 0) };
        let code = if ok == 0 {
            // SAFETY: Reads the thread error code only.
            unsafe { GetLastError() }
        } else {
            0
        };

        if code == ERROR_OPERATION_ABORTED {
            state.phase = Phase::Failed;
            return;
        }
        if code == ERROR_NOTIFY_ENUM_DIR || (code == 0 && bytes == 0) {
            // The kernel lost changes (buffer full): ask the caller to rescan.
            event_cache.push_back(WatchEvent {
                kind: WatchEventKind::Rescan,
                path: state.root.clone(),
            });
        } else if code != 0 {
            tracing::warn!(
                "WinWatcher: read on {:?} failed: {}",
                state.root,
                io::Error::from_raw_os_error(code as i32)
            );
            state.phase = Phase::Failed;
            return;
        } else {
            let len = (bytes as usize).min(state.buffer.len() * mem::size_of::<u32>());
            // SAFETY: The buffer holds at least `len` initialised bytes and the
            // kernel no longer writes to it because the read is complete.
            let raw =
                unsafe { std::slice::from_raw_parts(state.buffer.as_ptr().cast::<u8>(), len) };
            records_to_events(&state.root, decode_records(raw), event_cache);
        }

        if let Err(err) = state.issue_read() {
            tracing::warn!(
                "WinWatcher: cannot re-issue read on {:?}: {err}",
                state.root
            );
            state.phase = Phase::Failed;
        }
    }

    /// Cancel the read of one watch, wait for its packet, then free it.
    ///
    /// If the packet does not arrive within `unwatch_timeout`, the watch memory
    /// and handle are leaked on purpose: the kernel can still write to them.
    fn shutdown_watch(&mut self, key: usize) -> io::Result<()> {
        let Some(state) = self.watches.get_mut(&key) else {
            return Ok(());
        };
        state.phase = Phase::Closing;
        if state.pending {
            let overlapped = ptr::addr_of_mut!(state.overlapped);
            // SAFETY: The handle is open and `overlapped` belongs to this watch.
            // The call fails with ERROR_NOT_FOUND if the read already finished;
            // that packet is then already in the port.
            unsafe { CancelIoEx(state.dir_handle, overlapped) };
        }
        let timeout = self.config.unwatch_timeout;
        while self.watches.get(&key).is_some_and(|s| s.pending) {
            if self.dequeue(timeout)? == 0 {
                if let Some(state) = self.watches.remove(&key) {
                    mem::forget(state);
                }
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the cancelled read did not finish in time",
                ));
            }
        }
        if let Some(state) = self.watches.remove(&key) {
            // SAFETY: The completion packet came back, so the kernel is done
            // with the buffer and the OVERLAPPED value. The handle is open.
            unsafe { CloseHandle(state.dir_handle) };
        }
        Ok(())
    }
}

impl NativeWatcher for WinWatcher {
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()> {
        if self.by_path.contains_key(path) {
            self.unwatch(path)?;
        }
        let dir_handle = Self::open_directory(path)?;
        let key = self.next_key;
        self.next_key += 1;

        // SAFETY: Both handles are open. The key identifies the watch.
        let port = unsafe { CreateIoCompletionPort(dir_handle, self.iocp, key, 0) };
        if port.is_null() {
            let err = io::Error::last_os_error();
            // SAFETY: The handle is open and no read uses it yet.
            unsafe { CloseHandle(dir_handle) };
            return Err(err.into());
        }

        let words = (self.config.buffer_size as usize).div_ceil(mem::size_of::<u32>());
        let mut state = Box::new(WatchState {
            // SAFETY: A zeroed OVERLAPPED is valid.
            overlapped: unsafe { mem::zeroed() },
            buffer: vec![0u32; words].into_boxed_slice(),
            dir_handle,
            root: path.to_path_buf(),
            recursive,
            pending: false,
            phase: Phase::Active,
        });
        if let Err(err) = state.issue_read() {
            // SAFETY: The read failed to start, so the handle is idle.
            unsafe { CloseHandle(dir_handle) };
            return Err(err.into());
        }
        self.watches.insert(key, state);
        self.by_path.insert(path.to_path_buf(), key);
        Ok(())
    }

    fn unwatch(&mut self, path: &Path) -> Result<()> {
        let key = self.by_path.remove(path).ok_or(WatchError::NotWatched)?;
        self.shutdown_watch(key)?;
        Ok(())
    }

    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>> {
        if self.event_cache.is_empty() {
            self.dequeue(timeout)?;
        }
        Ok(self.event_cache.drain(..).collect())
    }

    fn clear(&mut self) -> Result<()> {
        let keys: Vec<usize> = self.watches.keys().copied().collect();
        self.by_path.clear();
        let mut first_error = None;
        for key in keys {
            if let Err(err) = self.shutdown_watch(key) {
                first_error.get_or_insert(err);
            }
        }
        match first_error {
            Some(err) => Err(err.into()),
            None => Ok(()),
        }
    }

    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        if self.event_cache.is_empty() {
            if let Err(err) = self.dequeue(timeout.unwrap_or(Duration::ZERO)) {
                tracing::warn!("WinWatcher: has_events failed: {err}");
            }
        }
        !self.event_cache.is_empty()
    }
}

impl Drop for WinWatcher {
    fn drop(&mut self) {
        if let Err(err) = self.clear() {
            tracing::warn!("WinWatcher: drop could not release every watch: {err}");
        }
        // SAFETY: The port handle is open and no watch uses it any more
        // (a leaked watch keeps its own memory alive).
        unsafe { CloseHandle(self.iocp) };
    }
}
