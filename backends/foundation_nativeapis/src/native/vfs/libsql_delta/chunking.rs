//! Chunking strategy for LibsqlDelta.
//! File content is stored as ordered chunks in vfs_chunks.

use libsql::Connection;

use crate::shared::vfs::error::VfsResult;
use super::types;

/// Split data into chunks of the given size.
pub fn split_into_chunks<'a>(data: &'a [u8], chunk_size: usize) -> Vec<&'a [u8]> {
    data.chunks(chunk_size).collect()
}

/// Read a byte range from a file starting at offset.
pub async fn read_chunk_range_async(
    conn: &Connection,
    ino: i64,
    file_size: u64,
    chunk_size: usize,
    buf: &mut [u8],
    offset: u64,
) -> VfsResult<usize> {
    if offset >= file_size {
        return Ok(0);
    }

    let end = std::cmp::min(offset + buf.len() as u64, file_size);
    let read_len = (end - offset) as usize;

    let start_chunk = (offset / chunk_size as u64) as i64;
    let end_chunk = ((end - 1) / chunk_size as u64) as i64;

    let stmt = conn
        .prepare("SELECT chunk_idx, data FROM vfs_chunks WHERE ino = ? AND chunk_idx BETWEEN ? AND ? ORDER BY chunk_idx")
        .await
        .map_err(types::libsql_err)?;

    let mut rows = stmt
        .query((ino, start_chunk, end_chunk))
        .await
        .map_err(types::libsql_err)?;

    let mut chunks: Vec<(i64, Vec<u8>)> = Vec::new();
    while let Some(row) = rows.next().await.map_err(types::libsql_err)? {
        let chunk_idx = row.get::<i64>(0).map_err(types::libsql_err)?;
        let data = row.get::<Vec<u8>>(1).map_err(types::libsql_err)?;
        chunks.push((chunk_idx, data));
    }

    if chunks.is_empty() {
        return Ok(0);
    }

    let global_offset = start_chunk * chunk_size as i64;
    let local_offset = (offset - global_offset as u64) as usize;

    let mut assembled = Vec::new();
    for (_, data) in &chunks {
        assembled.extend_from_slice(data);
    }

    let available = assembled.len().saturating_sub(local_offset);
    let to_read = std::cmp::min(read_len, available);

    buf[..to_read].copy_from_slice(&assembled[local_offset..local_offset + to_read]);
    Ok(to_read)
}

/// Write all chunks for a file.
pub async fn write_all_chunks_async(
    conn: &Connection,
    ino: i64,
    data: &[u8],
    chunk_size: usize,
) -> VfsResult<()> {
    let chunks = split_into_chunks(data, chunk_size);

    // Batch insert all chunks in a single dynamic multi-row INSERT.
    // We must NOT use a prepared statement — libsql silently drops rows
    // when a prepared Statement is reused in a loop.
    let placeholders = chunks
        .iter()
        .map(|_| "(?, ?, ?)")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO vfs_chunks (ino, chunk_idx, data) VALUES {}",
        placeholders
    );
    let mut values: Vec<libsql::Value> = Vec::with_capacity(chunks.len() * 3);
    for (idx, &chunk) in chunks.iter().enumerate() {
        values.push(ino.into());
        values.push((idx as i64).into());
        values.push(chunk.to_vec().into());
    }
    conn.execute(&sql, values).await.map_err(types::libsql_err)?;

    let checksum = blake3::hash(data);
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    conn.execute(
        "UPDATE vfs_dentry SET size = ?, checksum = ?, updated_at = ? WHERE ino = ?",
        (data.len() as i64, checksum.as_bytes().to_vec(), updated_at, ino),
    ).await.map_err(types::libsql_err)?;

    Ok(())
}

/// Truncate a file to the given size.
pub async fn truncate_file_async(
    conn: &Connection,
    ino: i64,
    new_size: u64,
    chunk_size: usize,
) -> VfsResult<()> {
    if new_size == 0 {
        conn.execute("DELETE FROM vfs_chunks WHERE ino = ?", [ino])
            .await
            .map_err(types::libsql_err)?;
    } else {
        let last_chunk_idx = ((new_size - 1) / chunk_size as u64) as i64;

        conn.execute(
            "DELETE FROM vfs_chunks WHERE ino = ? AND chunk_idx > ?",
            (ino, last_chunk_idx),
        ).await.map_err(types::libsql_err)?;

        let remainder = new_size as usize % chunk_size;
        if remainder != 0 {
            conn.execute(
                "UPDATE vfs_chunks SET data = SUBSTR(data, 1, ?) WHERE ino = ? AND chunk_idx = ?",
                (remainder as i64, ino, last_chunk_idx),
            ).await.map_err(types::libsql_err)?;
        }
    }

    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    conn.execute(
        "UPDATE vfs_dentry SET size = ?, updated_at = ? WHERE ino = ?",
        (new_size as i64, updated_at, ino),
    ).await.map_err(types::libsql_err)?;

    Ok(())
}
