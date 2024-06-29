use tracing;

#[cfg(any(debug_trace))]
#[macro_export]
macro_rules! info {
    ( $($t:tt )* ) => {
        tracing::info!($($t)*);
    }
}

#[cfg(not(debug_trace))]
#[macro_export]
macro_rules! info {
    ( $($t:tt )* ) => {};
}

#[cfg(any(debug_trace))]
#[macro_export]
macro_rules! warn {
    ( $($t:tt )* ) => {
        tracing::warn!($($t)*);
    }
}

#[cfg(not(debug_trace))]
#[macro_export]
macro_rules! warn {
    ( $($t:tt )* ) => {};
}

#[cfg(any(debug_trace))]
#[macro_export]
macro_rules! debug {
    ( $($t:tt )* ) => {
        tracing::warn!($($t)*);
    }
}

#[cfg(not(debug_trace))]
#[macro_export]
macro_rules! debug {
    ( $($t:tt )* ) => {};
}

#[cfg(any(debug_trace))]
#[macro_export]
macro_rules! error {
    ( $($t:tt )* ) => {
        tracing::warn!($($t)*);
    }
}

#[cfg(not(debug_trace))]
#[macro_export]
macro_rules! error {
    ( $($t:tt )* ) => {};
}
