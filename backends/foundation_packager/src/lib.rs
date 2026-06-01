#![allow(dead_code)]

mod error;
mod files;
mod package;

pub mod tinytemplate;

pub(crate) use files::*;
pub use package::*;
