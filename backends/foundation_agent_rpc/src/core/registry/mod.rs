//! The instance registry: whose instances a call sees, and the trait.
//!
//! WHY: an instance is a named agent conversation. Every host creates, finds,
//! lists and destroys instances and starts turns; only the storage differs.
//!
//! WHAT: [`Scope`] and [`ScopePolicy`] (the name space of a call), [`Caller`],
//! [`InstanceSpec`], [`InstanceInfo`], [`Page`], [`TurnEventStream`] and the
//! [`InstanceRegistry`] trait.
//!
//! HOW: a handler reads the checked [`Principal`] and the policy turns it into
//! a [`Caller`]. The registry methods take that caller, so the Worker can
//! separate clients and the native host can keep one shared name space.

pub mod local;

pub use local::{InMemoryStores, LocalInstanceRegistry, SessionStores};

use std::pin::Pin;

use base64::Engine;
use foundation_auth::client_auth::Principal;
use foundation_auth::fingerprint::sha256_hex;
use futures::Stream;

use crate::core::model::{AgentRpcFuture, Device, ModelKind, ModelRequest};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;

/// The name space that an instance name lives in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    /// One name space for every caller (native default, today's behavior).
    Shared,
    /// One name space per caller. The value is a Secure Hash Algorithm with
    /// 256 bits (SHA-256) hex of the principal subject, so names stay short
    /// and hide the subject.
    Owner(String),
}

impl Scope {
    /// The text that the store uses for this scope.
    #[must_use]
    pub fn as_key(&self) -> &str {
        match self {
            Self::Shared => "shared",
            Self::Owner(hash) => hash,
        }
    }
}

/// Which scope a call gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopePolicy {
    /// One name space for every caller.
    Shared,
    /// One name space per caller.
    PerOwner,
}

impl ScopePolicy {
    /// The scope of one principal.
    #[must_use]
    pub fn scope_for(self, principal: &Principal) -> Scope {
        match self {
            Self::Shared => Scope::Shared,
            Self::PerOwner => Scope::Owner(sha256_hex(principal.subject.as_bytes())),
        }
    }

    /// The registry caller for one call.
    #[must_use]
    pub fn caller(self, principal: Principal) -> Caller {
        Caller {
            scope: self.scope_for(&principal),
            principal,
        }
    }
}

/// Who calls the registry. Every registry method takes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    /// The name space of the call.
    pub scope: Scope,
    /// The checked identity. A Durable Object registry forwards it to the
    /// Durable Object. The local registry does not read it.
    pub principal: Principal,
}

/// The events of one turn, as wire messages.
///
/// Nothing is buffered: each event goes out when the model makes it.
pub type TurnEventStream = Pin<Box<dyn Stream<Item = AgentRpcResult<pb::TurnEvent>> + Send>>;

/// What `CreateInstance` asks for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstanceSpec {
    /// The instance name.
    pub name: String,
    /// The model request.
    pub model: ModelRequest,
    /// The system prompt for the session.
    pub system_prompt: Option<String>,
    /// The most tokens in one model call.
    pub max_tokens: Option<usize>,
    /// The sampling temperature.
    pub temperature: Option<f32>,
}

impl InstanceSpec {
    /// Read the wire request.
    ///
    /// # Errors
    /// [`AgentRpcError::ModelSelection`] for an unknown enum number, or for a
    /// kind that no host runs yet.
    pub fn from_wire(req: &pb::CreateInstanceRequest) -> AgentRpcResult<Self> {
        Ok(Self {
            name: req.name.clone(),
            model: ModelRequest::from_wire(req)?,
            system_prompt: req.system_prompt.clone(),
            max_tokens: req
                .max_tokens
                .map(|tokens| usize::try_from(tokens).unwrap_or(usize::MAX)),
            temperature: req.temperature,
        })
    }
}

/// What `ListInstances` and `CreateInstance` report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceInfo {
    /// The instance name.
    pub name: String,
    /// The model name.
    pub model: String,
    /// The model kind.
    pub kind: ModelKind,
    /// The model device.
    pub device: Device,
    /// When the instance was created (Unix milliseconds).
    pub created_unix_ms: u64,
    /// Turns started so far.
    pub turns: u64,
}

/// One page of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// The items of this page.
    pub items: Vec<T>,
    /// `None` means no more pages.
    pub next_token: Option<String>,
}

/// The page token of a name: unpadded base64url of the name.
///
/// The name is unique in a scope, so a name is a stable cursor.
#[must_use]
pub fn encode_page_token(name: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(name.as_bytes())
}

/// Read a page token. An empty token means the first page.
///
/// # Errors
/// [`AgentRpcError::Codec`] for text that is not unpadded base64url or not
/// valid Unicode.
pub fn decode_page_token(token: &str) -> AgentRpcResult<Option<String>> {
    if token.is_empty() {
        return Ok(None);
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|error| AgentRpcError::Codec(format!("page token: {error}")))?;
    let name = String::from_utf8(bytes)
        .map_err(|error| AgentRpcError::Codec(format!("page token: {error}")))?;
    Ok(Some(name))
}

/// Creates, finds and destroys instances, and starts turns.
pub trait InstanceRegistry: Send + Sync + 'static {
    /// Create one instance.
    ///
    /// # Errors
    /// `InvalidName`, `InstanceExists`, `InstanceLimit`, `ModelSelection`,
    /// `ModelLoad`, `Inference` (the session could not start).
    fn create<'a>(
        &'a self,
        caller: &'a Caller,
        spec: InstanceSpec,
    ) -> AgentRpcFuture<'a, InstanceInfo>;

    /// Find one instance.
    ///
    /// # Errors
    /// `InstanceNotFound`.
    fn get<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, InstanceInfo>;

    /// The instances of one scope, in name order.
    ///
    /// `page_size = 0` means the default.
    ///
    /// # Errors
    /// `Codec` for a bad page token.
    fn list<'a>(
        &'a self,
        caller: &'a Caller,
        page_size: u32,
        page_token: Option<&'a str>,
    ) -> AgentRpcFuture<'a, Page<InstanceInfo>>;

    /// Abort a running turn, end the session (flush its history), remove it.
    ///
    /// # Errors
    /// `InstanceNotFound`; `Inference` when ending the session fails (the
    /// instance is removed either way).
    fn destroy<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, ()>;

    /// Start one turn.
    ///
    /// The stream holds the instance's one-turn claim until it ends or drops.
    /// A stream dropped before its last event aborts the turn.
    ///
    /// # Errors
    /// `InstanceNotFound`; `InstanceBusy` (another turn runs: `ABORTED`);
    /// `Inference` when the turn cannot start.
    fn start_turn<'a>(
        &'a self,
        caller: &'a Caller,
        name: &'a str,
        prompt: String,
    ) -> AgentRpcFuture<'a, TurnEventStream>;

    /// How many instances the caller's scope holds.
    fn count<'a>(&'a self, caller: &'a Caller) -> AgentRpcFuture<'a, u64>;
}
