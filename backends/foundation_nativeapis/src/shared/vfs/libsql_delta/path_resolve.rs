//! Path resolution for LibsqlDelta.

use std::sync::Arc;

use libsql::Connection;
use foundation_errstacks::ErrorTrace;

use crate::shared::vfs::error::{VfsError, VfsResult};
use super::types;

fn le(e: libsql::Error) -> ErrorTrace<VfsError> { ErrorTrace::new(types::libsql_err(e)) }

/// Resolve a VFS path to an ino by walking components from root.
pub async fn resolve_path_async(conn: Arc<Connection>, path: String) -> VfsResult<i64> {
    if path == "/" || path.is_empty() { return Ok(1); }

    let path_str = path.trim_start_matches('/');
    let components: Vec<String> = path_str.split('/').filter(|c| !c.is_empty()).map(String::from).collect();
    let full_path = path;

    let mut current_ino: i64 = 1;
    for component in &components {
        let stmt = conn.prepare("SELECT ino FROM vfs_dentry WHERE parent_ino = ? AND name = ?").await.map_err(le)?;
        let mut rows = stmt.query((current_ino, component.as_str())).await.map_err(le)?;
        if let Some(row) = rows.next().await.map_err(le)? {
            current_ino = row.get::<i64>(0).map_err(le)?;
        } else {
            return Err(ErrorTrace::new(VfsError::NotFound { path: full_path }));
        }
    }
    Ok(current_ino)
}

/// Resolve parent ino and leaf name from a VFS path.
pub async fn resolve_parent_async(conn: Arc<Connection>, path: String) -> VfsResult<(i64, String)> {
    let path_str = path.trim_start_matches('/');
    let (parent_path, leaf_name) = match path_str.rfind('/') {
        Some(idx) => (&path_str[..idx], &path_str[idx + 1..]),
        None => ("", path_str),
    };
    let parent_ino = if parent_path.is_empty() {
        1
    } else {
        resolve_path_async(conn.clone(), format!("/{}", parent_path)).await?
    };
    Ok((parent_ino, leaf_name.to_string()))
}

/// Get all descendant inos of a directory using recursive CTE.
#[allow(dead_code)]
pub async fn subtree_inos_async(conn: Arc<Connection>, dir_ino: i64) -> VfsResult<Vec<i64>> {
    let stmt = conn.prepare(
        "WITH RECURSIVE subtree(ino, file_type) AS (
            SELECT ino, file_type FROM vfs_dentry WHERE ino = ?
            UNION ALL
            SELECT d.ino, d.file_type FROM vfs_dentry d
            JOIN subtree s ON d.parent_ino = s.ino WHERE s.file_type = 'dir'
        ) SELECT ino FROM subtree WHERE ino != ?",
    ).await.map_err(le)?;
    let mut rows = stmt.query((dir_ino, dir_ino)).await.map_err(le)?;
    let mut inos = Vec::new();
    while let Some(row) = rows.next().await.map_err(le)? {
        inos.push(row.get::<i64>(0).map_err(le)?);
    }
    Ok(inos)
}

