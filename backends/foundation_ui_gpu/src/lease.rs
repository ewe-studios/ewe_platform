//! A local provider offers retained graphics resources through authenticated action input.
use async_channel::{Receiver, Sender};
use foundation_context::Ctx;
use futures_util::future::BoxFuture;
use rand_core::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError, Weak},
};

/// The application chooses these stable host resource names before submitting a request.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Selection {
    pub device: String,
    pub surface: Option<String>,
}

/// The host increments the generation after withdrawing or replacing a device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub selection: Selection,
    pub generation: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error, Serialize, Deserialize)]
pub enum LeaseError {
    #[error("The host denied the request")]
    Denied,
    #[error("The request was cancelled")]
    Cancelled,
    #[error("The supplied graphics resources are incompatible")]
    Incompatible,
    #[error("The host device was lost")]
    DeviceLost,
    #[error("This host grant is stale")]
    Stale,
    #[error("The host graphics service is unavailable: {0}")]
    Unavailable(String),
}

/// This object holds the host's original device. Its owner performs all device operations.
pub trait LeasedGraphics: Send + Sync {
    fn identity(&self) -> &Identity;
    fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> BoxFuture<'_, Result<Vec<u32>, LeaseError>>;
    /// Bind the supplied texture as input. Write the output back to that texture.
    fn process_texture(
        &self,
        shader: String,
        groups: [u32; 3],
    ) -> BoxFuture<'_, Result<(), LeaseError>>;
    /// Stop new work and notify the host after the device finishes all accepted work.
    fn release(&self);
}

/// A grant remains exclusive until the last in-flight caller drops its handle.
struct LeaseLife {
    resource: Arc<dyn LeasedGraphics>,
    active: std::sync::atomic::AtomicBool,
}
impl LeaseLife {
    fn release(&self) {
        if self.active.swap(false, std::sync::atomic::Ordering::AcqRel) {
            self.resource.release();
        }
    }
}
pub struct Lease(Arc<LeaseLife>);
impl Lease {
    fn new(resource: Arc<dyn LeasedGraphics>) -> Self {
        Self(Arc::new(LeaseLife {
            resource,
            active: std::sync::atomic::AtomicBool::new(true),
        }))
    }
    fn resource(&self) -> Result<&dyn LeasedGraphics, LeaseError> {
        if self.0.active.load(std::sync::atomic::Ordering::Acquire) {
            Ok(self.0.resource.as_ref())
        } else {
            Err(LeaseError::Stale)
        }
    }
    #[must_use]
    pub fn identity(&self) -> &Identity {
        self.0.resource.identity()
    }
    /// # Errors
    /// Returns the device or shader error from the original host device.
    pub async fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> Result<Vec<u32>, LeaseError> {
        self.resource()?.compute(shader, words, groups).await
    }
    /// # Errors
    /// Returns an incompatibility, submission, or device loss error.
    pub async fn process_texture(
        &self,
        shader: String,
        groups: [u32; 3],
    ) -> Result<(), LeaseError> {
        self.resource()?.process_texture(shader, groups).await
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// Only this data enters the typed local action. The resource remains in the originating exchange.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub request: u64,
    pub selection: Selection,
    pub result: Result<u64, LeaseError>,
    proof: [u8; 16],
}

pub trait LeaseProvider: Send + Sync {
    /// Schedule host work and return. A provider can also complete the request before returning.
    ///
    /// # Errors
    /// Return an error if the host cannot schedule the request.
    fn request(&self, request: HostRequest) -> Result<(), LeaseError>;
}
type Offered = Result<Lease, LeaseError>;
struct Pending {
    send: Sender<Offered>,
    offer: Option<(Notice, Offered)>,
}
#[derive(Default)]
struct State {
    next: u64,
    pending: BTreeMap<u64, Pending>,
    generations: BTreeMap<Selection, (u64, Weak<LeaseLife>)>,
}
struct Inner {
    proof: Box<dyn Fn() -> [u8; 16] + Send + Sync>,
    provider: Arc<dyn LeaseProvider>,
    state: Mutex<State>,
}
impl Inner {
    fn cancel(&self, id: u64) {
        let entry = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending
            .remove(&id);
        // Drop the grant outside the map lock. Its release can call the application.
        drop(entry);
    }
}

/// Install one exchange per core. The host invokes its configured local action with a Notice.
#[derive(Clone)]
pub struct DeviceExchange(Arc<Inner>);
impl DeviceExchange {
    /// # Errors
    /// Returns an error when the host context lacks its injected random source.
    pub fn new(ctx: &Ctx, provider: Arc<dyn LeaseProvider>) -> Result<Self, LeaseError> {
        let rng = ctx
            .try_rng()
            .ok_or_else(|| LeaseError::Unavailable("No context random source".into()))?;
        Ok(Self(Arc::new(Inner {
            proof: Box::new(move || {
                let mut bytes = [0; 16];
                let mut guard = match rng.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.fill_bytes(&mut bytes);
                bytes
            }),
            provider,
            state: Mutex::default(),
        })))
    }
    /// Install this exchange if the application has no provider yet.
    #[must_use]
    pub fn install(&self, ctx: &Ctx) -> bool {
        Arc::ptr_eq(&ctx.get_or_insert(self.clone()).0, &self.0)
    }
    /// Stop an outstanding request, including one whose caller has already started waiting.
    pub fn cancel(&self, id: u64) {
        self.0.cancel(id);
    }
    /// Register the pending request before calling the provider.
    ///
    /// # Errors
    /// Returns a configuration or scheduling error.
    pub fn request(&self, selection: Selection) -> Result<PendingLease, LeaseError> {
        if selection.device.is_empty() || selection.surface.as_ref().is_some_and(String::is_empty) {
            return Err(LeaseError::Incompatible);
        }
        let (send, receive) = async_channel::bounded(1);
        let request = {
            let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
            let id = state.next.checked_add(1).ok_or(LeaseError::Incompatible)?;
            state.next = id;
            state.pending.insert(id, Pending { send, offer: None });
            HostRequest {
                id,
                selection,
                exchange: Arc::downgrade(&self.0),
            }
        };
        let ticket = PendingLease {
            id: request.id,
            exchange: Arc::downgrade(&self.0),
            receive,
        };
        self.0.provider.request(request)?;
        Ok(ticket)
    }
    /// Consume only the exact offer that this core's host previously registered.
    pub fn accept(&self, notice: &Notice) -> bool {
        let entry = {
            let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
            if !state
                .pending
                .get(&notice.request)
                .and_then(|entry| entry.offer.as_ref())
                .is_some_and(|(issued, _)| issued == notice)
            {
                return false;
            }
            state.pending.remove(&notice.request)
        };
        if let Some(Pending {
            send,
            offer: Some((_, offer)),
        }) = entry
        {
            return send.try_send(offer).is_ok();
        }
        false
    }
}

pub struct PendingLease {
    id: u64,
    receive: Receiver<Offered>,
    exchange: Weak<Inner>,
}
impl PendingLease {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }
    /// # Errors
    /// Returns the host refusal, loss, or cancellation.
    pub async fn wait(self) -> Offered {
        self.receive
            .recv()
            .await
            .map_err(|_| LeaseError::Cancelled)?
    }
}
impl Drop for PendingLease {
    fn drop(&mut self) {
        if let Some(exchange) = self.exchange.upgrade() {
            exchange.cancel(self.id);
        }
    }
}

