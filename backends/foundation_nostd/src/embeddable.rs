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

    fn utf8_slice(&self) -> &'static [u8] {
        Self::UTF8
    }

    fn utf16_slice(&self) -> &'static [u16] {
        Self::UTF16
    }

    fn date_modified(&self) -> Option<&i64> {
        Self::DATE_MODIFIED_SINCE_UNIX_EPOC.as_ref()
    }

    fn mime_type(&self) -> Option<&str> {
        Self::MIME_TYPE
    }

    fn root_dir(&self) -> &str {
        Self::ROOT_DIR
    }

    fn source_file(&self) -> &str {
        Self::SOURCE_FILE
    }

    fn source_path(&self) -> &str {
        Self::SOURCE_PATH
    }

    fn etag(&self) -> &str {
        Self::ETAG
    }

    fn hash(&self) -> &str {
        Self::HASH
    }
}
