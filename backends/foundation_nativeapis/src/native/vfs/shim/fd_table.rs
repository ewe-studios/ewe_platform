use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::Mutex;

use crate::shared::vfs::dynfs::ErasedFile;

/// Thread-safe virtual FD table.
/// Maps synthetic FD numbers (starting at 10000+) to type-erased VFS file handles.
pub struct VirtualFdTable {
    entries: Mutex<HashMap<c_int, FdEntry>>,
    next_fd: Mutex<c_int>,
}

struct FdEntry {
    path: String,
    offset: u64,
    flags: c_int,
    file: ErasedFile,
}

impl VirtualFdTable {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            next_fd: Mutex::new(10_000),
        }
    }

    /// Allocate a new virtual FD number.
    pub fn alloc_fd(&self) -> c_int {
        let mut next = self.next_fd.lock().unwrap();
        let fd = *next;
        *next += 1;
        fd
    }

    /// Insert a file handle at the given FD number.
    pub fn insert(&self, fd: c_int, path: String, file: ErasedFile, flags: c_int) {
        let entry = FdEntry {
            path,
            offset: 0,
            flags,
            file,
        };
        self.entries.lock().unwrap().insert(fd, entry);
    }

    /// Remove and drop an entry (closes the VFS file handle).
    pub fn remove(&self, fd: c_int) {
        self.entries.lock().unwrap().remove(&fd);
    }

    /// Get the path for a virtual FD.
    pub fn path(&self, fd: c_int) -> Option<String> {
        self.entries.lock().unwrap().get(&fd).map(|e| e.path.clone())
    }

    /// Read from a virtual FD.
    pub fn read(&self, fd: c_int, buf: &mut [u8]) -> Option<usize> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get_mut(&fd)?;
        let offset = entry.offset;
        let n = entry.file.read_at(buf, offset).ok()?;
        entry.offset += n as u64;
        Some(n)
    }

    /// Write to a virtual FD.
    pub fn write(&self, fd: c_int, buf: &[u8]) -> Option<usize> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get_mut(&fd)?;
        let offset = entry.offset;
        let n = entry.file.write_at(buf, offset).ok()?;
        entry.offset += n as u64;
        Some(n)
    }

    /// Read from a virtual FD at a specific offset (does not advance cursor).
    pub fn read_at(&self, fd: c_int, buf: &mut [u8], offset: u64) -> Option<usize> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(&fd)?;
        entry.file.read_at(buf, offset).ok()
    }

    /// Write to a virtual FD at a specific offset (does not advance cursor).
    pub fn write_at(&self, fd: c_int, buf: &[u8], offset: u64) -> Option<usize> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(&fd)?;
        entry.file.write_at(buf, offset).ok()
    }

    /// Get the offset for a virtual FD.
    pub fn get_offset(&self, fd: c_int) -> Option<u64> {
        self.entries.lock().unwrap().get(&fd).map(|e| e.offset)
    }

    /// Set the offset for a virtual FD.
    pub fn set_offset(&self, fd: c_int, offset: u64) {
        if let Some(e) = self.entries.lock().unwrap().get_mut(&fd) {
            e.offset = offset;
        }
    }
}
