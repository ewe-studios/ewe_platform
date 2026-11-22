//! The key fixtures and the in-memory file system helpers.

use foundation_context::Ctx;
use foundation_nativeapis::shared::vfs::AsyncVfsFile;
use foundation_nativeapis::{AsyncVfsFileSystem, MemoryFs};

/// An Ed25519 key in PKCS#8 PEM (version 1, 48 bytes).
pub const ED25519_PKCS8: &str = include_str!("../fixtures/test-only-ed25519-key.pem");
/// The same Ed25519 key in OpenSSH private key form.
pub const ED25519_OPENSSH: &str = include_str!("../fixtures/ed25519-openssh-key");
/// An ECDSA P-256 key in PKCS#8 PEM.
pub const P256_PKCS8: &str = include_str!("../fixtures/p256-pkcs8-key.pem");
/// An ECDSA P-256 key in SEC1 PEM (`openssl ecparam -genkey`).
pub const P256_SEC1: &str = include_str!("../fixtures/p256-sec1-key.pem");
/// An ECDSA P-256 key in OpenSSH private key form.
pub const P256_OPENSSH: &str = include_str!("../fixtures/p256-openssh-key");
/// An RSA key in PKCS#8 PEM.
pub const RSA_PKCS8: &str = include_str!("../fixtures/rsa-pkcs8-key.pem");
/// An Ed25519 key in PKCS#8 PEM with a passphrase.
pub const ENCRYPTED_PKCS8: &str = include_str!("../fixtures/encrypted-pkcs8-key.pem");
/// An Ed25519 key in OpenSSH private key form with a passphrase.
pub const ENCRYPTED_OPENSSH: &str = include_str!("../fixtures/encrypted-openssh-key");

/// An empty in-memory file system.
pub fn memory_fs() -> MemoryFs {
    MemoryFs::new(Ctx::host())
}

/// Create `path` with `text` and the given Unix mode.
pub async fn write(fs: &MemoryFs, path: &str, text: &str, mode: u32) {
    if let Some((parent, _)) = path.rsplit_once('/') {
        if !parent.is_empty() {
            fs.mkdir_all_async(parent.to_string())
                .await
                .expect("create the parent directory");
        }
    }
    let file = fs
        .create_async(path.to_string(), mode)
        .await
        .expect("create the file");
    file.write_at_async(text.as_bytes().to_vec(), 0)
        .await
        .expect("write the file");
}
