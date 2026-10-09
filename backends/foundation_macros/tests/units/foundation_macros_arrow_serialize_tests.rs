use foundation_arrow::ipc::{decode_ipc, encode_ipc};
use foundation_arrow::{ArrowMessageBox, FromArrow, ToArrow};
use foundation_macros::{ArrowSchema, FromArrow, ToArrow};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct SimpleRecord {
    name: String,
    count: u64,
    score: f64,
    active: bool,
}

#[test]
fn test_to_arrow_single_row() {
    let record = SimpleRecord {
        name: "test".to_string(),
        count: 42,
        score: 3.5,
        active: true,
    };
    let batch = record.to_arrow().unwrap();
    assert_eq!(batch.num_rows(), 1);
    assert_eq!(batch.num_columns(), 4);
}

#[test]
fn test_to_arrow_batch() {
    let records = vec![
        SimpleRecord {
            name: "alice".to_string(),
            count: 1,
            score: 1.0,
            active: true,
        },
        SimpleRecord {
            name: "bob".to_string(),
            count: 2,
            score: 2.0,
            active: false,
        },
    ];
    let batch = SimpleRecord::to_arrow_batch(&records).unwrap();
    assert_eq!(batch.num_rows(), 2);
    assert_eq!(batch.num_columns(), 4);
}

