//! Arrow serialization for VFS types.
//!
//! Implements `ToArrow`, `FromArrow`, and `ArrowSchema` for `VfsMetadata`,
//! `VfsDirEntry`, `VfsCapabilities`, `VfsFileType`, `VfsEntryState`, and `Checksum`.

use std::sync::Arc;
use std::time::SystemTime;

use foundation_arrow::arrow_array::{
    self, ArrayRef, BinaryArray, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
    UInt64Array, UInt8Array,
};
use foundation_arrow::arrow_schema::{DataType, Field, Schema, ArrowError};
use foundation_arrow::{ArrowSchema, ArrowValue, FromArrow, IpcResult, ToArrow};

use crate::shared::vfs::types::{
    Checksum, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType, VfsMetadata,
};

fn err(s: String) -> ArrowError { ArrowError::ComputeError(s) }

// ──────────────────────────────────────────────
// VfsFileType
// ──────────────────────────────────────────────

impl ArrowSchema for VfsFileType {
    fn schema() -> Schema { Schema::new(vec![Field::new("file_type", DataType::UInt8, false)]) }
    fn fields() -> Vec<Field> { vec![Field::new("file_type", DataType::UInt8, false)] }
}

impl ToArrow for VfsFileType {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let arr: UInt8Array = values.iter().map(|v| match v {
            VfsFileType::Regular => 0u8, VfsFileType::Directory => 1u8, VfsFileType::Symlink => 2u8,
        }).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(arr) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for VfsFileType {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let val: u8 = batch.column(0).arrow_value(0).ok_or_else(|| err("null VfsFileType".into()))?;
        Ok(match val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, 2 => VfsFileType::Symlink, _ => return Err(err(format!("invalid VfsFileType: {val}"))) })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        let arr = batch.column(0);
        (0..batch.num_rows()).map(|i| {
            let val: u8 = arr.arrow_value(i).ok_or_else(|| err("null VfsFileType".into()))?;
            Ok(match val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, 2 => VfsFileType::Symlink, _ => return Err(err(format!("invalid VfsFileType: {val}"))) })
        }).collect()
    }
}

// ──────────────────────────────────────────────
// VfsEntryState
// ──────────────────────────────────────────────

impl ArrowSchema for VfsEntryState {
    fn schema() -> Schema { Schema::new(vec![Field::new("entry_state", DataType::UInt8, false)]) }
    fn fields() -> Vec<Field> { vec![Field::new("entry_state", DataType::UInt8, false)] }
}

impl ToArrow for VfsEntryState {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let arr: UInt8Array = values.iter().map(|v| match v { VfsEntryState::Ready => 0u8, VfsEntryState::Pending => 1u8 }).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(arr) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for VfsEntryState {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let val: u8 = batch.column(0).arrow_value(0).ok_or_else(|| err("null VfsEntryState".into()))?;
        Ok(match val { 0 => VfsEntryState::Ready, 1 => VfsEntryState::Pending, _ => return Err(err(format!("invalid VfsEntryState: {val}"))) })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        let arr = batch.column(0);
        (0..batch.num_rows()).map(|i| {
            let val: u8 = arr.arrow_value(i).ok_or_else(|| err("null VfsEntryState".into()))?;
            Ok(match val { 0 => VfsEntryState::Ready, 1 => VfsEntryState::Pending, _ => return Err(err(format!("invalid VfsEntryState: {val}"))) })
        }).collect()
    }
}

// ──────────────────────────────────────────────
// Checksum
// ──────────────────────────────────────────────

impl ArrowSchema for Checksum {
    fn schema() -> Schema {
        Schema::new(vec![Field::new("checksum_kind", DataType::UInt8, false), Field::new("checksum_data", DataType::Binary, true)])
    }
    fn fields() -> Vec<Field> {
        vec![Field::new("checksum_kind", DataType::UInt8, false), Field::new("checksum_data", DataType::Binary, true)]
    }
}

