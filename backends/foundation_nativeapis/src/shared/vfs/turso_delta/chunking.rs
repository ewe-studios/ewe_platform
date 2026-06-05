//! Chunking strategy for TursoDelta.
//! File content is stored as ordered chunks in turso_chunks.

use std::sync::Mutex;

use turso::Connection;

use crate::shared::vfs::error::{VfsError, VfsResult};

/// Split data into chunks of the given size.
/// Returns a Vec of byte slices (borrowed from input).
pub fn split_into_chunks<'a>(data: &'a [u8], chunk_size: usize) -> Vec<&'a [u8]> {
    data.chunks(chunk_size).collect()
}

/// Reassemble chunks into a single Vec<u8>.
pub fn reassemble_chunks(chunks: &[Vec<u8>]) -> Vec<u8> {
    let total_len: usize = chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for chunk in chunks {
        result.extend_from_slice(chunk);
    }
    result
}

/// Read a byte range from a file starting at offset.
/// Returns the number of bytes read and fills the buffer.
pub fn read_chunk_range(
    conn: &Mutex<Connection>,
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

    let conn = conn.lock().unwrap();

    let mut stmt = conn
        .prepare("SELECT chunk_idx, data FROM turso_chunks WHERE ino = ? AND chunk_idx BETWEEN ? AND ? ORDER BY chunk_idx")
        .map_err(|e| VfsError::Io { source: e.into() })?;

    let mut rows = stmt
        .query((ino, start_chunk, end_chunk))
        .map_err(|e| VfsError::Io { source: e.into() })?;

    // Collect all chunks
    let mut chunks: Vec<(i64, Vec<u8>)> = Vec::new();
    while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
        let chunk_idx = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(0);
        let data = row
            .get_value(1)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_blob()
            .unwrap_or(&[])
            .to_vec();
        chunks.push((chunk_idx, data));
    }

    if chunks.is_empty() {
        return Ok(0);
    }

    // Reassemble into a contiguous buffer, then slice
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

/// Write data to a file at the given offset, replacing/adding chunks.
/// Returns the new file size.
pub fn write_chunk_data(
    conn: &Mutex<Connection>,
    ino: i64,
    chunk_size: usize,
    buf: &[u8],
    offset: u64,
) -> VfsResult<u64> {
    let conn = conn.lock().unwrap();

    // Read existing file size
    let current_size: u64 = {
        let mut stmt = conn
            .prepare("SELECT size FROM turso_dentry WHERE ino = ?")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let row = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?
            .next()
            .map_err(|e| VfsError::Io { source: e.into() })?
            .ok_or_else(|| VfsError::NotFound { path: format!("ino={ino}") })?;

        row.get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(0) as u64
    };

    // Read all existing chunks
    let mut existing_chunks: Vec<(i64, Vec<u8>)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT chunk_idx, data FROM turso_chunks WHERE ino = ? ORDER BY chunk_idx")
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut rows = stmt
            .query((ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
            let idx = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .unwrap_or(0);
            let data = row
                .get_value(1)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_blob()
                .unwrap_or(&[])
                .to_vec();
            existing_chunks.push((idx, data));
        }
    }

    // Reassemble into a single buffer
    let mut file_data = Vec::with_capacity(current_size as usize);
    for (_, chunk) in &existing_chunks {
        file_data.extend_from_slice(chunk);
    }

    // Extend if needed
    if file_data.len() < current_size as usize {
        file_data.resize(current_size as usize, 0);
    }

    // Write the new data at offset
    let end = offset as usize + buf.len();
    if end > file_data.len() {
        file_data.resize(end, 0);
    }
    file_data[offset as usize..end].copy_from_slice(buf);

    let new_size = file_data.len() as u64;

    // Split into new chunks and write them
    let new_chunks = split_into_chunks(&file_data, chunk_size);

    // Delete old chunks
    conn.execute("DELETE FROM turso_chunks WHERE ino = ?", (ino,))
        .map_err(|e| VfsError::Io { source: e.into() })?;

    // Insert new chunks
    let mut stmt = conn
        .prepare("INSERT INTO turso_chunks (ino, chunk_idx, data) VALUES (?, ?, ?)")
        .map_err(|e| VfsError::Io { source: e.into() })?;

    for (idx, chunk) in new_chunks.iter().enumerate() {
        stmt.execute((ino, idx as i64, chunk))
            .map_err(|e| VfsError::Io { source: e.into() })?;
    }

    // Update dentry size
    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    conn.execute(
        "UPDATE turso_dentry SET size = ?, updated_at = ? WHERE ino = ?",
        (new_size as i64, updated_at, ino),
    )
    .map_err(|e| VfsError::Io { source: e.into() })?;

    Ok(new_size)
}

/// Write all chunks for a new file (atomic, within transaction).
pub fn write_all_chunks(
    conn: &Mutex<Connection>,
    ino: i64,
    data: &[u8],
    chunk_size: usize,
) -> VfsResult<()> {
    let conn = conn.lock().unwrap();

    let chunks = split_into_chunks(data, chunk_size);

    let mut stmt = conn
        .prepare("INSERT INTO turso_chunks (ino, chunk_idx, data) VALUES (?, ?, ?)")
        .map_err(|e| VfsError::Io { source: e.into() })?;

    for (idx, chunk) in chunks.iter().enumerate() {
        stmt.execute((ino, idx as i64, chunk))
            .map_err(|e| VfsError::Io { source: e.into() })?;
    }

    // Compute blake3 checksum
    let checksum = blake3::hash(data);

    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    conn.execute(
        "UPDATE turso_dentry SET size = ?, checksum = ?, updated_at = ? WHERE ino = ?",
        (
            data.len() as i64,
            checksum.as_bytes().to_vec(),
            updated_at,
            ino,
        ),
    )
    .map_err(|e| VfsError::Io { source: e.into() })?;

    Ok(())
}

/// Truncate a file to the given size.
pub fn truncate_file(
    conn: &Mutex<Connection>,
    ino: i64,
    new_size: u64,
    chunk_size: usize,
) -> VfsResult<()> {
    let conn = conn.lock().unwrap();

    if new_size == 0 {
        conn.execute("DELETE FROM turso_chunks WHERE ino = ?", (ino,))
            .map_err(|e| VfsError::Io { source: e.into() })?;
    } else {
        let last_chunk_idx = ((new_size - 1) / chunk_size as u64) as i64;

        // Delete chunks beyond the last one
        conn.execute(
            "DELETE FROM turso_chunks WHERE ino = ? AND chunk_idx > ?",
            (ino, last_chunk_idx),
        )
        .map_err(|e| VfsError::Io { source: e.into() })?;

        // Truncate the last chunk if needed
        let remainder = new_size as usize % chunk_size;
        if remainder != 0 {
            conn.execute(
                "UPDATE turso_chunks SET data = SUBSTR(data, 1, ?) WHERE ino = ? AND chunk_idx = ?",
                (remainder as i64, ino, last_chunk_idx),
            )
            .map_err(|e| VfsError::Io { source: e.into() })?;
        }
    }

    let updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    conn.execute(
        "UPDATE turso_dentry SET size = ?, updated_at = ? WHERE ino = ?",
        (new_size as i64, updated_at, ino),
    )
    .map_err(|e| VfsError::Io { source: e.into() })?;

    Ok(())
}
