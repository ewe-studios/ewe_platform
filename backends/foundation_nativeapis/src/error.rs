use thiserror::Error;

/// Errors that can occur during file watching operations.
#[derive(Error, Debug)]
pub enum WatchError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("path is not valid UTF-8: {0:?}")]
    InvalidPath(std::path::PathBuf),

    #[error("watch limit reached: too many paths registered")]
    WatchLimit,

    #[error("path is not being watched")]
    NotWatched,

    #[error("the requested API is not available on this platform")]
    UnsupportedPlatform,

    #[error("all configured API backends failed")]
    AllBackendsFailed,
}

pub type Result<T> = std::result::Result<T, WatchError>;
