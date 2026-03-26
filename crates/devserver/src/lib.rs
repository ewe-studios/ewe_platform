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

// Implements the core functionality to manage and serve a local
// ewe platform web application for local development.

mod body;
mod builders;
mod cargo;
mod core;
mod errors;
mod operators;
mod proxy;
mod sender_ext;
mod streams;
mod vec_ext;
mod watchers;

pub mod assets;
pub mod types;

pub use body::*;
pub use builders::*;
pub use cargo::*;
pub use core::*;
pub use errors::*;
pub use operators::*;
pub use proxy::*;
pub use sender_ext::*;
pub use vec_ext::*;
pub use watchers::*;

// re-export core type without types module
