#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_trait_access)]

extern crate lazy_regex;
// extern crate lazy_static;

pub mod agentic;
pub mod backends;
pub mod costing;
pub mod errors;
pub mod models;
pub mod types;

#[cfg(feature = "toolbox")]
pub mod toolbox;
