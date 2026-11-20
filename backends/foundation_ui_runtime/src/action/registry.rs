//! Typed actions and the registry that selects them by origin and surface.

use super::Context;
use foundation_errstacks::ErrorTrace;
use foundation_ui_wire::{ErrorCode, ServerActionT};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{any::TypeId, collections::BTreeMap, fmt, future::Future, pin::Pin};

pub use foundation_ui_wire::MountSurface as Surface;

/// The runtime that receives the action request.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Origin {
    Host,
    Server,
}

/// The surfaces that support one declaration.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Scope {
    All,
    Web,
    Mobile,
}

impl Scope {
    #[must_use]
    pub fn supports(self, surface: Surface) -> bool {
        match surface {
            Surface::Web => matches!(self, Self::All | Self::Web),
            Surface::Native => matches!(self, Self::All | Self::Mobile),
            _ => false,
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self == other || self == Self::All || other == Self::All
    }
}

/// An action reference from an event binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionRef<'a> {
    pub origin: Origin,
    pub name: &'a str,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(|c: char| c == ':' || c.is_control() || c.is_whitespace())
}

impl<'a> ActionRef<'a> {
    /// Reads a reference such as `host:add_todo`.
    ///
    /// # Errors
    /// Returns an error if the prefix or name is invalid.
    pub fn parse(reference: &'a str) -> Result<Self, LookupError> {
        let (prefix, name) = reference
            .split_once(':')
            .ok_or(LookupError::InvalidReference)?;
        if !valid_name(name) {
            return Err(LookupError::InvalidReference);
        }
        let origin = match prefix {
            "host" => Origin::Host,
            "server" => Origin::Server,
            _ => return Err(LookupError::InvalidReference),
        };
        Ok(Self { origin, name })
    }
}

/// The input and decoded component state for one action.
#[derive(Debug)]
pub struct Interaction<T> {
    pub node_id: u32,
    pub event_name: String,
    pub input_value: String,
    pub state: T,
}

/// The error code and host message for an action failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionError {
    pub code: ErrorCode,
    pub message: String,
}

impl ActionError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ActionError {}

pub type ActionResult = Result<(), ErrorTrace<ActionError>>;

#[cfg(not(target_family = "wasm"))]
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = ActionResult> + Send + 'a>>;

#[cfg(target_family = "wasm")]
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = ActionResult> + 'a>>;

/// The action macro implements this trait for the author's function.
pub trait Action: Send + Sync + 'static {
    type State: Serialize + DeserializeOwned + Send + 'static;
    const NAME: &'static str;
    fn origin() -> Option<Origin>;
    fn scope() -> Scope;
    fn call<'a>(&'a self, ctx: &'a Context, event: Interaction<Self::State>) -> ActionFuture<'a>;
}

/// The dispatcher supplies these fields after it verifies the signed bytes.
pub struct RawInteraction {
    pub node_id: u32,
    pub event_name: String,
    pub input_value: String,
    pub state_body: Option<Vec<u8>>,
}

/// A registered Rust action with its concrete state type hidden.
pub trait ErasedAction: Send + Sync {
    fn name(&self) -> &'static str;
    fn origin(&self) -> Option<Origin>;
    fn scope(&self) -> Scope;
    fn requires_state(&self) -> bool;
    fn call_raw<'a>(&'a self, ctx: &'a Context, event: RawInteraction) -> ActionFuture<'a>;
}

struct Adapter<A>(A);

