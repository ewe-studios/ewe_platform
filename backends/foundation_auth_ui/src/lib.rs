//! # foundation_auth_ui
//!
//! WHY: WASM UI components for the authentication flow — login, register, MFA,
//! password reset, account management, and passkey enrollment.
//!
//! WHAT: Page components built on `foundation_wasm_ui` (reactive `html!`,
//! signals, DOM ops over a wire) and `foundation_ui_components` (headless
//! primitives: input, button, dialog, tabs, etc.).
//!
//! HOW: Each page is a `fn(&Context, &SharedInstructionReceiver, ...) -> Html`
//! that self-mounts and drives API calls via `primal:on*` event handlers.

#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

pub mod login;
pub mod register;
pub mod password;
pub mod account;
pub mod api;
pub mod types;
pub mod layout;
