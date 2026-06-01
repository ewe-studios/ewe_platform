use std::path::PathBuf;

/// A single file system event from any platform backend.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// What kind of change occurred.
    pub kind: WatchEventKind,
    /// The path that changed.
    pub path: PathBuf,
}

/// The type of file system change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    /// A file or directory was created.
    Created,
    /// A file or directory was modified (content or metadata).
    Modified,
    /// A file or directory was removed.
    Removed,
    /// A file was renamed (from old path, to new path).
    Renamed { from: PathBuf, to: PathBuf },
}
