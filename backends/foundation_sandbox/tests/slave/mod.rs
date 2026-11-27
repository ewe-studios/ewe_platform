//! The slave process tests. Each test starts the real slave as a normal
//! process on a Unix socket and calls it with the generated client.

#[cfg(feature = "control")]
mod control;
mod exec;
mod files;
mod info;
mod input;
mod lifecycle;
mod root;
mod support;
mod unpack;