impl ToArrow for Checksum {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let kinds: UInt8Array = values.iter().map(|v| match v { Checksum::Blake3(_) => 1u8, Checksum::None => 0u8 }).collect();
        let data: BinaryArray = values.iter().map(|v| match v { Checksum::Blake3(b) => Some(b.as_slice()), Checksum::None => None }).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(kinds) as ArrayRef, Arc::new(data) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for Checksum {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let kind: u8 = batch.column(0).arrow_value(0).unwrap_or(0);
        let data: Option<Vec<u8>> = batch.column(1).arrow_value(0);
        Ok(match kind {
            1 => { let bytes = data.unwrap_or_default(); let mut arr = [0u8; 32]; arr.copy_from_slice(&bytes[..bytes.len().min(32)]); Checksum::Blake3(arr) }
            _ => Checksum::None,
        })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        let kinds = batch.column(0); let datas = batch.column(1);
        (0..batch.num_rows()).map(|i| {
            let kind: u8 = kinds.arrow_value(i).unwrap_or(0);
            let data: Option<Vec<u8>> = datas.arrow_value(i);
            Ok(match kind { 1 => { let bytes = data.unwrap_or_default(); let mut arr = [0u8; 32]; arr.copy_from_slice(&bytes[..bytes.len().min(32)]); Checksum::Blake3(arr) } _ => Checksum::None })
        }).collect()
    }
}

// ──────────────────────────────────────────────
// VfsCapabilities
// ──────────────────────────────────────────────

impl ArrowSchema for VfsCapabilities {
    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("seekable", DataType::Boolean, false), Field::new("symlinks", DataType::Boolean, false),
            Field::new("permissions_enforced", DataType::Boolean, false), Field::new("event_emission", DataType::Boolean, false),
            Field::new("persistent", DataType::Boolean, false),
        ])
    }
    fn fields() -> Vec<Field> {
        vec![
            Field::new("seekable", DataType::Boolean, false), Field::new("symlinks", DataType::Boolean, false),
            Field::new("permissions_enforced", DataType::Boolean, false), Field::new("event_emission", DataType::Boolean, false),
            Field::new("persistent", DataType::Boolean, false),
        ]
    }
}

impl ToArrow for VfsCapabilities {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let seekable: BooleanArray = values.iter().map(|v| Some(v.seekable)).collect();
        let symlinks: BooleanArray = values.iter().map(|v| Some(v.symlinks)).collect();
        let perms: BooleanArray = values.iter().map(|v| Some(v.permissions_enforced)).collect();
        let events: BooleanArray = values.iter().map(|v| Some(v.event_emission)).collect();
        let persistent: BooleanArray = values.iter().map(|v| Some(v.persistent)).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(seekable) as ArrayRef, Arc::new(symlinks) as ArrayRef, Arc::new(perms) as ArrayRef, Arc::new(events) as ArrayRef, Arc::new(persistent) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for VfsCapabilities {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let idx = 0;
        Ok(VfsCapabilities {
            seekable: batch.column(0).arrow_value(idx).unwrap_or(false), symlinks: batch.column(1).arrow_value(idx).unwrap_or(false),
            permissions_enforced: batch.column(2).arrow_value(idx).unwrap_or(false), event_emission: batch.column(3).arrow_value(idx).unwrap_or(false),
            persistent: batch.column(4).arrow_value(idx).unwrap_or(false),
        })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        (0..batch.num_rows()).map(|idx| {
            Ok(VfsCapabilities {
                seekable: batch.column(0).arrow_value(idx).unwrap_or(false), symlinks: batch.column(1).arrow_value(idx).unwrap_or(false),
                permissions_enforced: batch.column(2).arrow_value(idx).unwrap_or(false), event_emission: batch.column(3).arrow_value(idx).unwrap_or(false),
                persistent: batch.column(4).arrow_value(idx).unwrap_or(false),
            })
        }).collect()
    }
}

// ──────────────────────────────────────────────
// VfsDirEntry
// ──────────────────────────────────────────────

impl ArrowSchema for VfsDirEntry {
    fn schema() -> Schema { Schema::new(vec![Field::new("inode", DataType::UInt64, false), Field::new("name", DataType::Utf8, false), Field::new("file_type", DataType::UInt8, false)]) }
    fn fields() -> Vec<Field> { vec![Field::new("inode", DataType::UInt64, false), Field::new("name", DataType::Utf8, false), Field::new("file_type", DataType::UInt8, false)] }
}

