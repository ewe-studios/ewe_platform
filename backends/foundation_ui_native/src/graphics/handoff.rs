//! The native provider returns its authenticated offer through an ordinary local Rust action.
use super::GraphicsError;
use crate::NativeCore;
use foundation_ui_gpu::{
    lease::{
        DeviceExchange, HostRequest, Identity, Lease, LeaseError, LeaseProvider, LeasedGraphics,
        PendingLease, Selection,
    },
    lease_native::{ImportedDevice, NativeLease},
    GpuConfig,
};
use foundation_ui_runtime::action::TriggerEvent;
use std::sync::{Arc, Mutex, PoisonError, Weak};

impl From<LeaseError> for GraphicsError {
    fn from(error: LeaseError) -> Self {
        match error {
            LeaseError::Denied => Self::Denied,
            LeaseError::Incompatible => Self::IncompatibleSurface,
            LeaseError::Cancelled => Self::Cancelled,
            LeaseError::Stale => Self::StaleSurface,
            LeaseError::DeviceLost => Self::DeviceLost,
            LeaseError::Unavailable(reason) => Self::Unavailable { reason },
        }
    }
}

pub use foundation_ui_gpu::lease::GraphicsReady;

#[uniffi::export(with_foreign)]
pub trait HostDeviceProvider: Send + Sync {
    /// Schedule collection of the host device. Return immediately to the originating action.
    fn request(&self, request: Arc<NativeDeviceRequest>);
}
#[uniffi::export(with_foreign)]
pub trait HostDeviceCompletion: Send + Sync {
    /// The host can reuse its resources after this callback. Loss requires a new device.
    fn released(&self, device_lost: bool);
}
struct Provider {
    core: Weak<NativeCore>,
    action: String,
    host: Arc<dyn HostDeviceProvider>,
}
impl LeaseProvider for Provider {
    fn request(&self, request: HostRequest) -> Result<(), LeaseError> {
        if self.core.strong_count() == 0 {
            return Err(LeaseError::Cancelled);
        }
        self.host.request(Arc::new(NativeDeviceRequest {
            request,
            core: self.core.clone(),
            action: self.action.clone(),
        }));
        Ok(())
    }
}

#[derive(uniffi::Object)]
pub struct NativeDeviceRequest {
    request: HostRequest,
    core: Weak<NativeCore>,
    action: String,
}

/// A native platform bridge retains the resources in this single-use object.
#[derive(uniffi::Object)]
pub struct NativeDeviceGrant {
    import: Mutex<Option<Import>>,
    state: Mutex<(bool, Weak<NativeLease>)>,
}
type Import = Box<dyn FnOnce() -> Result<ImportedDevice, LeaseError> + Send>;
impl NativeDeviceGrant {
    pub(crate) fn new(import: Import) -> Self {
        Self {
            import: Mutex::new(Some(import)),
            state: Mutex::default(),
        }
    }
    async fn open(&self) -> Result<ImportedDevice, LeaseError> {
        let import = self
            .import
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or(LeaseError::Stale)?;
        let (reply, result) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("foundation-ui-gpu-import".into())
            .spawn(move || {
                // A cancelled request no longer owns a receiver. Drop its retained device here.
                drop(reply.try_send(import()));
            })
            .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        result.recv().await.map_err(|_| LeaseError::Cancelled)?
    }
}

#[derive(uniffi::Object)]
pub struct NativePendingLease {
    exchange: Arc<DeviceExchange>,
    id: u64,
    ticket: Mutex<Option<PendingLease>>,
}
#[derive(uniffi::Object)]
pub struct NativeGraphicsLease {
    lease: Mutex<Option<Arc<Lease>>>,
}
#[derive(Debug, Copy, Clone, uniffi::Enum)]
pub enum HostDeviceFailure {
    Denied,
    Cancelled,
    Incompatible,
    DeviceLost,
}

