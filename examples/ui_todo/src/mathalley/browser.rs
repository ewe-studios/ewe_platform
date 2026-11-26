//! Browser bindings for one local game.
use super::{audio::Track, gpu::Renderer, scene, Session};
use foundation_ui_gpu::{browser::BrowserSurface, GpuError};
use std::{cell::RefCell, rc::Rc, time::Duration};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Mathalley {
    session: Session,
    audio: Track,
    renderer: Rc<RefCell<Option<Renderer>>>,
}
/// The browser creates one context for all of its local sound sessions.
#[wasm_bindgen]
pub struct MathalleyHost {
    ctx: foundation_context::Ctx,
}
#[wasm_bindgen]
impl MathalleyHost {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        use foundation_ui_runtime::sound::{PullOutput, SoundHost};
        let ctx = foundation_context::Ctx::host();
        ctx.store(SoundHost(std::sync::Arc::new(PullOutput)));
        Self { ctx }
    }
    pub fn open(&self, seed: u64) -> Result<Mathalley, JsError> {
        Ok(Mathalley {
            session: Session::new(seed),
            audio: Track::new(&self.ctx, seed, 48_000)?,
            renderer: Rc::default(),
        })
    }
}
#[wasm_bindgen]
impl Mathalley {
    pub fn screen_name(&self) -> String {
        self.session.screen_name().to_owned()
    }
    pub fn tap(&mut self, x: f32, y: f32, width: f32, height: f32) -> Result<(), JsError> {
        self.session.tap(x, y, width, height);
        Ok(self.audio.output.volume(f32::from(self.session.sound_on))?)
    }
    pub fn active(&mut self, value: bool) -> Result<(), JsError> {
        self.session.set_active(value);
        Ok(self.audio.output.active(value)?)
    }
    pub fn close(&mut self) -> Result<(), JsError> {
        Ok(self.audio.output.close()?)
    }
    pub fn samples(&self, at: f64, rate: u32, count: u32) -> Result<Vec<f32>, JsError> {
        let mut samples = vec![0.0; count as usize];
        self.audio
            .output
            .render(Duration::try_from_secs_f64(at)?, rate, &mut samples)?;
        Ok(samples)
    }
    pub fn tick(&mut self, millis: f64) -> Result<(), JsError> {
        self.session
            .tick(Duration::try_from_secs_f64(millis / 1000.0)?);
        Ok(self.audio.observe(&self.session.game)?)
    }
    pub fn draw(&self, surface: &BrowserSurface) -> js_sys::Promise {
        let scene = scene::build(&self.session.game, self.session.sound_on);
        let renderer = self.renderer.clone();
        let surface = surface.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            let result = surface
                .present(move |gpu, tex, format| {
                    renderer
                        .borrow_mut()
                        .get_or_insert_with(|| {
                            Renderer::new(&gpu.device, format, scene.instances.len().max(1))
                        })
                        .render(gpu, tex, &scene)
                })
                .await;
            match result {
                Ok(()) => Ok(JsValue::TRUE),
                Err(GpuError::Suspended) => Ok(JsValue::FALSE),
                Err(error) => Err(JsError::from(error).into()),
            }
        })
    }
}
