

// #[macro]
// extern crate serde_derive;
// extern crate serde_json;

use std::path::Path;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use watchers::{self};

type Result = std::io::Result<()>;

fn main() -> Result {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let current_directory = std::env::current_dir().unwrap();
    let target_directory = current_directory.join(Path::new("crates"));

    // let file_watcher = watchers::FileWatcher::new(target_directory.clone());
    // println!("Hello, world in {:?}!", file_watcher);

    watchers::file_watcher::watch(target_directory.as_path())
        .expect("watch function registered directory");

    // change some content

    Ok(())
}
