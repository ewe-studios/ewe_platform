#![doc = include_str!("../README.md")]

extern crate self as foundation_ui_runtime;

pub use foundation_macros::{
    action, html_add, html_add_after, html_add_before, html_remove, html_swap, html_view as html,
};
pub mod action;
pub mod dispatch;
pub mod state;
pub mod view;
