//! Tests for VFS Arrow serialization.

use foundation_arrow::{decode_ipc, encode_ipc, ToArrow};
use foundation_nativeapis::shared::vfs::types::{VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata};

#[test]
fn test_vfs_capabilities_roundtrip() {
    let caps = VfsCapabilities {
        seekable: true,
        symlinks: true,
        permissions_enforced: false,
        event_emission: false,
        persistent: true,
    };
    let batch = caps.to_arrow().expect("to_arrow failed");
    let encoded = encode_ipc(&batch).expect("encode failed");
    let decoded = decode_ipc(&encoded).expect("decode failed");
    assert_eq!(decoded.num_rows(), 1);
}

#[test]
fn test_vfs_dir_entry_roundtrip() {
    let entry = VfsDirEntry {
        inode: 1,
        name: "test.txt".to_string(),
        file_type: VfsFileType::Regular,
    };
    let batch = entry.to_arrow().expect("to_arrow failed");
    let encoded = encode_ipc(&batch).expect("encode failed");
    let decoded = decode_ipc(&encoded).expect("decode failed");
    assert_eq!(decoded.num_rows(), 1);
}

#[test]
fn test_vfs_metadata_roundtrip() {
    let meta = VfsMetadata::new_file(1, 42, 0o644);
    let batch = meta.to_arrow().expect("to_arrow failed");
    let encoded = encode_ipc(&batch).expect("encode failed");
    let decoded = decode_ipc(&encoded).expect("decode failed");
    assert_eq!(decoded.num_rows(), 1);
}

#[test]
fn test_vec_roundtrip() {
    let entries = vec![
        VfsDirEntry { inode: 1, name: "foo".to_string(), file_type: VfsFileType::Directory },
        VfsDirEntry { inode: 2, name: "bar.txt".to_string(), file_type: VfsFileType::Regular },
        VfsDirEntry { inode: 3, name: "link".to_string(), file_type: VfsFileType::Symlink },
    ];
    let batch = VfsDirEntry::to_arrow_batch(&entries).expect("to_arrow_batch failed");
    assert_eq!(batch.num_rows(), 3);
    let encoded = encode_ipc(&batch).expect("encode failed");
    let decoded = decode_ipc(&encoded).expect("decode failed");
    assert_eq!(decoded.num_rows(), 3);
}