#[uniffi::export]
impl NativeCore {
    /// Install a host provider that can deliver a result to the named local Rust action.
    ///
    /// # Errors
    /// Rejects a server reference, an absent Rust handler, or a second provider on the same context.
    pub fn install_device_provider(
        self: Arc<Self>,
        ready_action: String,
        provider: Arc<dyn HostDeviceProvider>,
    ) -> Result<(), GraphicsError> {
        self.manager
            .require_local_handler(&ready_action)
            .map_err(|e| GraphicsError::Operation {
                reason: e.to_string(),
            })?;
        let exchange = DeviceExchange::new(
            &self.graphics_ctx,
            Arc::new(Provider {
                core: Arc::downgrade(&self),
                action: ready_action,
                host: provider,
            }),
        )?;
        if !exchange.install(&self.graphics_ctx) {
            return Err(GraphicsError::Operation {
                reason: "A host provider is already installed".into(),
            });
        }
        Ok(())
    }

    /// Start the exchange from this core. A Rust action can also call `DeviceExchange::request` directly.
    ///
    /// # Errors
    /// Reports a missing provider or an invalid resource name.
    pub fn request_host_device(
        &self,
        device: String,
        surface: Option<String>,
    ) -> Result<Arc<NativePendingLease>, GraphicsError> {
        let exchange = self.graphics_ctx.get::<DeviceExchange>().ok_or_else(|| {
            GraphicsError::Unavailable {
                reason: "No device provider".into(),
            }
        })?;
        let ticket = exchange.request(Selection { device, surface })?;
        Ok(Arc::new(NativePendingLease {
            id: ticket.id(),
            exchange,
            ticket: Mutex::new(Some(ticket)),
        }))
    }
}
#[uniffi::export]
impl NativePendingLease {
    /// Wait once for the callback action to deliver its offer.
    ///
    /// # Errors
    /// Returns the refusal, cancellation, or platform import error.
    pub async fn wait(&self) -> Result<Arc<NativeGraphicsLease>, GraphicsError> {
        let ticket = self
            .ticket
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or(LeaseError::Cancelled)?;
        let lease = ticket.wait().await?;
        Ok(Arc::new(NativeGraphicsLease {
            lease: Mutex::new(Some(Arc::new(lease))),
        }))
    }
    pub fn cancel(&self) {
        self.exchange.cancel(self.id);
        self.ticket
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}
#[uniffi::export]
impl NativeDeviceRequest {
    #[must_use]
    pub fn device(&self) -> String {
        self.request.selection.device.clone()
    }
    #[must_use]
    pub fn surface(&self) -> Option<String> {
        self.request.selection.surface.clone()
    }
    #[must_use]
    pub fn request_id(&self) -> u64 {
        self.request.id
    }
    /// Import the host's original device. Dispatch the result to the configured local handler.
    ///
    /// # Errors
    /// Reports an import, dispatch, or cancellation error.
    pub async fn ready(
        &self,
        grant: Arc<NativeDeviceGrant>,
        generation: u64,
        completion: Arc<dyn HostDeviceCompletion>,
    ) -> Result<(), GraphicsError> {
        let imported = match grant.open().await {
            Ok(parts) => NativeLease::import(
                Identity {
                    selection: self.request.selection.clone(),
                    generation,
                },
                parts,
                GpuConfig::default(),
                {
                    let callback = completion.clone();
                    move |result| callback.released(result.is_err())
                },
            ),
            Err(error) => Err(error),
        };
        let result = match imported {
            Ok(lease) => {
                let mut state = grant.state.lock().unwrap_or_else(PoisonError::into_inner);
                if state.0 {
                    lease.withdraw();
                    Err(LeaseError::Stale)
                } else {
                    state.1 = Arc::downgrade(&lease);
                    Ok(lease as Arc<dyn LeasedGraphics>)
                }
            }
            Err(error) => {
                completion.released(error == LeaseError::DeviceLost);
                Err(error)
            }
        };
        self.deliver(result).await
    }
    /// Dispatch a typed refusal through the same local action.
    ///
    /// # Errors
    /// Returns an error if the initiating core or request has stopped.
    pub async fn fail(&self, reason: HostDeviceFailure) -> Result<(), GraphicsError> {
        self.deliver(Err(match reason {
            HostDeviceFailure::Denied => LeaseError::Denied,
            HostDeviceFailure::Cancelled => LeaseError::Cancelled,
            HostDeviceFailure::Incompatible => LeaseError::Incompatible,
            HostDeviceFailure::DeviceLost => LeaseError::DeviceLost,
        }))
        .await
    }
}
impl NativeDeviceRequest {
    async fn deliver(
        &self,
        result: Result<Arc<dyn LeasedGraphics>, LeaseError>,
    ) -> Result<(), GraphicsError> {
        let notice = self.request.offer(result)?;
        let core = self.core.upgrade().ok_or(LeaseError::Cancelled)?;
        let mut input = foundation_ui_wire::ActionInputT::default();
        input.event_name = Some("graphics-ready".into());
        let event = TriggerEvent {
            node_id: 0,
            input,
            state: Vec::new(),
        };
        let result = core
            .manager
            .trigger_structured(&self.action, event, &notice)
            .await;
        match result {
            Err(error) => {
                self.request.cancel();
                return Err(GraphicsError::Operation {
                    reason: error.to_string(),
                });
            }
            Ok(Some(bytes)) => {
                if let Err(error) = core.deliver_device_reply(bytes) {
                    self.request.cancel();
                    return Err(error);
                }
            }
            Ok(None) => {}
        }
        if self.request.awaiting_action() {
            self.request.cancel();
            return Err(GraphicsError::Operation {
                reason: "The ready action did not accept the host offer".into(),
            });
        }
        Ok(())
    }
}
impl NativeGraphicsLease {
    fn active(&self) -> Result<Arc<Lease>, GraphicsError> {
        self.lease
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(GraphicsError::StaleSurface)
    }
}
#[uniffi::export]
impl NativeGraphicsLease {
    /// # Errors
    /// Returns a shader error, cancellation, or device loss.
    pub async fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> Result<Vec<u32>, GraphicsError> {
        Ok(self.active()?.compute(shader, words, groups).await?)
    }
    /// # Errors
    /// Rejects an absent texture, invalid shader, or lost device.
    pub async fn process_texture(
        &self,
        shader: String,
        x: u32,
        y: u32,
        z: u32,
    ) -> Result<(), GraphicsError> {
        Ok(self.active()?.process_texture(shader, [x, y, z]).await?)
    }
    pub fn release(&self) {
        self.lease
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

#[cfg(target_vendor = "apple")]
/// Register the application's Metal objects in a single-use, retained grant.
///
/// # Safety
/// Supply a valid Metal device, its queue, and optionally its initialized 2D RGBA8 texture.
/// Lend the queue exclusively through the release callback.
#[no_mangle]
pub unsafe extern "C" fn ewe_ui_gpu_retain_metal_device(
    device: *mut std::ffi::c_void,
    queue: *mut std::ffi::c_void,
    texture: *mut std::ffi::c_void,
) -> u64 {
    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_metal::{MTLCommandQueue, MTLDevice, MTLTexture};
    // SAFETY: The typed Swift bridge lends the corresponding live Metal protocol objects.
    let (Some(device), Some(queue)) = (
        unsafe { Retained::<ProtocolObject<dyn MTLDevice>>::retain(device.cast()) },
        unsafe { Retained::<ProtocolObject<dyn MTLCommandQueue>>::retain(queue.cast()) },
    ) else {
        return 0;
    };
    let texture = unsafe { Retained::<ProtocolObject<dyn MTLTexture>>::retain(texture.cast()) };
    let grant = foundation_ui_gpu::metal::MetalGrant {
        device,
        queue,
        texture,
    };
    let native = NativeDeviceGrant::new(Box::new(move || {
        futures_lite::future::block_on(grant.import())
    }));
    <NativeDeviceGrant as uniffi::FfiConverterArc<crate::UniFfiTag>>::lower(Arc::new(native))
        .as_raw()
}

#[uniffi::export]
impl NativeDeviceGrant {
    /// Reject new submissions. The completion callback still waits for accepted device work.
    pub fn withdraw(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.0 = true;
        if let Some(lease) = state.1.upgrade() {
            lease.withdraw();
        }
    }
}
