//! Hand-written domain types: the error enum, and the few Hetzner shapes we care
//! about.
//!
//! **WHY:** the generated module has 100 structs — the closure of six endpoints.
//! Most are noise to a caller who wants "a server with this IP". These are the
//! ones the hand-written API speaks in.
//!
//! **WHAT:** [`HetznerError`], and the server/ssh-key shapes `server_ops` returns.
//!
//! **HOW:** deserialised from the generated types rather than re-parsing JSON —
//! the generated layer owns the wire format, this layer owns the vocabulary.

use serde::{Deserialize, Serialize};

/// Why a Hetzner operation failed.
///
/// The variants are split the way a **caller** must react, not the way the
/// failures arrive (decision 02 §3). "Your token is wrong", "the network is down"
/// and "you asked for a server type that does not exist" want three different
/// responses, and a deploy that dies on a typo'd token should say which it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HetznerError {
    /// No token in the environment.
    ///
    /// Names every variable that was tried — a "credentials not found" that does
    /// not say what it looked for is a scavenger hunt (decision 02 §2).
    NoCredentials {
        /// Every variable checked, in precedence order.
        looked_for: Vec<String>,
    },
    /// 401 — Hetzner rejected the token.
    ///
    /// Its own variant, never folded into [`HetznerError::Api`]: this one is
    /// almost always a setup mistake, and it should not read as an outage.
    Unauthorized,
    /// 429 — rate limited.
    ///
    /// Hetzner allows 3600/hour per project. Carries the hint from the
    /// `RateLimit-Reset` header when there is one, so a poll loop can back off
    /// rather than spin.
    RateLimited {
        /// Seconds until the limit resets, if the response said.
        retry_after_secs: Option<u64>,
    },
    /// The API answered, and said no.
    Api {
        /// HTTP status.
        status: u16,
        /// Hetzner's `error.code` — a stable string like `invalid_input`.
        code: String,
        /// Hetzner's `error.message`.
        message: String,
    },
    /// The request never got an answer.
    Transport(String),
    /// The answer did not parse.
    ///
    /// Distinct from [`HetznerError::Api`] on purpose: it means our generated
    /// types and Hetzner's spec disagree, which is a bug on our side, not theirs.
    Decode(String),
    /// A poll gave up.
    Timeout {
        /// What we were waiting for.
        waiting_for: String,
        /// How long we waited.
        after_secs: u64,
    },
}

impl std::fmt::Display for HetznerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCredentials { looked_for } => write!(
                f,
                "no Hetzner token: set one of {}",
                looked_for.join(" or ")
            ),
            Self::Unauthorized => write!(
                f,
                "Hetzner rejected the token (401) — check the token's value and that it has \
                 read+write on the project"
            ),
            Self::RateLimited { retry_after_secs } => match retry_after_secs {
                Some(secs) => write!(f, "Hetzner rate limit hit; resets in {secs}s"),
                None => write!(f, "Hetzner rate limit hit"),
            },
            Self::Api {
                status,
                code,
                message,
            } => write!(f, "Hetzner API error {status} ({code}): {message}"),
            Self::Transport(e) => write!(f, "transport error talking to Hetzner: {e}"),
            Self::Decode(e) => write!(f, "could not decode Hetzner's response: {e}"),
            Self::Timeout {
                waiting_for,
                after_secs,
            } => write!(f, "timed out after {after_secs}s waiting for {waiting_for}"),
        }
    }
}

impl std::error::Error for HetznerError {}

/// Where a server is in its lifecycle.
///
/// `POST /servers` returns while the server is still `Initializing` — the box is
/// not usable when the call returns, which is what `await_running` exists for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    /// Being built.
    Initializing,
    /// Booting.
    Starting,
    /// Up. Note this does **not** mean sshd is accepting connections.
    Running,
    /// Shutting down.
    Stopping,
    /// Powered off.
    Off,
    /// Being removed.
    Deleting,
    /// Being migrated between hosts.
    Migrating,
    /// Being rebuilt from an image.
    Rebuilding,
    /// Hetzner could not say.
    Unknown,
    /// A status this crate does not know.
    ///
    /// Not a parse failure: Hetzner adding a status should not break a running
    /// deploy that only cares whether we reached `Running`.
    #[serde(other)]
    Other,
}

impl ServerStatus {
    /// Whether the server has reached a state that will not change on its own.
    ///
    /// A poll loop that only watches for `Running` spins until timeout on a
    /// server that failed to build.
    #[must_use]
    pub fn is_settled(self) -> bool {
        matches!(self, Self::Running | Self::Off | Self::Unknown | Self::Other)
    }
}

/// A Hetzner server, reduced to what a deploy needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    /// Hetzner's id — what `get`/`delete` take.
    pub id: i64,
    /// The name we created it with. `deploy` finds an existing box by this.
    pub name: String,
    /// Lifecycle state.
    pub status: ServerStatus,
    /// The public IPv4 address, when it has one.
    ///
    /// `Option` because a server can be created without public IPv4 — and because
    /// it is absent from the create response until the action completes.
    pub public_ipv4: Option<String>,
}

/// An SSH key registered with Hetzner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshKey {
    /// Hetzner's id — what `create_server` references.
    pub id: i64,
    /// The name we registered it under.
    pub name: String,
    /// The key's fingerprint, as Hetzner computed it.
    pub fingerprint: String,
}

/// What to create.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreateServerRequest {
    /// The server's name — also how `deploy` finds it again.
    pub name: String,
    /// Hetzner's server type, e.g. `cx22`.
    pub server_type: String,
    /// Image to boot, e.g. `ubuntu-24.04`.
    pub image: String,
    /// Location, e.g. `nbg1`. `None` lets Hetzner choose.
    pub location: Option<String>,
    /// SSH key ids that may log in as root on first boot.
    pub ssh_keys: Vec<i64>,
    /// cloud-init user-data.
    ///
    /// Where hardening goes (feature 04): this runs **before first boot
    /// completes**, which is the only way to have a box that was never unhardened.
    pub user_data: Option<String>,
}
