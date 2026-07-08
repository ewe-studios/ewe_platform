//! HTTP/2 SETTINGS negotiation state (RFC 7540 §6.5).
//!
//! WHY: Each endpoint maintains its own SETTINGS values and must synchronise
//! changes via the SETTINGS+ACK handshake. This module tracks the local and
//! remote settings plus the ACK-pending state machine.
//!
//! WHAT: [`Settings`] stores both local (our) and remote (peer) values with
//! RFC 7540 defaults. It validates settings on receipt and tracks whether a
//! SETTINGS frame is pending acknowledgement.
//!
//! HOW: Pure state machine — no I/O. Callers apply incoming SETTINGS frames
//! and query current values.

use crate::http2::frame::{Setting, SettingId, SettingsFrame};

/// RFC 7540 §6.5.2 default values.
pub const DEFAULT_HEADER_TABLE_SIZE: u32 = 4096;
pub const DEFAULT_ENABLE_PUSH: u32 = 1;
pub const DEFAULT_MAX_CONCURRENT_STREAMS: u32 = u32::MAX; // "unlimited"
pub const DEFAULT_INITIAL_WINDOW_SIZE: u32 = 65_535;
pub const DEFAULT_MAX_FRAME_SIZE: u32 = 16_384;
pub const DEFAULT_MAX_HEADER_LIST_SIZE: u32 = u32::MAX; // "unlimited"

/// Validation limits.
const MAX_INITIAL_WINDOW_SIZE: u32 = 2_147_483_647; // 2^31 - 1
const MIN_MAX_FRAME_SIZE: u32 = 16_384;
const MAX_MAX_FRAME_SIZE: u32 = 16_777_215;

/// Whether we've sent our SETTINGS and are waiting for an ACK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsState {
    /// No SETTINGS sent yet, or ACK received — ready to send.
    Ready,
    /// SETTINGS sent, waiting for ACK.
    PendingAck,
}

/// Endpoint-specific SETTINGS store.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    header_table_size: u32,
    enable_push: u32,
    max_concurrent_streams: u32,
    initial_window_size: u32,
    max_frame_size: u32,
    max_header_list_size: u32,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self {
            header_table_size: DEFAULT_HEADER_TABLE_SIZE,
            enable_push: DEFAULT_ENABLE_PUSH,
            max_concurrent_streams: DEFAULT_MAX_CONCURRENT_STREAMS,
            initial_window_size: DEFAULT_INITIAL_WINDOW_SIZE,
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            max_header_list_size: DEFAULT_MAX_HEADER_LIST_SIZE,
        }
    }
}

