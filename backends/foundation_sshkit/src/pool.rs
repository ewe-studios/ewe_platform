//! Connection pool for SSH sessions.

use crate::host::Host;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

        // Create new session
        let tcp = std::net::TcpStream::connect(format!("{}:{}", host.hostname, host.port))
            .map_err(|e| format!("connect: {e}"))?;
        tcp.set_read_timeout(Some(Duration::from_secs(10))).ok();

        let mut session = ssh2::Session::new()
            .map_err(|e| format!("session: {e}"))?;
        session.set_tcp_stream(tcp);
        session.handshake().map_err(|e| format!("handshake: {e}"))?;

        // Authenticate
        let user = &host.user;
        if let Some(ref pw) = host.password {
            session.userauth_password(user, pw).map_err(|e| format!("auth: {e}"))?;
        } else if let Some(first_key) = host.key_paths.first() {
            session.userauth_pubkey_file(user, None, first_key, None)
                .map_err(|e| format!("auth: {e}"))?;
        } else {
            // Try agent
            let mut agent = session.agent().map_err(|e| format!("agent: {e}"))?;
            agent.connect().map_err(|e| format!("agent connect: {e}"))?;
            agent.list_identities().map_err(|e| format!("agent list: {e}"))?;
            let identities = agent.identities().map_err(|e| format!("agent identities: {e}"))?;
            if let Some(identity) = identities.first() {
                agent.userauth(user, identity).map_err(|e| format!("agent auth: {e}"))?;
            } else {
                return Err("no auth method available".to_string());
            }
        }

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
