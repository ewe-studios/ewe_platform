//! Keep an external WebGPU device on its event loop. Dispatch authenticated notices to local Rust actions.
use crate::lease::{
    DeviceExchange, GraphicsReady, HostRequest, Identity, Lease, LeaseError, LeaseProvider,
    LeasedGraphics, PendingLease, Selection,
};
use async_channel::Sender;
use foundation_context::Ctx;
use foundation_ui_runtime::{
    action::{ActionManager, ActionRegistry, TriggerEvent},
    network::{client_manager, reply_message},
};
use foundation_ui_wire::{
    writer::{ClientCaps, WriterConfig},
    ActionInputT, MountSurface,
};
use futures_util::future::{select, BoxFuture, Either};
use js_sys::{Function, Promise, Uint32Array};
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/src/web_device.js")]
extern "C" {
    #[wasm_bindgen(catch,js_name=inspectHostDevice)]
    async fn inspect(device: &JsValue, texture: &JsValue) -> Result<bool, JsValue>;
    type DeviceWatch;
    #[wasm_bindgen(js_name=watchHostDevice)]
    fn watch_device(device: &JsValue) -> DeviceWatch;
    #[wasm_bindgen(method, getter)]
    fn promise(this: &DeviceWatch) -> Promise;
    #[wasm_bindgen(method)]
    fn close(this: &DeviceWatch);
    #[wasm_bindgen(catch,js_name=computeHost)]
    async fn compute(
        device: &JsValue,
        shader: &str,
        words: Uint32Array,
        count: u32,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch,js_name=processHost)]
    async fn process(
        device: &JsValue,
        texture: &JsValue,
        shader: &str,
        x: u32,
        y: u32,
        z: u32,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch,js_name=finishHost)]
    async fn finish(device: &JsValue) -> Result<JsValue, JsValue>;
}
fn notify_return(released: &Function, lost: bool) {
    if let Err(error) = released.call1(&JsValue::UNDEFINED, &lost.into()) {
        web_sys::console::error_1(&error);
    }
}
fn js_error(value: &JsValue) -> LeaseError {
    LeaseError::Unavailable(
        js_sys::Reflect::get(value, &"message".into())
            .ok()
            .and_then(|s| s.as_string())
            .unwrap_or_else(|| format!("{value:?}")),
    )
}
enum Job {
    Compute(String, Vec<u32>, u32, Sender<Result<Vec<u32>, LeaseError>>),
    Texture(String, [u32; 3], Sender<Result<(), LeaseError>>),
}
struct WebLease {
    id: Identity,
    send: Sender<Job>,
    lost: AtomicBool,
}
impl WebLease {
    fn closed_error(&self, fallback: LeaseError) -> LeaseError {
        if self.lost.load(Ordering::Acquire) {
            LeaseError::DeviceLost
        } else {
            fallback
        }
    }
}
impl LeasedGraphics for WebLease {
    fn identity(&self) -> &Identity {
        &self.id
    }
    fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> BoxFuture<'_, Result<Vec<u32>, LeaseError>> {
        Box::pin(async move {
            let (reply, result) = async_channel::bounded(1);
            self.send
                .send(Job::Compute(shader, words, groups, reply))
                .await
                .map_err(|_| self.closed_error(LeaseError::Stale))?;
            result
                .recv()
                .await
                .map_err(|_| self.closed_error(LeaseError::Cancelled))?
        })
    }
    fn process_texture(
        &self,
        shader: String,
        groups: [u32; 3],
    ) -> BoxFuture<'_, Result<(), LeaseError>> {
        Box::pin(async move {
            let (reply, result) = async_channel::bounded(1);
            self.send
                .send(Job::Texture(shader, groups, reply))
                .await
                .map_err(|_| self.closed_error(LeaseError::Stale))?;
            result
                .recv()
                .await
                .map_err(|_| self.closed_error(LeaseError::Cancelled))?
        })
    }
    fn release(&self) {
        self.send.close();
    }
}
#[wasm_bindgen]
pub struct BrowserGrant {
    lease: Arc<WebLease>,
}
#[wasm_bindgen]
impl BrowserGrant {
    pub fn withdraw(&self) {
        self.lease.release();
    }
}

