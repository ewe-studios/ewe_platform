#[allow(unused_macros)]
#[cfg(all(target_arch = "wasm"))]
pub mod logging {

    macro_rules! warn {
        ( $( $t:tt )* ) => {
            web_sys::console::warn(&format!( $( $t )* ).into());
        }
    }

    macro_rules! error {
        ( $( $t:tt )* ) => {
            web_sys::console::error_1(&format!( $( $t )* ).into());
        }
    }

    macro_rules! log {
        ( $( $t:tt )* ) => {
            web_sys::console::log_1(&format!( $( $t )* ).into());
        }
    }
}

#[cfg(all(not(target_arch = "wasm")))]
#[allow(unused_macros)]
pub mod logging {

    macro_rules! warn {
        ( $( $t:tt )* ) => {
            tracing::warn(&format!( $( $t )* ).into());
        }
    }

    macro_rules! error {
        ( $( $t:tt )* ) => {
            tracing::error(&format!( $( $t )* ).into());
        }
    }

    macro_rules! log {
        ( $( $t:tt )* ) => {
            tracing::info(&format!( $( $t )* ).into());
        }
    }
}
