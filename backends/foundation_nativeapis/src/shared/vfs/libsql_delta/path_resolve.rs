//! Path resolution for LibsqlDelta.
//! Walks the sqlite_dentry table from root to target, one component at a time.

use std::sync::Arc;

use libsql::Connection;

use crate::shared::vfs::error::{VfsError, VfsResult};
use foundation_errstacks::ErrorTrace;
use super::types;

/// Resolve a VFS path to an ino by walking components from root.
/// Path must be absolute (start with '/') or empty (root).
pub async fn resolve_path_async(conn: Arc<Connection>, path: &str) -> VfsResult<i64> {
    if path == "/" || path.is_empty() {
        return Ok(1); // root ino
    }

    let path = path.trim_start_matches('/');
    let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();

    let mut current_ino: i64 = 1; // start at root

    for component in components {
        let mut stmt = conn
            .prepare("SELECT ino FROM sqlite_dentry WHERE parent_ino = ? AND name = ?")
            .await
            .map_err(types::libsql_err)?;

        let mut rows = stmt
            .query((current_ino, component))
            .await
            .map_err(types::libsql_err)?;

        if let Some(row) = rows.next().await.map_err(types::libsql_err)? {
            current_ino = row
                .get::<i64>(0)
                .map_err(types::libsql_err)?;
        } else {
            return Err(ErrorTrace::new(VfsError::NotFound { path: path.to_string() }));
        }
    }

    Ok(current_ino)
}

/// Resolve parent ino and leaf name from a VFS path.
/// Returns (parent_ino, leaf_name).
pub async fn resolve_parent_async(conn: Arc<Connection>, path: &str) -> VfsResult<(i64, String)> {
    let path = path.trim_start_matches('/');

    let (parent_path, leaf_name) = match path.rfind('/') {
        Some(idx) => (&path[..idx], &path[idx + 1..]),
        None => ("", path),
    };

    let parent_ino = if parent_path.is_empty() {
        1 // root
    } else {
        resolve_path_async(conn, &format!("/{}", parent_path)).await?
    };

    Ok((parent_ino, leaf_name.to_string()))
}

/// Get all descendant inos of a directory using recursive CTE.
pub async fn subtree_inos_async(conn: Arc<Connection>, dir_ino: i64) -> VfsResult<Vec<i64>> {
    let query = r#"
        WITH RECURSIVE subtree(ino, file_type) AS (
            SELECT ino, file_type FROM sqlite_dentry WHERE ino = ?
            UNION ALL
            SELECT d.ino, d.file_type
            FROM sqlite_dentry d
            JOIN subtree s ON d.parent_ino = s.ino
            WHERE s.file_type = 'dir'
        )
        SELECT ino FROM subtree WHERE ino != ?
    "#;

    let mut stmt = conn
        .prepare(query)
        .await
        .map_err(types::libsql_err)?;

    let mut rows = stmt
        .query((dir_ino, dir_ino))
        .await
        .map_err(types::libsql_err)?;

    let mut inos = Vec::new();
    while let Some(row) = rows.next().await.map_err(types::libsql_err)? {
        let ino = row.get::<i64>(0).map_err(types::libsql_err)?;
        inos.push(ino);
    }

    Ok(inos)
}