#[wasm_bindgen]
pub struct BrowserHostRequest {
    request: HostRequest,
    manager: Rc<ActionManager>,
    action: String,
    capacity: usize,
}
#[wasm_bindgen]
impl BrowserHostRequest {
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn device(&self) -> String {
        self.request.selection.device.clone()
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn surface(&self) -> Option<String> {
        self.request.selection.surface.clone()
    }
    /// # Errors
    /// Returns an error if the pending request has ended or the local action fails.
    pub async fn deny(&self) -> Result<(), JsError> {
        self.deliver(Err(LeaseError::Denied)).await?;
        Ok(())
    }
    /// Lend the original WebGPU device and optional texture. The host owns presentation.
    ///
    /// # Errors
    /// Rejects incompatible resources and grants that the local action cannot accept.
    pub async fn ready(
        &self,
        device: JsValue,
        texture: JsValue,
        generation: u64,
        released: Function,
    ) -> Result<BrowserGrant, JsError> {
        let check = if generation == 0
            || self.request.selection.surface.is_some()
                == (texture.is_null() || texture.is_undefined())
        {
            Err(LeaseError::Incompatible)
        } else {
            match inspect(&device, &texture).await {
                Ok(false) => Ok(()),
                Ok(true) => Err(LeaseError::DeviceLost),
                Err(_) => Err(LeaseError::Incompatible),
            }
        };
        if let Err(error) = check {
            notify_return(&released, error == LeaseError::DeviceLost);
            self.deliver(Err(error.clone())).await?;
            return Err(error.into());
        }
        let (send, queue) = async_channel::bounded(self.capacity);
        let lease = Arc::new(WebLease {
            id: Identity {
                selection: self.request.selection.clone(),
                generation,
            },
            send,
            lost: AtomicBool::new(false),
        });
        let worker = Arc::downgrade(&lease);
        wasm_bindgen_futures::spawn_local(async move {
            let watch = watch_device(&device);
            let lost = JsFuture::from(watch.promise());
            let jobs = async {
                while let Ok(job) = queue.recv().await {
                    if queue.is_closed() {
                        break;
                    }
                    match job {
                        Job::Compute(shader, words, count, reply) => {
                            let result = compute(
                                &device,
                                &shader,
                                Uint32Array::from(words.as_slice()),
                                count,
                            )
                            .await
                            .map(|v| Uint32Array::new(&v).to_vec())
                            .map_err(|error| js_error(&error));
                            drop(reply.try_send(result));
                        }
                        Job::Texture(shader, [x, y, z], reply) => {
                            let result = process(&device, &texture, &shader, x, y, z)
                                .await
                                .map(|_| ())
                                .map_err(|error| js_error(&error));
                            drop(reply.try_send(result));
                        }
                    }
                }
            };
            let lost = match select(lost, Box::pin(jobs)).await {
                Either::Left((_, jobs)) => {
                    if let Some(lease) = worker.upgrade() {
                        lease.lost.store(true, Ordering::Release);
                        lease.release();
                    }
                    drop(jobs);
                    true
                }
                Either::Right(((), lost)) => {
                    matches!(
                        select(lost, Box::pin(finish(&device))).await,
                        Either::Left(_) | Either::Right((Err(_), _))
                    )
                }
            };
            watch.close();
            // Drop replies for jobs that did not start. This wakes their callers.
            while queue.try_recv().is_ok() {}
            drop(queue);
            notify_return(&released, lost);
        });
        if let Err(error) = self.deliver(Ok(lease.clone())).await {
            lease.release();
            return Err(error.into());
        }
        Ok(BrowserGrant { lease })
    }
}
impl BrowserHostRequest {
    async fn deliver(
        &self,
        offer: Result<Arc<dyn LeasedGraphics>, LeaseError>,
    ) -> Result<(), LeaseError> {
        let was_grant = offer.is_ok();
        let notice = self.request.offer(offer)?;
        let mut input = ActionInputT::default();
        input.event_name = Some("graphics-ready".into());
        let result = self
            .manager
            .trigger_structured(
                &self.action,
                TriggerEvent {
                    node_id: 0,
                    input,
                    state: Vec::new(),
                },
                &notice,
            )
            .await;
        if let Ok(Some(bytes)) = &result {
            let body = reply_message(
                bytes,
                &foundation_ui_wire::WireLimits::default(),
                MountSurface::Web,
            )
            .map_err(|e| {
                self.request.cancel();
                LeaseError::Unavailable(e.to_string())
            })?;
            if body
                .messages()
                .into_iter()
                .flatten()
                .any(|msg| msg.control().and_then(|c| c.error()).is_some())
            {
                self.request.cancel();
                return Err(LeaseError::Unavailable(
                    "The local action rejected the host grant".into(),
                ));
            }
        }
        if self.request.awaiting_action() {
            self.request.cancel();
            return Err(LeaseError::Cancelled);
        }
        result.map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        if was_grant {
            notice.result?;
        }
        Ok(())
    }
}
struct Provider(Sender<HostRequest>);
impl LeaseProvider for Provider {
    fn request(&self, r: HostRequest) -> Result<(), LeaseError> {
        self.0
            .try_send(r)
            .map_err(|_| LeaseError::Unavailable("Host request queue is closed or full".into()))
    }
}
#[wasm_bindgen]
pub struct BrowserDeviceExchange {
    exchange: DeviceExchange,
    provider: Sender<HostRequest>,
}
impl BrowserDeviceExchange {
    /// # Errors
    /// Rejects an invalid queue capacity, local action, or context.
    pub fn install(
        ctx: &Ctx,
        manager: Rc<ActionManager>,
        action: String,
        host: Function,
        capacity: usize,
    ) -> Result<Self, LeaseError> {
        if capacity == 0 {
            return Err(LeaseError::Incompatible);
        }
        manager
            .require_local_handler(&action)
            .map_err(|e| LeaseError::Unavailable(e.to_string()))?;
        let (provider, requests) = async_channel::bounded(capacity);
        let exchange = DeviceExchange::new(ctx, Arc::new(Provider(provider.clone())))?;
        if !exchange.install(ctx) {
            return Err(LeaseError::Incompatible);
        }
        let pending_exchange = exchange.clone();
        wasm_bindgen_futures::spawn_local(async move {
            while let Ok(request) = requests.recv().await {
                let id = request.id;
                let request = BrowserHostRequest {
                    request,
                    manager: manager.clone(),
                    action: action.clone(),
                    capacity,
                };
                if let Err(error) = host.call1(&JsValue::UNDEFINED, &JsValue::from(request)) {
                    web_sys::console::error_1(&error);
                    // Dropping the pending request wakes the Rust caller.
                    pending_exchange.cancel(id);
                }
            }
        });
        Ok(Self { exchange, provider })
    }
}
#[wasm_bindgen]
impl BrowserDeviceExchange {
    /// # Errors
    /// Returns an error if the host context or default action cannot start.
    #[wasm_bindgen(constructor)]
    pub fn new(host: Function) -> Result<BrowserDeviceExchange, JsError> {
        let ctx = Ctx::try_host().map_err(|e| JsError::new(&e.to_string()))?;
        let mut registry = ActionRegistry::new();
        registry
            .register(GraphicsReady)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let manager = client_manager(
            ctx.clone(),
            Arc::new(registry),
            &ClientCaps::web_current(),
            &WriterConfig::default(),
        );
        Ok(Self::install(
            &ctx,
            Rc::new(manager),
            "host:foundation_gpu_ready".into(),
            host,
            crate::GpuConfig::default().queue_capacity,
        )?)
    }
    /// # Errors
    /// Rejects an invalid selection or an unavailable provider.
    pub fn request(
        &self,
        device: String,
        surface: Option<String>,
    ) -> Result<BrowserPendingLease, JsError> {
        let pending = self.exchange.request(Selection { device, surface })?;
        Ok(BrowserPendingLease {
            id: pending.id(),
            ticket: std::cell::RefCell::new(Some(pending)),
            exchange: self.exchange.clone(),
        })
    }
    pub fn close(&self) {
        self.provider.close();
    }
}
#[wasm_bindgen]
pub struct BrowserPendingLease {
    id: u64,
    ticket: std::cell::RefCell<Option<PendingLease>>,
    exchange: DeviceExchange,
}
#[wasm_bindgen]
impl BrowserPendingLease {
    /// # Errors
    /// Reports denial, cancellation, device loss, or an incompatible grant.
    pub async fn wait(&self) -> Result<BrowserGraphicsLease, JsError> {
        let ticket = self
            .ticket
            .borrow_mut()
            .take()
            .ok_or(LeaseError::Cancelled)?;
        Ok(BrowserGraphicsLease {
            lease: std::cell::RefCell::new(Some(Rc::new(ticket.wait().await?))),
        })
    }
    pub fn cancel(&self) {
        self.exchange.cancel(self.id);
    }
}
#[wasm_bindgen]
pub struct BrowserGraphicsLease {
    lease: std::cell::RefCell<Option<Rc<Lease>>>,
}
#[wasm_bindgen]
impl BrowserGraphicsLease {
    /// # Errors
    /// Reports lease, shader, or device errors.
    pub async fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> Result<Vec<u32>, JsError> {
        let lease = self.lease.borrow().clone().ok_or(LeaseError::Stale)?;
        Ok(lease.compute(shader, words, groups).await?)
    }
    /// # Errors
    /// Reports lease, shader, or device errors.
    pub async fn process_texture(
        &self,
        shader: String,
        x: u32,
        y: u32,
        z: u32,
    ) -> Result<(), JsError> {
        let lease = self.lease.borrow().clone().ok_or(LeaseError::Stale)?;
        Ok(lease.process_texture(shader, [x, y, z]).await?)
    }
    pub fn release(&self) {
        self.lease.borrow_mut().take();
    }
}