impl ToArrow for VfsDirEntry {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let inodes: UInt64Array = values.iter().map(|v| Some(v.inode)).collect();
        let names: StringArray = values.iter().map(|v| Some(v.name.as_str())).collect();
        let file_types: UInt8Array = values.iter().map(|v| match v.file_type { VfsFileType::Regular => 0u8, VfsFileType::Directory => 1u8, VfsFileType::Symlink => 2u8 }).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(inodes) as ArrayRef, Arc::new(names) as ArrayRef, Arc::new(file_types) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for VfsDirEntry {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let idx = 0;
        let inode: u64 = batch.column(0).arrow_value(idx).unwrap_or(0);
        let name: String = batch.column(1).arrow_value(idx).unwrap_or_default();
        let ft_val: u8 = batch.column(2).arrow_value(idx).unwrap_or(0);
        Ok(VfsDirEntry { inode, name, file_type: match ft_val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, _ => VfsFileType::Symlink } })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        (0..batch.num_rows()).map(|idx| {
            let inode: u64 = batch.column(0).arrow_value(idx).unwrap_or(0);
            let name: String = batch.column(1).arrow_value(idx).unwrap_or_default();
            let ft_val: u8 = batch.column(2).arrow_value(idx).unwrap_or(0);
            Ok(VfsDirEntry { inode, name, file_type: match ft_val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, _ => VfsFileType::Symlink } })
        }).collect()
    }
}

// ──────────────────────────────────────────────
// VfsMetadata
// ──────────────────────────────────────────────

impl ArrowSchema for VfsMetadata {
    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("inode", DataType::UInt64, false), Field::new("size", DataType::UInt64, false),
            Field::new("file_type", DataType::UInt8, false), Field::new("permissions", DataType::UInt32, false),
            Field::new("owner_uid", DataType::UInt64, false), Field::new("owner_gid", DataType::UInt64, false),
            Field::new("created_ms", DataType::Int64, true), Field::new("modified_ms", DataType::Int64, true),
            Field::new("accessed_ms", DataType::Int64, true), Field::new("checksum_kind", DataType::UInt8, false),
            Field::new("checksum_data", DataType::Binary, true), Field::new("version", DataType::UInt64, false),
            Field::new("entry_state", DataType::UInt8, false),
        ])
    }
    fn fields() -> Vec<Field> {
        vec![
            Field::new("inode", DataType::UInt64, false), Field::new("size", DataType::UInt64, false),
            Field::new("file_type", DataType::UInt8, false), Field::new("permissions", DataType::UInt32, false),
            Field::new("owner_uid", DataType::UInt64, false), Field::new("owner_gid", DataType::UInt64, false),
            Field::new("created_ms", DataType::Int64, true), Field::new("modified_ms", DataType::Int64, true),
            Field::new("accessed_ms", DataType::Int64, true), Field::new("checksum_kind", DataType::UInt8, false),
            Field::new("checksum_data", DataType::Binary, true), Field::new("version", DataType::UInt64, false),
            Field::new("entry_state", DataType::UInt8, false),
        ]
    }
}

fn system_time_to_ms(t: Option<SystemTime>) -> Option<i64> { t.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64) }
fn ms_to_system_time(ms: Option<i64>) -> Option<SystemTime> { ms.map(|ms| SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64)) }

impl ToArrow for VfsMetadata {
    fn to_arrow(&self) -> IpcResult<RecordBatch> { Self::to_arrow_batch(std::slice::from_ref(self)) }
    fn to_arrow_batch(values: &[Self]) -> IpcResult<RecordBatch> {
        let inode: UInt64Array = values.iter().map(|v| Some(v.inode)).collect();
        let size: UInt64Array = values.iter().map(|v| Some(v.size)).collect();
        let file_type: UInt8Array = values.iter().map(|v| match v.file_type { VfsFileType::Regular => 0u8, VfsFileType::Directory => 1u8, VfsFileType::Symlink => 2u8 }).collect();
        let permissions: foundation_arrow::arrow_array::UInt32Array = values.iter().map(|v| Some(v.permissions)).collect();
        let owner_uid: UInt64Array = values.iter().map(|v| Some(v.owner.0 as u64)).collect();
        let owner_gid: UInt64Array = values.iter().map(|v| Some(v.owner.1 as u64)).collect();
        let created_ms: Int64Array = values.iter().map(|v| system_time_to_ms(v.created)).collect();
        let modified_ms: Int64Array = values.iter().map(|v| system_time_to_ms(v.modified)).collect();
        let accessed_ms: Int64Array = values.iter().map(|v| system_time_to_ms(v.accessed)).collect();
        let checksum_kind: UInt8Array = values.iter().map(|v| match &v.checksum { Checksum::Blake3(_) => 1u8, Checksum::None => 0u8 }).collect();
        let checksum_data: BinaryArray = values.iter().map(|v| match &v.checksum { Checksum::Blake3(b) => Some(b.as_slice()), Checksum::None => None }).collect();
        let version: UInt64Array = values.iter().map(|v| Some(v.version)).collect();
        let entry_state: UInt8Array = values.iter().map(|v| match v.state { VfsEntryState::Ready => 0u8, VfsEntryState::Pending => 1u8 }).collect();
        RecordBatch::try_new(Arc::new(Self::schema()), vec![Arc::new(inode) as ArrayRef, Arc::new(size) as ArrayRef, Arc::new(file_type) as ArrayRef, Arc::new(permissions) as ArrayRef, Arc::new(owner_uid) as ArrayRef, Arc::new(owner_gid) as ArrayRef, Arc::new(created_ms) as ArrayRef, Arc::new(modified_ms) as ArrayRef, Arc::new(accessed_ms) as ArrayRef, Arc::new(checksum_kind) as ArrayRef, Arc::new(checksum_data) as ArrayRef, Arc::new(version) as ArrayRef, Arc::new(entry_state) as ArrayRef]).map_err(|e| err(e.to_string()))
    }
}

