//! Draw a mounted game on a native host surface.
use super::{audio::Track, gpu::Renderer, scene, Session};
use foundation_ui_gpu::GpuError;
use foundation_ui_native::{graphics::NativeDrawingSurface, CoreError, NativeCore};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(uniffi::Object)]
pub struct MathalleyGame {
    session: Mutex<Session>,
    audio: Mutex<Track>,
    rate: u32,

    renderer: Arc<Mutex<Option<Renderer>>>,
}

fn locked<T>(value: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, CoreError> {
    value.lock().map_err(|_| CoreError::Write {
        detail: "Mathalley session lock failed".into(),
    })
}

#[uniffi::export]
impl MathalleyGame {
    #[uniffi::constructor]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "The binding receives a reference-counted native object."
    )]
    pub fn new(core: Arc<NativeCore>, seed: u64, sample_rate: u32) -> Result<Arc<Self>, CoreError> {
        let audio = Track::new(core.host_context(), seed, sample_rate).map_err(sound_error)?;
        Ok(Arc::new(Self {
            session: Mutex::new(Session::new(seed)),
            audio: Mutex::new(audio),
            rate: sample_rate,
            renderer: Arc::default(),
        }))
    }

    pub fn tap(&self, x: f32, y: f32, width: f32, height: f32) -> Result<(), CoreError> {
        let mut session = locked(&self.session)?;
        session.tap(x, y, width, height);
        locked(&self.audio)?
            .output
            .volume(f32::from(session.sound_on))
            .map_err(sound_error)?;
        Ok(())
    }
    pub fn samples(&self, at_frame: u64, count: u32) -> Result<Vec<f32>, CoreError> {
        let mut data = vec![0.0; count as usize];
        let rate = u64::from(self.rate);
        let time = Duration::from_secs(at_frame / rate)
            + Duration::from_nanos((at_frame % rate) * 1_000_000_000 / rate);
        locked(&self.audio)?
            .output
            .render(time, self.rate, &mut data)
            .map_err(sound_error)?;
        Ok(data)
    }
    pub fn active(&self, value: bool) -> Result<(), CoreError> {
        locked(&self.session)?.set_active(value);
        locked(&self.audio)?
            .output
            .active(value)
            .map_err(sound_error)
    }
    pub fn stop_sound(&self) -> Result<(), CoreError> {
        locked(&self.audio)?.output.close().map_err(sound_error)
    }
    pub async fn draw(
        &self,
        surface: Arc<NativeDrawingSurface>,
        timestamp_nanos: u64,
    ) -> Result<(), CoreError> {
        let scene = {
            let mut session = locked(&self.session)?;
            session.tick(Duration::from_nanos(timestamp_nanos));
            let game = &session.game;
            locked(&self.audio)?.observe(game).map_err(sound_error)?;
            scene::build(game, session.sound_on)
        };
        let renderer = self.renderer.clone();
        match surface
            .present(move |gpu, texture, format| {
                renderer
                    .lock()
                    .map_err(|_| GpuError::Operation("Mathalley renderer lock failed".into()))?
                    .get_or_insert_with(|| {
                        Renderer::new(&gpu.device, format, scene.instances.len().max(1))
                    })
                    .render(gpu, texture, &scene)
            })
            .await
        {
            Ok(()) | Err(GpuError::Suspended) => Ok(()),
            Err(e) => Err(CoreError::Write {
                detail: e.to_string(),
            }),
        }
    }
}

fn sound_error(e: foundation_ui_runtime::sound::SoundError) -> CoreError {
    CoreError::Write {
        detail: e.to_string(),
    }
}
