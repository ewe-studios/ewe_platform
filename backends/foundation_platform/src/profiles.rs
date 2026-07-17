//! WebView profiles and access gates.
//!
//! Five profiles gate access to platform services at runtime. Every
//! platform service call checks the active profile before executing.
//! Profiles are assigned per-route in `RouteDecision.profile`.

use foundation_ui_traits::*;

// ── Service taxonomy ─────────────────────────────────────────────────

/// Platform services gated by profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Database,
    Auth,
    NativeApi,
    Http,
    Arrow,
    Signals,
    TauriCommand,
    TauriEvent,
    CustomProtocol,
}

/// Access level requested for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Execute,
}

// ── Profile gate ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    AccessDenied,
    StrippedInProduction,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::AccessDenied => write!(f, "access denied for profile"),
            ProfileError::StrippedInProduction => write!(f, "devtools profile stripped in production"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// Enforces profile-based access to platform services.
pub struct ProfileGate {
    profile: Profile,
}

impl ProfileGate {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

    /// Check whether the active profile permits the given service access.
    /// Returns `Ok(())` if allowed, `Err(ProfileError)` if denied.
    pub fn check(&self, service: Service, access: Access) -> Result<(), ProfileError> {
        use Access::{Execute, Read, Write};
        use Service::{
            Arrow, Auth as SvcAuth, CustomProtocol, Database, Http,
            NativeApi, Signals, TauriCommand, TauriEvent,
        };

        match self.profile {
            Profile::App => Ok(()),

            Profile::TrustedRemote => match (service, access) {
                (Database, Read) => Ok(()),
                (SvcAuth, Read) => Ok(()),
                (NativeApi, Execute) => Ok(()),
                (Http, Read | Write) => Ok(()),
                (Arrow, Read) => Ok(()),
                (Signals, Read) => Ok(()),
                (TauriCommand, Execute) => Ok(()),
                (TauriEvent, Read) => Ok(()),
                (CustomProtocol, Read) => Ok(()),
                _ => Err(ProfileError::AccessDenied),
            },

            Profile::UntrustedRemote => match (service, access) {
                (Http, Read) => Ok(()),
                _ => Err(ProfileError::AccessDenied),
            },

            Profile::Auth => match (service, access) {
                (SvcAuth, _) => Ok(()),
                (NativeApi, Execute) => Ok(()),
                (Http, Read) => Ok(()),
                _ => Err(ProfileError::AccessDenied),
            },

            Profile::Devtools => {
                #[cfg(debug_assertions)]
                { Ok(()) }
                #[cfg(not(debug_assertions))]
                { Err(ProfileError::StrippedInProduction) }
            }
        }
    }

    pub fn profile(&self) -> Profile {
        self.profile
    }
}

// ── Default profile assignment ────────────────────────────────────────

/// Default profile for a given route source, if no explicit profile
/// is set in the RouteDecision.
pub fn default_profile_for_source(source: RouteSource) -> Profile {
    match source {
        RouteSource::WebviewApp => Profile::App,
        RouteSource::IpcShell => Profile::TrustedRemote,
        RouteSource::RemoteServer => Profile::TrustedRemote,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(super::default_profile_for_source(RouteSource::WebviewApp), Profile::App);
        assert_eq!(super::default_profile_for_source(RouteSource::IpcShell), Profile::TrustedRemote);
        assert_eq!(super::default_profile_for_source(RouteSource::RemoteServer), Profile::TrustedRemote);
    }
}
