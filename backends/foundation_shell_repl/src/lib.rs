mod shared;

#[cfg(all(feature = "native", not(feature = "wasm")))]
mod native;

#[cfg(feature = "wasm")]
mod wasm;

pub use shared::config::ReplConfig;
#[cfg(feature = "colors")]
pub use shared::config::ReplColors;
pub use shared::commands::{CommandRegistry, CommandResult};
pub use shared::repl::{Repl, ReplBuilder, ReplMessageIter};
pub use shared::traits::{ReplDisplay, ReplInput};
