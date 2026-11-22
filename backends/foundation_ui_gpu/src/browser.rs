//! Canvas objects enter through the owning browser event loop.
use crate::{GpuConfig, GpuError, GpuOwner, GpuService, RenderHost, SurfaceId, Work};
use async_channel::Sender;
use foundation_context::Ctx;
use futures_util::{
    future::LocalBoxFuture,
    stream::{self, StreamExt},
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

pub(crate) fn ensure_available() -> Result<(), GpuError> {
    if !web_sys::window().is_some_and(|w| {
        js_sys::Reflect::get(&w.navigator(), &"gpu".into()).is_ok_and(|v| !v.is_undefined())
    }) {
        return Err(GpuError::Unavailable(
            "WebGPU requires a supported browser and secure context".into(),
        ));
    }
    Ok(())
}

type LocalWork = Box<dyn for<'a> FnOnce(&'a mut GpuOwner) -> LocalBoxFuture<'a, ()>>;
struct Canvas(HtmlCanvasElement);
impl RenderHost for Canvas {
    fn create_surface(
        &self,
        instance: &wgpu::Instance,
    ) -> Result<wgpu::Surface<'static>, GpuError> {
        instance
            .create_surface(wgpu::SurfaceTarget::Canvas(self.0.clone()))
            .map_err(|e| GpuError::Operation(e.to_string()))
    }
    fn size(&self) -> (u32, u32) {
        (self.0.width(), self.0.height())
    }
}

#[wasm_bindgen]
pub struct BrowserGraphics {
    local: Sender<LocalWork>,
    service: GpuService,
}
impl BrowserGraphics {
    /// # Errors
    /// Rejects an invalid configuration or an unavailable WebGPU backend.
    pub fn install(ctx: &Ctx, config: GpuConfig) -> Result<Self, GpuError> {
        let graphics = Self::start(config)?;
        ctx.store(graphics.service.clone());
        Ok(graphics)
    }

    fn start(config: GpuConfig) -> Result<Self, GpuError> {
        if config.queue_capacity == 0 {
            return Err(GpuError::Operation(
                "The queue capacity must be positive".into(),
            ));
        }
        ensure_available()?;
        let (send, work) = async_channel::bounded::<Work>(config.queue_capacity);
        let (local, ui) = async_channel::bounded::<LocalWork>(config.queue_capacity);
        let (finished, stopped) = async_channel::bounded(1);
        let service = GpuService {
            work: send,
            stopped,
        };
        wasm_bindgen_futures::spawn_local(async move {
            let mut owner = GpuService::owner(config, std::sync::Arc::new(crate::SystemDevice));
            // Every service clone can close the shared queue while the browser still holds its canvas sender.
            let shared = stream::unfold((work, ui.clone()), |(work, ui)| async move {
                match work.recv().await {
                    Ok(job) => Some((job as LocalWork, (work, ui))),
                    Err(_) => {
                        ui.close();
                        None
                    }
                }
            });
            let jobs = stream::select(shared, ui);
            futures_util::pin_mut!(jobs);
            while let Some(job) = jobs.next().await {
                job(&mut owner).await;
            }
            drop(owner);
            drop(finished);
        });
        Ok(Self { local, service })
    }
}

#[wasm_bindgen]
impl BrowserGraphics {
    /// # Errors
    /// Returns an error if the browser does not provide WebGPU.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<BrowserGraphics, JsError> {
        Ok(Self::start(GpuConfig::default())?)
    }
    /// # Errors
    /// Returns an error if device creation fails.
    pub async fn adapter(&self) -> Result<String, JsError> {
        Ok(self
            .service
            .run(|owner| {
                Box::pin(async move {
                    let info = owner.device().await?.adapter.get_info();
                    Ok(format!("{:?}: {}", info.backend, info.name))
                })
            })
            .await?)
    }
    /// # Errors
    /// Reports device, shader, or command errors.
    pub async fn compute(
        &self,
        shader: String,
        words: Vec<u32>,
        groups: u32,
    ) -> Result<Vec<u32>, JsError> {
        Ok(self
            .service
            .run(move |owner| {
                Box::pin(async move { owner.compute_words(&shader, &words, groups).await })
            })
            .await?)
    }
    /// # Errors
    /// Reports surface creation errors.
    pub async fn attach(
        &self,
        name: String,
        canvas: HtmlCanvasElement,
    ) -> Result<BrowserSurface, JsError> {
        if self.service.work.is_closed() {
            return Err(GpuError::Cancelled.into());
        }
        let (tx, rx) = async_channel::bounded(1);
        self.local
            .send(Box::new(move |owner| {
                Box::pin(async move {
                    let result = owner.attach(name, Box::new(Canvas(canvas))).await;
                    if let Err(async_channel::SendError(Ok(id))) = tx.send(result).await {
                        let _removed = owner.detach(&id);
                    }
                })
            }))
            .await
            .map_err(|_| GpuError::Cancelled)?;
        Ok(BrowserSurface {
            id: rx.recv().await.map_err(|_| GpuError::Cancelled)??,
            service: self.service.clone(),
        })
    }
    pub fn close(&self) {
        self.service.close();
        self.local.close();
    }
}

#[wasm_bindgen]
pub struct BrowserSurface {
    id: SurfaceId,
    service: GpuService,
}
#[wasm_bindgen]
impl BrowserSurface {
    /// # Errors
    /// Reports configuration errors or a removed target.
    pub async fn resize(&self, width: u32, height: u32) -> Result<(), JsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| Box::pin(async move { owner.resize(id, width, height).await }))
            .await?)
    }
    /// # Errors
    /// Reports presentation errors or a suspended or removed target.
    pub async fn clear(&self, r: f64, g: f64, b: f64, a: f64) -> Result<(), JsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| {
                Box::pin(async move { owner.clear(id, wgpu::Color { r, g, b, a }).await })
            })
            .await?)
    }
    /// # Errors
    /// Returns an error if the graphics service or target is unavailable.
    pub async fn remove(&self) -> Result<(), JsError> {
        let id = self.id.clone();
        Ok(self
            .service
            .run(move |owner| Box::pin(async move { owner.detach(&id) }))
            .await?)
    }
}
