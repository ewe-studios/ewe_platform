//! Tests for WebView profiles and access gates.

use foundation_platform::*;

fn gate(profile: Profile) -> ProfileGate {
    ProfileGate::new(profile)
}

// ── App profile ──────────────────────────────────────────────

#[test]
fn app_allows_everything() {
    let g = gate(Profile::App);
    assert!(g.check(Service::Database, Access::Read).is_ok());
    assert!(g.check(Service::Database, Access::Write).is_ok());
    assert!(g.check(Service::Auth, Access::Read).is_ok());
    assert!(g.check(Service::NativeApi, Access::Execute).is_ok());
    assert!(g.check(Service::Http, Access::Read).is_ok());
    assert!(g.check(Service::TauriCommand, Access::Execute).is_ok());
}

// ── TrustedRemote profile ────────────────────────────────────

#[test]
fn trusted_remote_allows_db_read() {
    assert!(gate(Profile::TrustedRemote).check(Service::Database, Access::Read).is_ok());
}

#[test]
fn trusted_remote_denies_db_write() {
    assert!(gate(Profile::TrustedRemote).check(Service::Database, Access::Write).is_err());
}

#[test]
fn trusted_remote_allows_native_api() {
    // Per-route allowlisting is checked separately by the capability registry
    assert!(gate(Profile::TrustedRemote).check(Service::NativeApi, Access::Execute).is_ok());
}

// ── UntrustedRemote profile ──────────────────────────────────

#[test]
fn untrusted_remote_allows_only_http_read() {
    let g = gate(Profile::UntrustedRemote);
    assert!(g.check(Service::Http, Access::Read).is_ok());
    assert!(g.check(Service::Database, Access::Read).is_err());
    assert!(g.check(Service::Auth, Access::Read).is_err());
    assert!(g.check(Service::NativeApi, Access::Execute).is_err());
    assert!(g.check(Service::TauriCommand, Access::Execute).is_err());
}

// ── Auth profile ─────────────────────────────────────────────

#[test]
fn auth_allows_auth_apis() {
    assert!(gate(Profile::Auth).check(Service::Auth, Access::Read).is_ok());
    assert!(gate(Profile::Auth).check(Service::Auth, Access::Write).is_ok());
}

#[test]
fn auth_denies_database() {
    assert!(gate(Profile::Auth).check(Service::Database, Access::Read).is_err());
}

// ── Devtools ─────────────────────────────────────────────────

#[test]
fn devtools_allows_everything_in_debug() {
    let g = gate(Profile::Devtools);
    assert!(g.check(Service::Database, Access::Write).is_ok());
    assert!(g.check(Service::NativeApi, Access::Execute).is_ok());
}

// ── Default profile assignment ────────────────────────────────

#[test]
fn default_profiles_by_source() {
    assert_eq!(default_profile_for_source(RouteSource::WebviewApp), Profile::App);
    assert_eq!(default_profile_for_source(RouteSource::IpcShell), Profile::TrustedRemote);
    assert_eq!(default_profile_for_source(RouteSource::RemoteServer), Profile::TrustedRemote);
}