impl<A: Action> ErasedAction for Adapter<A> {
    fn name(&self) -> &'static str {
        A::NAME
    }
    fn origin(&self) -> Option<Origin> {
        A::origin()
    }
    fn scope(&self) -> Scope {
        A::scope()
    }
    fn requires_state(&self) -> bool {
        TypeId::of::<A::State>() != TypeId::of::<()>()
    }

    fn call_raw<'a>(&'a self, ctx: &'a Context, event: RawInteraction) -> ActionFuture<'a> {
        Box::pin(async move {
            let body = match event.state_body.as_deref() {
                Some(bytes) => bytes,
                None if !self.requires_state() => b"null",
                None => {
                    return Err(ErrorTrace::new(ActionError::new(
                        ErrorCode::StateRejected,
                        "The event has no component state",
                    )))
                }
            };
            let state = serde_json::from_slice(body).map_err(|error| {
                ErrorTrace::new(ActionError::new(
                    ErrorCode::StateRejected,
                    "Cannot decode component state",
                ))
                .attach(error)
            })?;
            self.0
                .call(
                    ctx,
                    Interaction {
                        node_id: event.node_id,
                        event_name: event.event_name,
                        input_value: event.input_value,
                        state,
                    },
                )
                .await
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupError {
    InvalidReference,
    UnknownAction,
    UnsupportedAction,
    UnavailableOrigin,
}

impl LookupError {
    #[must_use]
    pub fn code(self) -> ErrorCode {
        match self {
            Self::InvalidReference => ErrorCode::BadRequest,
            Self::UnknownAction => ErrorCode::UnknownAction,
            Self::UnsupportedAction => ErrorCode::UnsupportedAction,
            Self::UnavailableOrigin => ErrorCode::UnavailableOrigin,
        }
    }
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LookupError {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    InvalidName,
    Duplicate,
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RegistrationError {}

/// Metadata used by the action manager to choose a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration {
    pub origin: Option<Origin>,
    pub scope: Scope,
    pub local: bool,
}

enum Implementation {
    Absent,
    Page,
    Rust(Box<dyn ErasedAction>),
}

struct Entry {
    origin: Option<Origin>,
    scope: Scope,
    implementation: Implementation,
}

/// Stores declarations and callable actions for a host.
#[derive(Default)]
pub struct ActionRegistry {
    entries: BTreeMap<String, Vec<Entry>>,
}

impl ActionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the Rust implementation for a declaration.
    ///
    /// # Errors
    /// Rejects an invalid name or an overlapping registration.
    pub fn register<A: Action>(&mut self, action: A) -> Result<(), RegistrationError> {
        self.insert(
            A::origin(),
            A::NAME,
            A::scope(),
            Implementation::Rust(Box::new(Adapter(action))),
        )
    }

    /// Registers a function provided by the page script transport.
    ///
    /// # Errors
    /// Rejects an invalid name or an overlapping registration.
    pub fn register_page(&mut self, name: &str) -> Result<(), RegistrationError> {
        self.insert(Some(Origin::Host), name, Scope::Web, Implementation::Page)
    }

    /// Declares an action whose implementation can live on another host.
    ///
    /// # Errors
    /// Rejects an invalid name or an overlapping declaration.
    pub fn declare(
        &mut self,
        origin: Option<Origin>,
        name: &str,
        scope: Scope,
    ) -> Result<(), RegistrationError> {
        self.insert(origin, name, scope, Implementation::Absent)
    }

    fn insert(
        &mut self,
        origin: Option<Origin>,
        name: &str,
        scope: Scope,
        implementation: Implementation,
    ) -> Result<(), RegistrationError> {
        if !valid_name(name) {
            return Err(RegistrationError::InvalidName);
        }
        let entries = self.entries.entry(name.to_owned()).or_default();
        for entry in entries.iter_mut() {
            let same_origin = origin == entry.origin || origin.is_none() || entry.origin.is_none();
            if same_origin && scope.overlaps(entry.scope) {
                if origin == entry.origin
                    && scope == entry.scope
                    && matches!(entry.implementation, Implementation::Absent)
                    && !matches!(implementation, Implementation::Absent)
                {
                    entry.implementation = implementation;
                    return Ok(());
                }
                return Err(RegistrationError::Duplicate);
            }
        }
        entries.push(Entry {
            origin,
            scope,
            implementation,
        });
        Ok(())
    }

    fn entry(&self, origin: Origin, name: &str, surface: Surface) -> Result<&Entry, LookupError> {
        let entries = self.entries.get(name).ok_or(LookupError::UnknownAction)?;
        let mut error = LookupError::UnavailableOrigin;
        for entry in entries {
            if entry.origin.is_none() || entry.origin == Some(origin) {
                if entry.scope.supports(surface) {
                    return Ok(entry);
                }
                error = LookupError::UnsupportedAction;
            }
        }
        Err(error)
    }

    /// Reads a declaration and reports whether this host has its implementation.
    ///
    /// # Errors
    /// Rejects unknown names, unavailable origins and unsupported surfaces.
    pub fn lookup(
        &self,
        origin: Origin,
        name: &str,
        surface: Surface,
    ) -> Result<Declaration, LookupError> {
        let entry = self.entry(origin, name, surface)?;
        Ok(Declaration {
            origin: entry.origin,
            scope: entry.scope,
            local: !matches!(entry.implementation, Implementation::Absent),
        })
    }

    /// Lists callable server actions for this surface in name order.
    #[must_use]
    pub fn server_actions(&self, surface: Surface) -> Vec<ServerActionT> {
        self.entries
            .iter()
            .filter_map(|(name, entries)| {
                let entry = entries.iter().find(|entry| {
                    entry.origin != Some(Origin::Host)
                        && entry.scope.supports(surface)
                        && matches!(entry.implementation, Implementation::Rust(_))
                })?;
                let mut declaration = ServerActionT::default();
                declaration.name = Some(name.clone());
                declaration.hash = declaration_hash(entry.origin, name, entry.scope);
                Some(declaration)
            })
            .collect()
    }

    /// Selects the Rust implementation that the dispatcher can call.
    ///
    /// # Errors
    /// Rejects a failed lookup or a declaration without a Rust implementation.
    pub fn get(
        &self,
        origin: Origin,
        name: &str,
        surface: Surface,
    ) -> Result<&dyn ErasedAction, LookupError> {
        match &self.entry(origin, name, surface)?.implementation {
            Implementation::Rust(action) => Ok(action.as_ref()),
            _ => Err(LookupError::UnavailableOrigin),
        }
    }
}

/// Hash the lowercase origin, name and scope with one colon between fields.
fn declaration_hash(origin: Option<Origin>, name: &str, scope: Scope) -> u64 {
    let origin = match origin {
        None => "any",
        Some(Origin::Host) => "host",
        Some(Origin::Server) => "server",
    };
    let scope = match scope {
        Scope::All => "all",
        Scope::Web => "web",
        Scope::Mobile => "mobile",
    };
    let hash = Sha256::new()
        .chain_update(origin)
        .chain_update(":")
        .chain_update(name)
        .chain_update(":")
        .chain_update(scope)
        .finalize();
    let mut first = [0; 8];
    first.copy_from_slice(&hash[..8]);
    u64::from_be_bytes(first)
}
