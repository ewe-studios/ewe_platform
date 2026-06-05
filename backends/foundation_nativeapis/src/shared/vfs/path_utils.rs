use foundation_errstacks::ErrorTrace;

use super::error::{VfsError, VfsResult};

pub fn normalize_vfs_path(path: &str) -> VfsResult<String> {
    if path.is_empty() || path == "/" {
        return Ok("/".to_string());
    }
    let mut result = String::with_capacity(path.len());
    result.push('/');
    for part in path.split('/').filter(|p| !p.is_empty()) {
        if part == "." {
            continue;
        }
        if part == ".." {
            return Err(ErrorTrace::new(VfsError::InvalidPath {
                path: format!("path traversal not allowed: {path}"),
            }));
        }
        result.push_str(part);
        result.push('/');
    }
    if result.len() > 1 {
        result.pop();
    }
    Ok(result)
}

pub fn parent_path(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }
    match path.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(idx) => Some(path[..idx].to_string()),
        None => None,
    }
}

pub fn file_name(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) => &path[idx + 1..],
        None => path,
    }
}
