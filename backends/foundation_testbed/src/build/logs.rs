//! Build log error extraction.
//!
//! When a build fails, this module greps the build log for error stanzas
//! with context, making it easier to diagnose what went wrong.

use crate::config::Result;
use crate::ssh::VmSession;

/// Error patterns to search for in build logs.
const ERROR_PATTERNS: &[&str] = &[
    "error[E",       // Rust compiler errors
    "error:",        // Generic errors
    "FAILED",        // Build failure markers
    "panic",         // Rust panics
    "fatal error",   // Compiler fatal errors
    "unresolved external symbol", // MSVC linker errors
    "LNK",           // MSVC linker error codes
    "cannot find -l", // Linker missing library
    "linker .* not found", // Missing linker
    "mise ERROR",    // Mise errors
];

/// Extract error stanzas from the build log.
///
/// Returns a formatted string with all matching error lines and 2 lines
/// of context before each match.
pub fn dump_build_log_errors(session: &mut VmSession) -> Result<String> {
    // Read the build log
    let log_content = match crate::ssh::exec(session, "cat ~/.testbed-build/build.log 2>/dev/null || echo 'NO_LOG'") {
        Ok(output) => output,
        Err(_) => return Ok("No build log available".to_string()),
    };

    if log_content.trim() == "NO_LOG" {
        return Ok("No build log available".to_string());
    }

    extract_errors(&log_content)
}

/// Extract error lines from log content.
pub fn extract_errors(log: &str) -> Result<String> {
    let lines: Vec<&str> = log.lines().collect();
    let mut errors = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        // Check if this line matches any error pattern
        if ERROR_PATTERNS.iter().any(|p| {
            if p.contains(".*") {
                // Simple regex-like check
                let parts: Vec<&str> = p.split(".*").collect();
                parts.len() == 2 && line.contains(parts[0]) && line.contains(parts[1])
            } else {
                line.contains(p)
            }
        }) {
            // Get 2 lines of context before this line
            let start = i.saturating_sub(2);
            let context: Vec<&str> = lines[start..=i].to_vec();
            errors.push(context.join("\n"));
        }
    }

    if errors.is_empty() {
        return Ok("No error patterns found in build log".to_string());
    }

    // Deduplicate and limit output
    errors.dedup();
    let output = errors[..errors.len().min(20)].join("\n---\n");

    Ok(format!("Build errors ({} found):\n---\n{}", errors.len(), output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_errors_finds_rust_error() {
        let log = r#"Compiling my-app v0.1.0
   Compiling dependency v1.0.0
error[E0308]: mismatched types
   --> src/main.rs:10:5
    |
10  |     let x: String = 42;
    |         ^   ------ expected due to this
    |         |
    |         expected `String`, found integer
"#;
        let result = extract_errors(log).unwrap();
        assert!(result.contains("E0308"));
        assert!(result.contains("mismatched types"));
    }

    #[test]
    fn test_extract_errors_empty_log() {
        let result = extract_errors("Build completed successfully\n").unwrap();
        assert!(result.contains("No error patterns"));
    }

    #[test]
    fn test_error_patterns_contains_expected_values() {
        assert!(ERROR_PATTERNS.iter().any(|p| p.contains("error")));
        assert!(ERROR_PATTERNS.iter().any(|p| p.contains("FAILED")));
        assert!(ERROR_PATTERNS.iter().any(|p| p.contains("linker")));
    }
}
