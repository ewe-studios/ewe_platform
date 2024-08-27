/// Crate to abstract out tracing so it never shows up in release builds using macros
/// See similar: https://doc.rust-lang.org/src/std/macros.rs.html#138-145.

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => {
        if cfg!(feature="log_info") {
            tracing::info!($($t)*);
        } else {
			// do nothing;
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => {
        if cfg!(feature="log_warnings") {
            tracing::warn!($($t)*);
        } else {
			// do nothing;
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        if cfg!(feature="log_debug") {
            tracing::debug!($($t)*);
        } else {
			// do nothing;
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        if cfg!(feature="log_errors") {
            tracing::error!($($t)*);
        } else {
			// do nothing;
        }
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
