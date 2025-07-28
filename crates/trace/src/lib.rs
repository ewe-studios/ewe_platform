/// Crate to abstract out tracing so it never shows up in release builds using macros
/// See similar: https://doc.rust-lang.org/src/std/macros.rs.html#138-145.

#[cfg(not(feature = "log_info"))]
#[macro_export]
macro_rules! info {
    ($($t:tt)*) => {};
}

#[cfg(not(feature = "log_warnings"))]
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => {};
}

#[cfg(not(feature = "log_errors"))]
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {};
}

#[cfg(not(feature = "log_debug"))]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {};
}

#[cfg(any(feature = "log_info", feature = "log_debug"))]
#[macro_export]
macro_rules! info {
    ($($t:tt)*) => {
        tracing::info!($($t)*);
    };
}

#[cfg(any(feature = "log_warnings", feature = "log_debug"))]
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => {
        tracing::warn!($($t)*);
    };
}

#[cfg(feature = "log_debug")]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        tracing::debug!($($t)*);
    };
}

#[cfg(any(feature = "log_errors", feature = "log_debug"))]
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        tracing::error!($($t)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_logs_without_arg() {
        info!("Help me out");
        debug!("Help me out");
        warn!("Help me out");
        error!("Help me out");
    }

    #[test]
    #[traced_test]
    fn test_logs_with_arg() {
        info!("Help me out: {}", 1);
        debug!("Help me out: {}", 1);
        warn!("Help me out: {}", 1);
        error!("Help me out: {}", 1);
    }
}
