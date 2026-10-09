//! Connection pool for SSH sessions.

use crate::host::Host;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ssh2::{CheckResult, KnownHostFileKind, KnownHostKeyFormat, Session};

/// A pool of SSH sessions, keyed by (hostname, port, user).
/// Idle sessions are evicted after `idle_timeout`.
pub struct ConnectionPool {
    sessions: Mutex<HashMap<String, (Instant, ssh2::Session)>>,
    idle_timeout: Duration,
}

impl ConnectionPool {
    pub fn new(idle_timeout: Duration) -> Self {
        Self { sessions: Mutex::new(HashMap::new()), idle_timeout }
    }

    fn key(host: &Host) -> String {
        format!("{}@{}:{}", host.user, host.hostname, host.port)
    }

    /// Get or create a session for the given host.
    pub fn get(&self, host: &Host) -> Result<ssh2::Session, String> {
        let key = Self::key(host);
        let mut sessions = self.sessions.lock().map_err(|e| format!("lock: {e}"))?;

        if let Some((_ts, session)) = sessions.get(&key) {
            if session.authenticated() {
                return Ok(session.clone()); // ssh2::Session is Clone (Arc internally)
            }
        }

        let session = connect_session(host)?;
        sessions.insert(key.clone(), (Instant::now(), session.clone()));
        Ok(session)
    }

    /// Evict idle sessions.
    pub fn evict_idle(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.retain(|_, (ts, _)| ts.elapsed() < self.idle_timeout);
        }
    }
}