#[test]
fn test_roundtrip_single() {
    let original = SimpleRecord {
        name: "roundtrip".to_string(),
        count: 99,
        score: 2.5,
        active: false,
    };
    let batch = original.to_arrow().unwrap();
    let restored = SimpleRecord::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_roundtrip_batch() {
    let originals = vec![
        SimpleRecord {
            name: "first".to_string(),
            count: 10,
            score: 1.1,
            active: true,
        },
        SimpleRecord {
            name: "second".to_string(),
            count: 20,
            score: 2.2,
            active: false,
        },
        SimpleRecord {
            name: "third".to_string(),
            count: 30,
            score: 3.3,
            active: true,
        },
    ];
    let batch = SimpleRecord::to_arrow_batch(&originals).unwrap();
    let restored = SimpleRecord::from_arrow_batch(&batch).unwrap();
    assert_eq!(originals, restored);
}

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct WithOptionals {
    required_name: String,
    optional_size: Option<u64>,
    optional_data: Option<Vec<u8>>,
}

#[test]
fn test_roundtrip_with_some_values() {
    let original = WithOptionals {
        required_name: "file.txt".to_string(),
        optional_size: Some(1024),
        optional_data: Some(vec![1, 2, 3]),
    };
    let batch = original.to_arrow().unwrap();
    let restored = WithOptionals::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_roundtrip_with_none_values() {
    let original = WithOptionals {
        required_name: "empty.txt".to_string(),
        optional_size: None,
        optional_data: None,
    };
    let batch = original.to_arrow().unwrap();
    let restored = WithOptionals::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct AllIntegers {
    a: u8,
    b: u16,
    c: u32,
    d: u64,
    e: i8,
    f: i16,
    g: i32,
    h: i64,
}

#[test]
fn test_roundtrip_all_integer_types() {
    let original = AllIntegers {
        a: 255,
        b: 65535,
        c: 4_000_000_000,
        d: 18_000_000_000_000_000_000,
        e: -128,
        f: -32768,
        g: -2_000_000_000,
        h: -9_000_000_000_000_000_000,
    };
    let batch = original.to_arrow().unwrap();
    let restored = AllIntegers::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct FloatAndBool {
    x: f32,
    y: f64,
    flag: bool,
}

#[test]
fn test_roundtrip_floats_and_bool() {
    let original = FloatAndBool {
        x: 1.5,
        y: std::f64::consts::PI,
        flag: true,
    };
    let batch = original.to_arrow().unwrap();
    let restored = FloatAndBool::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct BinaryRecord {
    data: Vec<u8>,
    label: String,
}

#[test]
fn test_roundtrip_binary_data() {
    let original = BinaryRecord {
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        label: "binary".to_string(),
    };
    let batch = original.to_arrow().unwrap();
    let restored = BinaryRecord::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_from_arrow_empty_batch_errors() {
    let original = SimpleRecord {
        name: "x".to_string(),
        count: 0,
        score: 0.0,
        active: false,
    };
    let batch = original.to_arrow().unwrap();
    let empty = batch.slice(0, 0);
    let result = SimpleRecord::from_arrow(&empty);
    assert!(result.is_err());
}

#[test]
fn test_ipc_roundtrip() {
    use foundation_arrow::ipc::{decode_ipc, encode_ipc};

    let original = SimpleRecord {
        name: "ipc_test".to_string(),
        count: 777,
        score: 9.81,
        active: true,
    };
    let batch = original.to_arrow().unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    let decoded = decode_ipc(&bytes).unwrap();
    let restored = SimpleRecord::from_arrow(&decoded).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_batch_ipc_roundtrip() {
    use foundation_arrow::ipc::{decode_ipc, encode_ipc};

    let originals = vec![
        SimpleRecord {
            name: "a".to_string(),
            count: 1,
            score: 0.1,
            active: true,
        },
        SimpleRecord {
            name: "b".to_string(),
            count: 2,
            score: 0.2,
            active: false,
        },
    ];
    let batch = SimpleRecord::to_arrow_batch(&originals).unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    let decoded = decode_ipc(&bytes).unwrap();
    let restored = SimpleRecord::from_arrow_batch(&decoded).unwrap();
    assert_eq!(originals, restored);
}

// ── SystemTime roundtrip tests ──

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct TimedEvent {
    id: u64,
    name: String,
    occurred: SystemTime,
}

#[test]
fn test_roundtrip_systemtime() {
    let now = SystemTime::now();
    let original = TimedEvent {
        id: 1,
        name: "startup".to_string(),
        occurred: now,
    };
    let batch = original.to_arrow().unwrap();
    let restored = TimedEvent::from_arrow(&batch).unwrap();
    // Millisecond precision loss is expected
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.name, original.name);
    assert!(
        restored.occurred.duration_since(now).unwrap_or_default() < Duration::from_millis(2),
        "occurred time differs by more than 2ms"
    );
}

#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct WithOptionalTime {
    name: String,
    created_at: Option<SystemTime>,
    deleted_at: Option<SystemTime>,
}

#[test]
fn test_roundtrip_optional_time_some() {
    let original = WithOptionalTime {
        name: "file.txt".to_string(),
        created_at: Some(SystemTime::now()),
        deleted_at: None,
    };
    let batch = original.to_arrow().unwrap();
    let restored = WithOptionalTime::from_arrow(&batch).unwrap();
    assert_eq!(restored.name, original.name);
    assert!(restored.created_at.is_some());
    assert!(restored.deleted_at.is_none());
}

#[test]
fn test_roundtrip_optional_time_none() {
    let original = WithOptionalTime {
        name: "new.txt".to_string(),
        created_at: None,
        deleted_at: None,
    };
    let batch = original.to_arrow().unwrap();
    let restored = WithOptionalTime::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

// ── VFS-style record tests (simplified, enums flattened to primitives) ──

/// Simplified VFS directory entry for Arrow roundtrip testing.
/// In production, enums would be serialized via discriminant or expanded.
#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct VfsDirEntryArrow {
    name: String,
    file_type: u8, // 0=Regular, 1=Directory, 2=Symlink
}

#[test]
fn test_vfs_dir_entry_roundtrip() {
    let original = VfsDirEntryArrow {
        name: "src".to_string(),
        file_type: 1, // Directory
    };
    let batch = original.to_arrow().unwrap();
    let restored = VfsDirEntryArrow::from_arrow(&batch).unwrap();
    assert_eq!(original, restored);
}

/// Simplified VFS metadata for Arrow roundtrip testing.
#[derive(Debug, Clone, PartialEq, ArrowSchema, ToArrow, FromArrow)]
struct VfsMetadataArrow {
    size: u64,
    file_type: u8,
    permissions: u32,
    owner_uid: u32,
    owner_gid: u32,
    modified: Option<SystemTime>,
    version: u64,
}

#[test]
fn test_vfs_metadata_roundtrip() {
    let now = SystemTime::now();
    let original = VfsMetadataArrow {
        size: 4096,
        file_type: 0, // Regular
        permissions: 0o644,
        owner_uid: 1000,
        owner_gid: 1000,
        modified: Some(now),
        version: 3,
    };
    let batch = original.to_arrow().unwrap();
    let restored = VfsMetadataArrow::from_arrow(&batch).unwrap();
    assert_eq!(restored.size, original.size);
    assert_eq!(restored.file_type, original.file_type);
    assert_eq!(restored.permissions, original.permissions);
    assert_eq!(restored.owner_uid, original.owner_uid);
    assert_eq!(restored.owner_gid, original.owner_gid);
    assert_eq!(restored.version, original.version);
    assert!(restored.modified.is_some());
}

#[test]
fn test_vfs_metadata_ipc_roundtrip() {
    let original = VfsMetadataArrow {
        size: 1_048_576,
        file_type: 1, // Directory
        permissions: 0o755,
        owner_uid: 0,
        owner_gid: 0,
        modified: Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
        version: 1,
    };
    let batch = original.to_arrow().unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    let decoded = decode_ipc(&bytes).unwrap();
    let restored = VfsMetadataArrow::from_arrow(&decoded).unwrap();
    assert_eq!(restored.size, original.size);
    assert_eq!(restored.permissions, original.permissions);
    assert!(restored.modified.is_some());
}

#[test]
fn test_vfs_metadata_batch_roundtrip() {
    let originals = vec![
        VfsMetadataArrow {
            size: 100,
            file_type: 0,
            permissions: 0o644,
            owner_uid: 1000,
            owner_gid: 1000,
            modified: Some(UNIX_EPOCH),
            version: 1,
        },
        VfsMetadataArrow {
            size: 200,
            file_type: 1,
            permissions: 0o755,
            owner_uid: 0,
            owner_gid: 0,
            modified: None,
            version: 2,
        },
    ];
    let batch = VfsMetadataArrow::to_arrow_batch(&originals).unwrap();
    let restored = VfsMetadataArrow::from_arrow_batch(&batch).unwrap();
    assert_eq!(originals, restored);
}

// ── ArrowMessageBox trait tests ──

#[test]
fn test_arrow_message_box_encode_decode() {
    let original = SimpleRecord {
        name: "message".to_string(),
        count: 42,
        score: 3.5,
        active: true,
    };
    let bytes = original.encode_arrow().unwrap();
    let restored = SimpleRecord::decode_arrow(&bytes).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn test_arrow_message_box_vfs_encode_decode() {
    let original = VfsMetadataArrow {
        size: 8192,
        file_type: 0,
        permissions: 0o644,
        owner_uid: 1000,
        owner_gid: 1000,
        modified: Some(SystemTime::now()),
        version: 5,
    };
    let bytes = original.encode_arrow().unwrap();
    let restored = VfsMetadataArrow::decode_arrow(&bytes).unwrap();
    assert_eq!(restored.size, original.size);
    assert_eq!(restored.permissions, original.permissions);
    assert_eq!(restored.version, original.version);
    assert!(restored.modified.is_some());
}

#[test]
fn test_arrow_message_box_invalid_bytes() {
    let invalid = b"not valid arrow ipc bytes";
    let result = SimpleRecord::decode_arrow(invalid);
    assert!(result.is_err());
}