impl FromArrow for VfsMetadata {
    fn from_arrow(batch: &RecordBatch) -> IpcResult<Self> {
        let idx = 0;
        let inode: u64 = batch.column(0).arrow_value(idx).unwrap_or(0);
        let ft_val: u8 = batch.column(2).arrow_value(idx).unwrap_or(0);
        let checksum_kind: u8 = batch.column(9).arrow_value(idx).unwrap_or(0);
        let checksum_data: Option<Vec<u8>> = batch.column(10).arrow_value(idx);
        let state_val: u8 = batch.column(12).arrow_value(idx).unwrap_or(0);
        let checksum = match checksum_kind { 1 => { let bytes = checksum_data.unwrap_or_default(); let mut arr = [0u8; 32]; arr.copy_from_slice(&bytes[..bytes.len().min(32)]); Checksum::Blake3(arr) } _ => Checksum::None };
        let permissions = batch.column(3).as_any().downcast_ref::<foundation_arrow::arrow_array::UInt32Array>().map_or(0, |a| a.value(idx));
        Ok(VfsMetadata {
            inode,
            size: batch.column(1).arrow_value(idx).unwrap_or(0),
            file_type: match ft_val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, _ => VfsFileType::Symlink },
            permissions,
            owner: (batch.column(4).arrow_value(idx).unwrap_or(0) as u32, batch.column(5).arrow_value(idx).unwrap_or(0) as u32),
            created: ms_to_system_time(batch.column(6).arrow_value(idx)),
            modified: ms_to_system_time(batch.column(7).arrow_value(idx)),
            accessed: ms_to_system_time(batch.column(8).arrow_value(idx)),
            checksum,
            version: batch.column(11).arrow_value(idx).unwrap_or(0),
            state: match state_val { 0 => VfsEntryState::Ready, _ => VfsEntryState::Pending },
        })
    }
    fn from_arrow_batch(batch: &RecordBatch) -> IpcResult<Vec<Self>> {
        (0..batch.num_rows()).map(|idx| {
            let inode: u64 = batch.column(0).arrow_value(idx).unwrap_or(0);
            let ft_val: u8 = batch.column(2).arrow_value(idx).unwrap_or(0);
            let checksum_kind: u8 = batch.column(9).arrow_value(idx).unwrap_or(0);
            let checksum_data: Option<Vec<u8>> = batch.column(10).arrow_value(idx);
            let state_val: u8 = batch.column(12).arrow_value(idx).unwrap_or(0);
            let checksum = match checksum_kind { 1 => { let bytes = checksum_data.clone().unwrap_or_default(); let mut arr = [0u8; 32]; arr.copy_from_slice(&bytes[..bytes.len().min(32)]); Checksum::Blake3(arr) } _ => Checksum::None };
            let permissions = batch.column(3).as_any().downcast_ref::<foundation_arrow::arrow_array::UInt32Array>().map_or(0, |a| a.value(idx));
            Ok(VfsMetadata {
                inode,
                size: batch.column(1).arrow_value(idx).unwrap_or(0),
                file_type: match ft_val { 0 => VfsFileType::Regular, 1 => VfsFileType::Directory, _ => VfsFileType::Symlink },
                permissions,
                owner: (batch.column(4).arrow_value(idx).unwrap_or(0) as u32, batch.column(5).arrow_value(idx).unwrap_or(0) as u32),
                created: ms_to_system_time(batch.column(6).arrow_value(idx)),
                modified: ms_to_system_time(batch.column(7).arrow_value(idx)),
                accessed: ms_to_system_time(batch.column(8).arrow_value(idx)),
                checksum,
                version: batch.column(11).arrow_value(idx).unwrap_or(0),
                state: match state_val { 0 => VfsEntryState::Ready, _ => VfsEntryState::Pending },
            })
        }).collect()
    }
}