/// Establish a fresh, authenticated ssh2 session to `host`.
///
/// WHY: Both the pool (`ConnectionPool::get`) and the pool-free channel dialer
/// ([`crate::backends::ssh2::Ssh2Backend::dial`]) need the same connect +
/// handshake + auth sequence. `dial` deliberately does **not** share pooled
/// sessions — a blocking read on one ssh2 `Channel` blocks every other object
/// derived from the same `Session` (they share one internal mutex), so a
/// long-lived duplex (e.g. `docker system dial-stdio`) must own its session.
///
/// WHAT: TCP-connects, runs the SSH handshake, verifies the server's host key
/// against `~/.ssh/known_hosts`, then authenticates. Authentication follows the
/// OpenSSH order: an explicit password if set, else the SSH **agent** (every
/// identity), else key files — the host's explicit [`key_paths`](Host::key_paths)
/// if any, otherwise the default `~/.ssh/id_*` keys.
///
/// # Errors
///
/// Returns a message if the TCP connect, handshake, host-key check, or
/// authentication fails (including when no auth method succeeds).
pub fn connect_session(host: &Host) -> Result<Session, String> {
    let tcp = std::net::TcpStream::connect(format!("{}:{}", host.hostname, host.port))
        .map_err(|e| format!("connect: {e}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(10))).ok();

    let mut session = Session::new().map_err(|e| format!("session: {e}"))?;
    session.set_tcp_stream(tcp);
    session.handshake().map_err(|e| format!("handshake: {e}"))?;

    verify_host_key(&session, host)?;
    authenticate(&session, host)?;

    Ok(session)
}

/// Verify the server's host key against `~/.ssh/known_hosts` (TOFU).
///
/// WHY: Without this the transport is exposed to a man-in-the-middle — libssh2
/// does no host-key checking on its own. Matches OpenSSH's
/// `StrictHostKeyChecking=accept-new`: a known-and-matching key passes, an
/// unknown host is trusted-on-first-use and appended, and a **changed** key is
/// rejected (the MITM signal).
///
/// Best-effort on the filesystem: a missing `~/.ssh` / `known_hosts` is fine
/// (treated as first use); an unwritable file only forgoes persistence.
fn verify_host_key(session: &Session, host: &Host) -> Result<(), String> {
    let Some(ssh_dir) = dirs::home_dir().map(|h| h.join(".ssh")) else {
        // No home directory to anchor known_hosts — cannot verify; skip.
        return Ok(());
    };
    let kh_path = ssh_dir.join("known_hosts");

    let mut known = session
        .known_hosts()
        .map_err(|e| format!("known_hosts init: {e}"))?;
    // A missing file just means "no known hosts yet".
    let _ = known.read_file(&kh_path, KnownHostFileKind::OpenSSH);

    let (key, key_type) = session
        .host_key()
        .ok_or("server presented no host key")?;

    match known.check_port(&host.hostname, host.port, key) {
        CheckResult::Match => Ok(()),
        CheckResult::Mismatch => {
            // Security-relevant: log directly so the reason is visible even if a
            // caller flattens the returned error.
            tracing::error!(
                host = %host.hostname,
                port = host.port,
                known_hosts = %kh_path.display(),
                "SSH host key MISMATCH — possible MITM; refusing connection"
            );
            Err(format!(
                "host key mismatch for {}:{} — possible MITM; refusing. If the host \
                 legitimately changed, remove its entry from {}",
                host.hostname,
                host.port,
                kh_path.display()
            ))
        }
        CheckResult::NotFound => {
            // Trust on first use: record the key and persist it.
            let fmt = KnownHostKeyFormat::from(key_type);
            if matches!(fmt, KnownHostKeyFormat::Unknown) {
                return Err(format!(
                    "server host key type for {} is unrecognized; refusing",
                    host.hostname
                ));
            }
            // known_hosts encodes non-default ports as "[host]:port".
            let entry = if host.port == 22 {
                host.hostname.clone()
            } else {
                format!("[{}]:{}", host.hostname, host.port)
            };
            known
                .add(&entry, key, "added by foundation_sshkit", fmt)
                .map_err(|e| format!("known_hosts add: {e}"))?;
            let _ = std::fs::create_dir_all(&ssh_dir);
            let _ = known.write_file(&kh_path, KnownHostFileKind::OpenSSH);
            tracing::info!(host = %host.hostname, port = host.port, "TOFU: added new host key to known_hosts");
            Ok(())
        }
        CheckResult::Failure => Err("known_hosts check failed".to_string()),
    }
}

/// Authenticate `session` for `host`, trying methods in OpenSSH order.
fn authenticate(session: &Session, host: &Host) -> Result<(), String> {
    let user = &host.user;

    if let Some(pw) = &host.password {
        return session
            .userauth_password(user, pw)
            .map_err(|e| format!("password auth: {e}"));
    }

    // 1) SSH agent — try every identity it holds.
    if authenticate_via_agent(session, user) {
        return Ok(());
    }

    // 2) Key files: the host's explicit keys, else the default `~/.ssh/id_*`.
    let keys = if host.key_paths.is_empty() {
        default_identity_files()
    } else {
        host.key_paths.clone()
    };
    for key in &keys {
        if session
            .userauth_pubkey_file(user, None, key, None)
            .is_ok()
        {
            return Ok(());
        }
    }

    if session.authenticated() {
        return Ok(());
    }
    Err(format!(
        "no working SSH authentication for {user}@{}:{} (tried agent + {} key file(s))",
        host.hostname,
        host.port,
        keys.len()
    ))
}

/// Try each identity the SSH agent holds; `true` on the first that authenticates.
fn authenticate_via_agent(session: &Session, user: &str) -> bool {
    let Ok(mut agent) = session.agent() else {
        return false;
    };
    if agent.connect().is_err() || agent.list_identities().is_err() {
        return false;
    }
    let Ok(identities) = agent.identities() else {
        return false;
    };
    identities
        .iter()
        .any(|id| agent.userauth(user, id).is_ok())
}

/// The default identity files OpenSSH tries, in preference order, that exist.
fn default_identity_files() -> Vec<PathBuf> {
    let Some(ssh_dir) = dirs::home_dir().map(|h| h.join(".ssh")) else {
        return Vec::new();
    };
    ["id_ed25519", "id_ecdsa", "id_rsa", "id_dsa"]
        .iter()
        .map(|name| ssh_dir.join(name))
        .filter(|p| p.exists())
        .collect()
}
