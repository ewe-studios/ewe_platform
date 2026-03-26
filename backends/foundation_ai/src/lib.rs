#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

extern crate lazy_regex;
// extern crate lazy_static;

pub mod backends;
pub mod errors;
pub mod models;
pub mod types;
