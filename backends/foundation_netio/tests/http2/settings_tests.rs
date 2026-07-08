//! Tests for `http2::settings` — SETTINGS store with RFC 7540 defaults,
//! validation, change-tracking, non-default frame building (Feature 29).

use foundation_netio::http2::frame::{Setting, SettingId};
use foundation_netio::http2::settings::*;

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
    assert!(s.set(SettingId::MaxFrameSize, 16383).is_err());
    assert!(s.set(SettingId::MaxFrameSize, 16777216).is_err());
}

#[test]
fn initial_window_size_validation() {
    let mut s = SettingsStore::default();
    assert!(s.set(SettingId::InitialWindowSize, 2147483647).is_ok());
    assert!(s.set(SettingId::InitialWindowSize, 2147483648).is_err());
}

#[test]
fn apply_tracks_changes() {
    let mut s = SettingsStore::default();
    let changes = s.apply(&[
        Setting { id: SettingId::MaxConcurrentStreams, value: 100 },
        Setting { id: SettingId::InitialWindowSize, value: 65535 },
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
