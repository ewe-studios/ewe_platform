//! The Cloudflare part of the platform.
//!
//! Feature 07 fills this module with the container control, the platform
//! wrapper and the host bundle.
//!
//! # Nested sandboxes (feature 11, section 6.6)
//!
//! A Cloudflare container has no `/dev/kvm`: the platform does not offer nested
//! virtualisation. The Firecracker probe of the inner control plane therefore
//! reports the missing device, and the slave reports `can_control: false`. A
//! caller that creates a child of that slave gets `DriverUnavailable`
//! (invariant I2). The code path is the same as on a native host; only the
//! probe result differs. If Cloudflare adds the capability, no code changes.

pub mod platform;
