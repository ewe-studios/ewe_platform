use derive_more::{Display, Error, From};

/// Errors that can occur during file watching operations.
#[derive(Debug, Display, Error, From)]
pub enum WatchError {
    #[display("I/O error: {_0}")]
    Io(#[from] std::io::Error),

    #[display("path is not valid UTF-8: {_0:?}")]
    #[from(ignore)]
    InvalidPath(#[error(not(source))] std::path::PathBuf),

    #[display("watch limit reached: too many paths registered")]
    WatchLimit,

    #[display("path is not being watched")]
    NotWatched,

    #[display("the requested API is not available on this platform")]
    UnsupportedPlatform,

    #[display("all configured API backends failed")]
    AllBackendsFailed,
}

pub type Result<T> = std::result::Result<T, WatchError>;
