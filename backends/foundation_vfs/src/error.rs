use derive_more::{Display, Error};
use foundation_errstacks::ErrorTrace;

#[derive(Debug, Display, Error)]
pub enum VfsError {
    #[display("not found: {path}")]
    NotFound { path: String },

    #[display("already exists: {path}")]
    AlreadyExists { path: String },

    #[display("permission denied: {path}")]
    PermissionDenied { path: String },

    #[display("not a file: {path}")]
    NotAFile { path: String },

    #[display("not a directory: {path}")]
    NotADirectory { path: String },

    #[display("operation not supported: {operation}")]
    Unsupported { operation: String },

    #[display("I/O error: {source}")]
    Io {
        #[error(source)]
        source: std::io::Error,
    },

    #[display("invalid path: {path}")]
    InvalidPath { path: String },

    #[display("filesystem is read-only")]
    ReadOnly,

    #[display("entry is pending (CoW in progress): {path}")]
    EntryPending { path: String },

    #[display("too many symlink hops: {path}")]
    SymlinkLoop { path: String },

    #[display("directory not empty: {path}")]
    DirectoryNotEmpty { path: String },

    #[display("not a symlink: {path}")]
    NotASymlink { path: String },

    #[display("backend error: {message}")]
    Backend { message: String },
}

pub type VfsResult<T> = Result<T, ErrorTrace<VfsError>>;

impl From<VfsError> for ErrorTrace<VfsError> {
    fn from(e: VfsError) -> Self {
        ErrorTrace::new(e)
    }
}
