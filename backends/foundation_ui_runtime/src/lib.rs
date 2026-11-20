#![doc = include_str!("../README.md")]

extern crate self as foundation_ui_runtime;

pub use foundation_macros::action;
pub mod action;
pub mod state;
