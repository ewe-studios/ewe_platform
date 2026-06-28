#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::default_trait_access)]

#![allow(clippy::items_after_statements)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::single_match_else)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
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
