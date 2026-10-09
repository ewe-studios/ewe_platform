// Signal handling for dev server shutdown.

use std::sync::Arc;

use foundation_core::synca::OnSignal;

pub fn setup_ctrlc_handler(shutdown: Arc<OnSignal>) {
    ctrlc::set_handler(move || {
        tracing::info!("Received Ctrl+C, shutting down dev server");
        shutdown.turn_on();
    })
    .expect("Error setting Ctrl+C handler");
}
