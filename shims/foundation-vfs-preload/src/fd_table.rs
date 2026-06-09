use std::collections::HashMap;
use std::os::raw::c_int;
use std::sync::Mutex;

pub struct VirtualFdEntry {
    pub path: String,
    pub offset: u64,
    pub flags: c_int,
}

pub struct VirtualFdTable {
    entries: Mutex<HashMap<c_int, VirtualFdEntry>>,
    next_fd: Mutex<c_int>,
}

impl VirtualFdTable {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            next_fd: Mutex::new(super::VIRTUAL_FD_BASE),
        }
    }

    pub fn insert(&self, path: String, flags: c_int) -> c_int {
        let mut next = self.next_fd.lock().unwrap();
        let fd = *next;
        *next += 1;

        let entry = VirtualFdEntry {
            path,
            offset: 0,
            flags,
        };
        self.entries.lock().unwrap().insert(fd, entry);
        fd
    }

    pub fn remove(&self, fd: c_int) -> Option<VirtualFdEntry> {
        self.entries.lock().unwrap().remove(&fd)
    }

    pub fn path(&self, fd: c_int) -> Option<String> {
        self.entries.lock().unwrap().get(&fd).map(|e| e.path.clone())
    }

    pub fn update_offset(&self, fd: c_int, offset: u64) -> bool {
        if let Some(entry) = self.entries.lock().unwrap().get_mut(&fd) {
            entry.offset = offset;
            true
        } else {
            false
        }
    }

    pub fn get_offset(&self, fd: c_int) -> Option<u64> {
        self.entries.lock().unwrap().get(&fd).map(|e| e.offset)
    }
}
