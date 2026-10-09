//! External-service tests — hit a real network service (OpenRouter).
//!
//! Gated behind `external-service-tests`. Each test self-skips (with an
//! eprintln) when its credential is unset, so a run without keys never fails.

mod openrouter;
