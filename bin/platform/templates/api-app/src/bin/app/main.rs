use foundation_core::valtron::valtron;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[valtron(seed = 100)]
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing::info!("Starting local binary");
}
