/// Example: Arrow Serialization for VFS Types.
///
/// Demonstrates how VfsMetadata and VfsDirEntry can be serialized to/from
/// Arrow IPC format using foundation_arrow. This enables zero-copy batch
/// operations on VFS metadata — useful for indexing, replication, and
/// analytics pipelines.

use foundation_arrow::{ArrowSchema, ToArrow, FromArrow};
use foundation_arrow::arrow_schema::Schema;
use foundation_nativeapis::shared::vfs::types::{VfsDirEntry, VfsFileType};

fn main() {
    println!("=== Arrow Serialization Example ===\n");

    // Build some sample VfsDirEntry
    let entries: Vec<VfsDirEntry> = vec![
        VfsDirEntry {
            name: "src".to_string(),
            inode: 1,
            file_type: VfsFileType::Directory,
        },
        VfsDirEntry {
            name: "Cargo.toml".to_string(),
            inode: 2,
            file_type: VfsFileType::Regular,
        },
        VfsDirEntry {
            name: "README.md".to_string(),
            inode: 3,
            file_type: VfsFileType::Regular,
        },
    ];

    println!("Directory listing ({} entries):", entries.len());
    for e in &entries {
        println!("  {} (inode: {}) - {:?}", e.name, e.inode, e.file_type);
    }
    println!();

    // Show Arrow schema
    let schema: Schema = VfsDirEntry::schema();
    println!("Arrow schema for VfsDirEntry:");
    for field in schema.fields() {
        println!("  {}: {:?} (nullable: {})", field.name(), field.data_type(), field.is_nullable());
    }
    println!();

    // Serialize to Arrow batch
    let batch = VfsDirEntry::to_arrow_batch(&entries).expect("failed to create Arrow batch");
    println!("Serialized to Arrow RecordBatch:");
    println!("  rows: {}", batch.num_rows());
    println!("  columns: {}", batch.num_columns());
    for (i, field) in batch.schema().fields().iter().enumerate() {
        println!("    col[{}] {}: {:?}", i, field.name(), batch.column(i).data_type());
    }
    println!();

    // Deserialize back
    let roundtrip: Vec<VfsDirEntry> = VfsDirEntry::from_arrow_batch(&batch)
        .expect("failed to deserialize from Arrow");

    println!("Round-trip verification:");
    for (original, recovered) in entries.iter().zip(roundtrip.iter()) {
        assert_eq!(original.name, recovered.name, "name mismatch for inode {}", original.inode);
        assert_eq!(original.inode, recovered.inode, "inode mismatch");
        assert!(std::mem::discriminant(&original.file_type) == std::mem::discriminant(&recovered.file_type),
            "file_type mismatch for {}", original.name);
        println!("  {} (inode: {}) - OK", original.name, original.inode);
    }
    println!();

    println!("With Arrow IPC, these entries can be:");
    println!("  1. Serialized to a single IPC batch (~100ns per entry)");
    println!("  2. Written to disk for persistence");
    println!("  3. Streamed over a network connection");
    println!("  4. Read back with zero-copy field access");

    println!("\n=== Done ===");
}
