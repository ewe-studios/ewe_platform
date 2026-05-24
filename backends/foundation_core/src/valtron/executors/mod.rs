mod actions;
mod background;
mod builders;
mod collect_next;
mod collectors;
mod constants;
mod controller;
mod dependent_lift;
mod do_next;
mod drivers;
mod future_task;
mod local;
mod on_next;
mod state_machine;
mod task_iters;
mod unified;
mod wrappers;

pub use actions::*;
pub use background::*;
pub use builders::*;
pub use collect_next::*;
pub use collectors::*;
pub use constants::*;
pub use controller::*;
pub use dependent_lift::*;
pub use do_next::*;
pub use drivers::*;
pub use future_task::*;
pub use local::*;
pub use on_next::*;
pub use state_machine::*;
pub use task_iters::*;
pub use unified::*;
pub use wrappers::*;

pub mod single;

#[cfg(any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))]
pub mod wasm;

#[cfg(feature = "multi")]
pub mod multi;

// Re-export PoolGuard at executor level so tests can use valtron::PoolGuard
#[cfg(not(feature = "multi"))]
pub use single::PoolGuard;

#[cfg(feature = "multi")]
pub use multi::PoolGuard;

// re-exported external libraries
pub use rand::SeedableRng;