/// Only the local provider receives this object. Incoming frames cannot create one.
pub struct HostRequest {
    pub id: u64,
    pub selection: Selection,
    exchange: Weak<Inner>,
}
impl HostRequest {
    /// # Errors
    /// Rejects cancelled requests, incompatible resource identities, and lost exchanges.
    pub fn offer(
        &self,
        resource: Result<Arc<dyn LeasedGraphics>, LeaseError>,
    ) -> Result<Notice, LeaseError> {
        let mut offered = resource.map(Lease::new);
        let exchange = self.exchange.upgrade().ok_or(LeaseError::Cancelled)?;
        let proof = (exchange.proof)();
        let mut state = exchange
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let pending = state.pending.get(&self.id).ok_or(LeaseError::Cancelled)?;
        if pending.offer.is_some() {
            return Err(LeaseError::Stale);
        }
        let mut superseded = None;
        let mut rejected = None;
        if let Ok(lease) = &offered {
            let identity = lease.identity();
            let previous = state.generations.get(&self.selection);
            let invalid = if identity.selection != self.selection || identity.generation == 0 {
                Some(LeaseError::Incompatible)
            } else if previous.is_some_and(|(generation, _)| identity.generation < *generation) {
                Some(LeaseError::Stale)
            } else if previous.is_some_and(|(generation, old)| {
                identity.generation == *generation
                    && old.upgrade().is_some_and(|grant| {
                        grant.active.load(std::sync::atomic::Ordering::Acquire)
                    })
            }) {
                Some(LeaseError::Denied)
            } else {
                None
            };
            if let Some(error) = invalid {
                rejected = Some(std::mem::replace(&mut offered, Err(error)));
            } else {
                superseded = state
                    .generations
                    .insert(
                        self.selection.clone(),
                        (identity.generation, Arc::downgrade(&lease.0)),
                    )
                    .and_then(|(_, prior)| prior.upgrade());
            }
        }
        let notice = Notice {
            request: self.id,
            selection: self.selection.clone(),
            result: offered
                .as_ref()
                .map(|lease| lease.identity().generation)
                .map_err(Clone::clone),
            proof,
        };
        if let Some(entry) = state.pending.get_mut(&self.id) {
            entry.offer = Some((notice.clone(), offered));
        }
        drop(state);
        if let Some(prior) = superseded {
            prior.release();
        }
        drop(rejected);
        Ok(notice)
    }
    /// Check whether the ready action still needs to consume the pending offer.
    #[must_use]
    pub fn awaiting_action(&self) -> bool {
        self.exchange.upgrade().is_some_and(|ex| {
            ex.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pending
                .contains_key(&self.id)
        })
    }
    /// Cancel a scheduled offer if the ready action fails to dispatch.
    pub fn cancel(&self) {
        if let Some(exchange) = self.exchange.upgrade() {
            exchange.cancel(self.id);
        }
    }
}

use foundation_ui_runtime::action::{
    Action, ActionError, ActionFuture, Context, Interaction, Origin, Scope,
};
/// An application can register this handler or its own typed Notice handler.
pub struct GraphicsReady;
impl Action<Notice> for GraphicsReady {
    type State = ();
    const NAME: &'static str = "foundation_gpu_ready";
    fn origin() -> Option<Origin> {
        Some(Origin::Host)
    }
    fn scope() -> Scope {
        Scope::All
    }
    fn call<'a>(&'a self, ctx: &'a Context, event: Interaction<(), Notice>) -> ActionFuture<'a> {
        Box::pin(async move {
            let exchange = ctx.inner().get::<DeviceExchange>().ok_or_else(|| {
                foundation_errstacks::ErrorTrace::new(ActionError::new(
                    foundation_ui_wire::ErrorCode::ActionFailed,
                    "Graphics provider unavailable",
                ))
            })?;
            // Duplicate or cancelled callbacks produce an empty successful reply.
            exchange.accept(&event.input_value);
            Ok(())
        })
    }
}
