//! Glob matching over stream-root-relative paths.
//!
//! `*` matches one path segment. `**` matches zero or more path segments. A literal segment
//! matches itself. This is the pattern language of protocol section 6.2.

/// Match a path against a glob pattern.
#[must_use]
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern_parts = split_path(pattern);
    let path_parts = split_path(path);
    match_parts(&pattern_parts, 0, &path_parts, 0)
}

fn split_path(value: &str) -> Vec<&str> {
    value
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn match_parts(pattern: &[&str], mut pi: usize, path: &[&str], mut si: usize) -> bool {
    while pi < pattern.len() && si < path.len() {
        match pattern[pi] {
            "**" => {
                for candidate in si..=path.len() {
                    if match_parts(pattern, pi + 1, path, candidate) {
                        return true;
                    }
                }
                return false;
            }
            "*" => {
                pi += 1;
                si += 1;
            }
            literal => {
                if literal != path[si] {
                    return false;
                }
                pi += 1;
                si += 1;
            }
        }
    }

    while pi < pattern.len() && pattern[pi] == "**" {
        pi += 1;
    }
    pi == pattern.len() && si == path.len()
}
