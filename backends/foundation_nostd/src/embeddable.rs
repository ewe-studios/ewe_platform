/// [`EmbeddableFile`] defines a trait definition in a no_std environment where
/// the underlying data of the file are brought into the source tree via direct
/// replication of the underlying bytes for a file and basic metadata (think:
/// date_modified, etag, and sha256 hash of file content).
pub trait EmbeddableFile {
    const DATE_MODIFIED_SINCE_UNIX_EPOC: Option<i64>;
    const MIME_TYPE: Option<&str>;
    const SOURCE_FILE: &str;
    const SOURCE_PATH: &str;
    const ROOT_DIR: &str;
    const ETAG: &str;
    const HASH: &str;
    const UTF8: &[u8];
    const UTF16: &[u16];

    /// [`utf8_slice`] returns the provided utf-8 byte slices of the file as is
    /// read from file which uses the endiancess of the native system
    /// when compiled by rust.
    fn utf8_slice() -> &'static [u8] {
        Self::UTF8
    }

    /// [`utf16_slice`] returns the provided utf-16 byte slices of the file as is
    /// read from file which uses the endiancess of the native system
    /// when compiled by rust.
    fn utf16_slice() -> &'static [u16] {
        Self::UTF16
    }

    /// [`date_modified`] returns the last known date-time modification
    /// date given in UNIX timestamp.
    fn date_modified() -> Option<&'static i64> {
        Self::DATE_MODIFIED_SINCE_UNIX_EPOC.as_ref()
    }

    /// [`mime_type`] returns the suggested mime-type for the file based on
    /// the extension of the source file.
    fn mime_type() -> Option<&'static str> {
        Self::MIME_TYPE
    }

    /// [`root_dir`] returns the root path of the file at the time of embedding during
    /// compilation.
    fn root_dir() -> &'static str {
        Self::ROOT_DIR
    }

    /// [`source_file`] returns file path has provided in source for trait.
    fn source_file() -> &'static str {
        Self::SOURCE_FILE
    }

    /// [`source_paths`] returns the relative file path has identified during compilation.
    fn source_path() -> &'static str {
        Self::SOURCE_PATH
    }

    /// [`etag`] returns the safe web-related e-tag value for use in web APIs.
    /// It is really just the [Self::HASH`] enclosed in double quotes.
    fn etag() -> &'static str {
        Self::ETAG
    }

    /// [`hash`] returns the SHA-265 encoded content of the file.
    fn hash() -> &'static str {
        Self::HASH
    }
}
