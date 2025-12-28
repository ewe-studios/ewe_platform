extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub enum DataCompression {
    NONE,
    GZIP,
    BROTTLI,
}

#[derive(Debug, Clone)]
pub enum FsInfo {
    Dir(DirectoryInfo),
    File(OwnedFileInfo),
}

#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    pub index: Option<usize>,
    pub dir_name: String,
    pub root_dir: Option<String>,
    pub date_modified_since_unix_epoc: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct OwnedFileInfo {
    pub index: Option<usize>,
    pub source_file_path: String,
    pub source_name: String,
    pub source_path: String,
    pub package_directory: String,
    pub e_tag: String,
    pub hash: String,
    pub mime_type: Option<String>,
    pub date_modified_since_unix_epoc: Option<i64>,
}

impl OwnedFileInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index: Option<usize>,
        source_file_path: String,
        source_name: String,
        source_path: String,
        package_directory: String,
        hash: String,
        e_tag: String,
        mime_type: Option<String>,
        date_modified: Option<i64>,
    ) -> Self {
        Self {
            date_modified_since_unix_epoc: date_modified,
            source_file_path,
            package_directory,
            source_name,
            source_path,
            index,
            e_tag,
            hash,
            mime_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub index: Option<usize>,
    pub source_file_path: &'static str,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub package_directory: &'static str,
    pub e_tag: &'static str,
    pub hash: &'static str,
    pub mime_type: Option<&'static str>,
    pub date_modified_since_unix_epoc: Option<i64>,
}

impl FileInfo {
    #[allow(clippy::too_many_arguments)]
    pub const fn create(
        index: Option<usize>,
        source_file_path: &'static str,
        source_name: &'static str,
        source_path: &'static str,
        package_directory: &'static str,
        hash: &'static str,
        e_tag: &'static str,
        mime_type: Option<&'static str>,
        date_modified: Option<i64>,
    ) -> Self {
        Self {
            source_file_path,
            date_modified_since_unix_epoc: date_modified,
            package_directory,
            source_name,
            source_path,
            index,
            e_tag,
            hash,
            mime_type,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index: Option<usize>,
        source_file_path: &'static str,
        source_name: &'static str,
        source_path: &'static str,
        package_directory: &'static str,
        hash: &'static str,
        e_tag: &'static str,
        mime_type: Option<&'static str>,
        date_modified: Option<i64>,
    ) -> Self {
        Self {
            date_modified_since_unix_epoc: date_modified,
            source_file_path,
            package_directory,
            source_name,
            source_path,
            index,
            e_tag,
            hash,
            mime_type,
        }
    }
}

pub trait HasCompression {
    /// [`compression`] returns the compression used for the file data.
    fn compression(&self) -> DataCompression;
}

pub trait FileData: HasCompression {
    /// [`read_utf8`] will return the data related to the File if its
    /// a file else returns None.
    fn read_utf8(&self) -> Option<Vec<u8>>;

    /// [`read_utf16`] will return the UTF16 data related to the File if its
    /// a file else returns None.
    fn read_utf16(&self) -> Option<Vec<u8>>;
}

/// [`EmbeddableFile`] defines a trait definition in a no_std environment where
/// the underlying data of the file are brought into the source tree via direct
/// replication of the underlying bytes for a file and basic metadata (think:
/// date_modified, etag, and sha256 hash of file content).
pub trait EmbeddableFile: FileData {
    /// [`get_info`] returns the related information for the self
    /// implementation of FileData.
    fn info(&self) -> &FileInfo;
}

pub trait DirectoryData: HasCompression {
    /// [`read_utf8_for`] will return the data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_utf8_for(&self, source: &str) -> Option<Vec<u8>>;

    /// [`read_utf16_for`] will return the UTF16 data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_utf16_for(&self, source: &str) -> Option<Vec<u8>>;
}

pub trait EmbeddableDirectory: DirectoryData {
    /// [`info_for`] returns the related information for the file based on the provided
    /// source path string if it exists internal else returns None.
    fn info_for(&self, source: &str) -> Option<FileInfo>;
}
