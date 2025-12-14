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
    File(FileInfo),
}

#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    pub index: Option<usize>,
    pub dir_name: String,
    pub root_dir: Option<String>,
    pub date_modified_since_unix_epoc: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
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

impl FileInfo {
    #[allow(clippy::too_many_arguments)]
    pub const fn create(
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

pub trait FileData {
    /// [`compression`] returns the compression used for the file data.
    fn compression(&self) -> DataCompression;

    /// [`read_utf8`] will return the data related to the File if its
    /// a file else returns None.
    fn read_utf8(&self) -> Option<Vec<u8>>;

    /// [`read_utf8_for`] will return the data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_utf8_for(&self, source: &str) -> Option<Vec<u8>>;

    /// [`read_utf16`] will return the UTF16 data related to the File if its
    /// a file else returns None.
    fn read_utf16(&self) -> Option<Vec<u8>>;

    /// [`read_utf16_for`] will return the UTF16 data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_utf16_for(&self, source: &str) -> Option<Vec<u8>>;
}

/// [`EmbeddableFile`] defines a trait definition in a no_std environment where
/// the underlying data of the file are brought into the source tree via direct
/// replication of the underlying bytes for a file and basic metadata (think:
/// date_modified, etag, and sha256 hash of file content).
pub trait EmbeddableFile: FileData {
    /// [`get_info`] returns the related information for the self
    /// implementation of FileData.
    fn get_info(&self) -> &FileInfo;

    /// [`info_for`] returns the related information for the file based on the provided
    /// source path string if it exists internal else returns None.
    fn info_for<'a>(&self, source: &'a str) -> Option<&'a FileInfo>;
}

#[derive(Debug, Clone)]
pub struct OwnedData(&'static [u8], &'static [u8], DataCompression);

impl OwnedData {
    pub const fn create(
        utf8_data: &'static [u8],
        utf16_data: &'static [u8],
        compression: DataCompression,
    ) -> Self {
        Self(utf8_data, utf16_data, compression)
    }

    pub fn new(
        utf8_data: &'static [u8],
        utf16_data: &'static [u8],
        compression: DataCompression,
    ) -> Self {
        Self(utf8_data, utf16_data, compression)
    }
}

impl FileData for OwnedData {
    fn compression(&self) -> DataCompression {
        self.2.clone()
    }

    fn read_utf8(&self) -> Option<Vec<u8>> {
        let mut data = Vec::with_capacity(self.0.len());
        data.extend_from_slice(self.0);
        Some(data)
    }

    fn read_utf16(&self) -> Option<Vec<u8>> {
        let mut data = Vec::with_capacity(self.1.len());
        data.extend_from_slice(self.1);
        Some(data)
    }

    fn read_utf8_for(&self, _: &str) -> Option<Vec<u8>> {
        None
    }

    fn read_utf16_for(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}
