//! # `foundation_auth_ui`
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
//!
//! STATUS: only the request/response wire types (`types`) exist so far. The
//! page and API-client modules (layout, login, register, password, account,
//! api) are specified in
//! `specifications/48-foundation-auth-app/features/10-auth-ui-package` and are
//! added here as that feature is implemented.

#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_name_repetitions)]

extern crate alloc;

pub mod types;
