extern crate alloc;

use alloc::vec::Vec;

pub enum DataCompression {
    NONE,
    GZIP,
    BROTTLI,
}

pub struct FileInfo<'a> {
    pub source_file_path: &'a str,
    pub source_name: &'a str,
    pub source_path: &'a str,
    pub root_dir: &'a str,
    pub e_tag: &'a str,
    pub hash: &'a str,
    pub mime_type: Option<&'a str>,
    pub date_modified_since_unix_epoc: Option<i64>,
}

impl<'a> FileInfo<'a> {
    #[allow(clippy::too_many_arguments)]
    pub const fn create(
        source_file_path: &'a str,
        source_name: &'a str,
        source_path: &'a str,
        root_dir: &'a str,
        hash: &'a str,
        e_tag: &'a str,
        mime_type: Option<&'a str>,
        date_modified: Option<i64>,
    ) -> Self {
        Self {
            source_file_path,
            date_modified_since_unix_epoc: date_modified,
            source_name,
            source_path,
            root_dir,
            e_tag,
            hash,
            mime_type,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_file_path: &'a str,
        source_name: &'a str,
        source_path: &'a str,
        root_dir: &'a str,
        hash: &'a str,
        e_tag: &'a str,
        mime_type: Option<&'a str>,
        date_modified: Option<i64>,
    ) -> Self {
        Self {
            source_file_path,
            date_modified_since_unix_epoc: date_modified,
            source_name,
            source_path,
            root_dir,
            e_tag,
            hash,
            mime_type,
        }
    }
}

pub trait FileData {
    /// [`read_u8`] will return the data related to the File if its
    /// a file else returns None.
    fn read_u8(&self) -> Option<Vec<u8>>;

    /// [`read_u8_for`] will return the data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_u8_for(&self, source: &str) -> Option<Vec<u8>>;

    /// [`read_u16`] will return the UTF16 data related to the File if its
    /// a file else returns None.
    fn read_u16(&self) -> Option<Vec<u16>>;

    /// [`read_u16_for`] will return the UTF16 data related to the File
    /// pointed to by source path str pointer the if its
    /// a file else returns None.
    fn read_u16_for(&self, source: &str) -> Option<Vec<u16>>;
}

/// [`EmbeddableFile`] defines a trait definition in a no_std environment where
/// the underlying data of the file are brought into the source tree via direct
/// replication of the underlying bytes for a file and basic metadata (think:
/// date_modified, etag, and sha256 hash of file content).
pub trait EmbeddableFile: FileData {
    /// [`get_info`] returns the related information for the self
    /// implementation of FileData.
    fn get_info<'a>(&'a self) -> FileInfo<'a>;

    /// [`info_for`] returns the related information for the file based on the provided
    /// source path string if it exists internal else returns None.
    fn info_for<'a>(&self, source: &'a str) -> Option<FileInfo<'a>>;
}

pub struct OwnedData(&'static [u8], &'static [u16]);

impl OwnedData {
    pub const fn create(u8_data: &'static [u8], u16_data: &'static [u16]) -> Self {
        Self(u8_data, u16_data)
    }

    pub fn new(u8_data: &'static [u8], u16_data: &'static [u16]) -> Self {
        Self(u8_data, u16_data)
    }
}

impl FileData for OwnedData {
    fn read_u8(&self) -> Option<Vec<u8>> {
        let mut data = Vec::with_capacity(self.0.len());
        data.extend_from_slice(self.0);
        Some(data)
    }

    fn read_u16(&self) -> Option<Vec<u16>> {
        let mut data = Vec::with_capacity(self.1.len());
        data.extend_from_slice(self.1);
        Some(data)
    }

    fn read_u8_for(&self, _: &str) -> Option<Vec<u8>> {
        None
    }

    fn read_u16_for(&self, _: &str) -> Option<Vec<u16>> {
        None
    }
}