impl SettingsStore {
    /// Apply an incoming SETTINGS frame's parameters, validating each one.
    ///
    /// # Errors
    /// Returns an error string if any value is out of bounds.
    pub fn apply(&mut self, settings: &[Setting]) -> Result<Vec<SettingChange>, &'static str> {
        let mut changes = Vec::new();
        for s in settings {
            let old = self.get(s.id);
            self.set(s.id, s.value)?;
            if old != s.value {
                changes.push(SettingChange { id: s.id, old, new: s.value });
            }
        }
        Ok(changes)
    }

    /// Get a setting value.
    #[must_use]
    pub fn get(&self, id: SettingId) -> u32 {
        match id {
            SettingId::HeaderTableSize => self.header_table_size,
            SettingId::EnablePush => self.enable_push,
            SettingId::MaxConcurrentStreams => self.max_concurrent_streams,
            SettingId::InitialWindowSize => self.initial_window_size,
            SettingId::MaxFrameSize => self.max_frame_size,
            SettingId::MaxHeaderListSize => self.max_header_list_size,
        }
    }

    /// Set a setting value with validation.
    ///
    /// # Errors
    /// Returns an error string if the value is out of bounds.
    pub fn set(&mut self, id: SettingId, value: u32) -> Result<(), &'static str> {
        match id {
            SettingId::HeaderTableSize => {
                // No upper bound in RFC, but 0 is valid (clear table).
                self.header_table_size = value;
            }
            SettingId::EnablePush => {
                if value > 1 {
                    return Err("SETTINGS_ENABLE_PUSH must be 0 or 1");
                }
                self.enable_push = value;
            }
            SettingId::MaxConcurrentStreams => {
                // No explicit bound — any u32 is valid.
                self.max_concurrent_streams = value;
            }
            SettingId::InitialWindowSize => {
                if value > MAX_INITIAL_WINDOW_SIZE {
                    return Err("SETTINGS_INITIAL_WINDOW_SIZE exceeds 2^31-1");
                }
                self.initial_window_size = value;
            }
            SettingId::MaxFrameSize => {
                if value < MIN_MAX_FRAME_SIZE || value > MAX_MAX_FRAME_SIZE {
                    return Err("SETTINGS_MAX_FRAME_SIZE out of range [16384, 16777215]");
                }
                self.max_frame_size = value;
            }
            SettingId::MaxHeaderListSize => {
                // No explicit bound.
                self.max_header_list_size = value;
            }
        }
        Ok(())
    }

    /// Build a SETTINGS frame containing our current local values.
    #[must_use]
    pub fn to_frame(&self) -> SettingsFrame {
        SettingsFrame::new(vec![
            Setting { id: SettingId::HeaderTableSize, value: self.header_table_size },
            Setting { id: SettingId::EnablePush, value: self.enable_push },
            Setting { id: SettingId::MaxConcurrentStreams, value: self.max_concurrent_streams },
            Setting { id: SettingId::InitialWindowSize, value: self.initial_window_size },
            Setting { id: SettingId::MaxFrameSize, value: self.max_frame_size },
            Setting { id: SettingId::MaxHeaderListSize, value: self.max_header_list_size },
        ])
    }

    /// Build a SETTINGS frame containing only values that differ from defaults.
    #[must_use]
    pub fn to_frame_non_default(&self) -> SettingsFrame {
        let mut settings = Vec::new();
        self.push_if_non_default(&mut settings, SettingId::HeaderTableSize, self.header_table_size, DEFAULT_HEADER_TABLE_SIZE);
        self.push_if_non_default(&mut settings, SettingId::EnablePush, self.enable_push, DEFAULT_ENABLE_PUSH);
        self.push_if_non_default(&mut settings, SettingId::MaxConcurrentStreams, self.max_concurrent_streams, DEFAULT_MAX_CONCURRENT_STREAMS);
        self.push_if_non_default(&mut settings, SettingId::InitialWindowSize, self.initial_window_size, DEFAULT_INITIAL_WINDOW_SIZE);
        self.push_if_non_default(&mut settings, SettingId::MaxFrameSize, self.max_frame_size, DEFAULT_MAX_FRAME_SIZE);
        self.push_if_non_default(&mut settings, SettingId::MaxHeaderListSize, self.max_header_list_size, DEFAULT_MAX_HEADER_LIST_SIZE);
        SettingsFrame::new(settings)
    }

    fn push_if_non_default(&self, v: &mut Vec<Setting>, id: SettingId, value: u32, default: u32) {
        if value != default {
            v.push(Setting { id, value });
        }
    }
}

/// A change to a SETTINGS parameter (for flow-control / HPACK updates).
#[derive(Debug, Clone, Copy)]
pub struct SettingChange {
    pub id: SettingId,
    pub old: u32,
    pub new: u32,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_rfc() {
        let s = SettingsStore::default();
        assert_eq!(s.get(SettingId::HeaderTableSize), 4096);
        assert_eq!(s.get(SettingId::EnablePush), 1);
        assert_eq!(s.get(SettingId::InitialWindowSize), 65535);
        assert_eq!(s.get(SettingId::MaxFrameSize), 16384);
    }

    #[test]
    fn enable_push_validation() {
        let mut s = SettingsStore::default();
        assert!(s.set(SettingId::EnablePush, 0).is_ok());
        assert!(s.set(SettingId::EnablePush, 1).is_ok());
        assert!(s.set(SettingId::EnablePush, 2).is_err());
    }

    #[test]
    fn max_frame_size_validation() {
        let mut s = SettingsStore::default();
        assert!(s.set(SettingId::MaxFrameSize, 16384).is_ok());
        assert!(s.set(SettingId::MaxFrameSize, 16777215).is_ok());
        assert!(s.set(SettingId::MaxFrameSize, 16383).is_err()); // below min
        assert!(s.set(SettingId::MaxFrameSize, 16777216).is_err()); // above max
    }

    #[test]
    fn initial_window_size_validation() {
        let mut s = SettingsStore::default();
        assert!(s.set(SettingId::InitialWindowSize, 2147483647).is_ok());
        assert!(s.set(SettingId::InitialWindowSize, 2147483648).is_err()); // > 2^31-1
    }

    #[test]
    fn apply_tracks_changes() {
        let mut s = SettingsStore::default();
        let changes = s.apply(&[
            Setting { id: SettingId::MaxConcurrentStreams, value: 100 },
            Setting { id: SettingId::InitialWindowSize, value: 65535 }, // unchanged
        ]).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, SettingId::MaxConcurrentStreams);
    }

    #[test]
    fn to_frame_roundtrip() {
        let mut s = SettingsStore::default();
        s.set(SettingId::MaxConcurrentStreams, 50).unwrap();
        s.set(SettingId::InitialWindowSize, 128000).unwrap();

        let frame = s.to_frame();
        let mut s2 = SettingsStore::default();
        s2.apply(&frame.settings).unwrap();
        assert_eq!(s2.get(SettingId::MaxConcurrentStreams), 50);
        assert_eq!(s2.get(SettingId::InitialWindowSize), 128000);
    }
}
