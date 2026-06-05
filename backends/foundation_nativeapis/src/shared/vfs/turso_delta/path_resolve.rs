//! Path resolution for TursoDelta.
//! Walks the turso_dentry table from root to target, one component at a time.

use std::sync::Mutex;

use libsql::Connection;

use crate::shared::vfs::error::{VfsError, VfsResult};

/// Resolve a VFS path to an ino by walking components from root.
/// Path must be absolute (start with '/') or empty (root).
pub fn resolve_path(conn: &Mutex<libsql::Connection>, path: &str) -> VfsResult<i64> {
    let conn = conn.lock().unwrap();

    if path == "/" || path.is_empty() {
        return Ok(1); // root ino
    }

    let path = path.trim_start_matches('/');
    let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();

    let mut current_ino: i64 = 1; // start at root

    for component in &components {
        let mut stmt = conn
            .prepare(
                "SELECT ino FROM turso_dentry WHERE parent_ino = ? AND name = ?",
            )
            .map_err(|e| VfsError::Io { source: e.into() })?;

        let mut rows = stmt
            .query((current_ino, component))
            .map_err(|e| VfsError::Io { source: e.into() })?;

        if let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
            current_ino = row
                .get_value(0)
                .map_err(|e| VfsError::Io { source: e.into() })?
                .as_integer()
                .ok_or_else(|| VfsError::NotFound { path: path.to_string() })?;
        } else {
            return Err(VfsError::NotFound { path: path.to_string() });
        }
    }

    Ok(current_ino)
}

/// Resolve parent ino and leaf name from a VFS path.
/// Returns (parent_ino, leaf_name).
pub fn resolve_parent(conn: &Mutex<libsql::Connection>, path: &str) -> VfsResult<(i64, String)> {
    let path = path.trim_start_matches('/');

    let (parent_path, leaf_name) = match path.rfind('/') {
        Some(idx) => (&path[..idx], &path[idx + 1..]),
        None => ("", path),
    };

    let parent_ino = if parent_path.is_empty() {
        1 // root
    } else {
        resolve_path(conn, &format!("/{}", parent_path))?
    };

    Ok((parent_ino, leaf_name.to_string()))
}

/// Get all descendant inos of a directory using recursive CTE.
pub fn subtree_inos(conn: &Mutex<libsql::Connection>, dir_ino: i64) -> VfsResult<Vec<i64>> {
    let conn = conn.lock().unwrap();

    let query = r#"
        WITH RECURSIVE subtree(ino, file_type) AS (
            SELECT ino, file_type FROM turso_dentry WHERE ino = ?
            UNION ALL
            SELECT d.ino, d.file_type
            FROM turso_dentry d
            JOIN subtree s ON d.parent_ino = s.ino
            WHERE s.file_type = 'dir'
        )
        SELECT ino FROM subtree WHERE ino != ?
    "#;

    let mut stmt = conn
        .prepare(query)
        .map_err(|e| VfsError::Io { source: e.into() })?;

    let mut rows = stmt
        .query((dir_ino, dir_ino))
        .map_err(|e| VfsError::Io { source: e.into() })?;

    let mut inos = Vec::new();
    while let Some(row) = rows.next().map_err(|e| VfsError::Io { source: e.into() })? {
        let ino = row
            .get_value(0)
            .map_err(|e| VfsError::Io { source: e.into() })?
            .as_integer()
            .unwrap_or(0);
        inos.push(ino);
    }

    Ok(inos)
}
