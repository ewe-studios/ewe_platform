use clap::Parser;
use std::fs::File;
use std::io::Read;
use std::path;

use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use ewe_watchers::{self};

type Result = std::io::Result<()>;

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    /// Name of config file
    #[arg(short, long, default_value = "watch.json")]
    config_file: String,

    #[arg(short, long, default_value = "800")]
    debounce: u64,
}

fn main() -> Result {
    let arg = Args::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let current_directory = std::env::current_dir().unwrap();
    let target_config = current_directory.join(arg.config_file);

    // let config = load_config(target_config.as_path()).expect("should load config");

    let watcher = ewe_watchers::watcher::Watchers::new();
    let config_watcher = ewe_watchers::watcher::ConfigWatcher::new(
        target_config.as_path().into(),
        watcher,
        arg.debounce,
    );

    config_watcher
        .listen()
        .expect("should have started watcher and config watching");

    // change some content
    // ctrlc::set_handler(move || {
    //     // close all watchers
    // })
    // .expect("error setting ctrl-c handler");

    Ok(())
}
