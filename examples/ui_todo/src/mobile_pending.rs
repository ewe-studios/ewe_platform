//! Store pending changes in the application directory.
use std::io::Write;
use std::path::PathBuf;

pub(super) struct FileStorage(pub(super) PathBuf);

#[async_trait::async_trait]
impl crate::PendingStorage for FileStorage {
    async fn load(&self) -> Result<Vec<u8>, String> {
        let path = self.0.clone();
        blocking::unblock(move || match std::fs::read(path) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(error) => Err(error.to_string()),
        })
        .await
    }

    async fn save(&self, contents: Vec<u8>) -> Result<(), String> {
        let path = self.0.clone();
        blocking::unblock(move || -> std::io::Result<()> {
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("Missing outbox directory"))?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(&contents)?;
            file.as_file().sync_all()?;
            file.persist(&path).map_err(|e| e.error)?;
            std::fs::File::open(parent)?.sync_all()
        })
        .await
        .map_err(|e| e.to_string())
    }
}
