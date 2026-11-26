//! A host supplies the playback clock for each view's sound output.
use foundation_context::Ctx;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Produce mono samples for the host's next playback interval.
pub trait SoundSource: Send + Sync {
    /// Fill the buffer at the given playback time.
    ///
    /// # Errors
    /// Returns a source error if the samples are unavailable.
    fn render(&self, at: Duration, rate: u32, samples: &mut [f32]) -> Result<(), SoundError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum SoundError {
    #[display("The host has no sound output")]
    MissingHost,
    #[display("The sample rate must be positive")]
    InvalidRate,
    #[display("The volume must be between zero and one")]
    InvalidVolume,
    #[display("The sound session is closed")]
    Closed,
    #[display("The sound session lock failed")]
    Lock,
}

/// Install an output factory at the host entry point.
pub struct SoundHost(pub Arc<dyn SoundOutput>);
pub trait SoundOutput: Send + Sync {
    /// Allocate the output for one view.
    ///
    /// # Errors
    /// Returns a host error if the output is unavailable.
    fn open(&self, source: Arc<dyn SoundSource>) -> Result<SoundSession, SoundError>;
}

/// A host pulls samples when its playback device needs them.
pub struct PullOutput;
impl SoundOutput for PullOutput {
    fn open(&self, source: Arc<dyn SoundSource>) -> Result<SoundSession, SoundError> {
        Ok(SoundSession(Mutex::new(State {
            source: Some(source),
            volume: 1.0,
            active: true,
        })))
    }
}
struct State {
    source: Option<Arc<dyn SoundSource>>,
    volume: f32,
    active: bool,
}
/// An output that belongs to one view.
pub struct SoundSession(Mutex<State>);
impl SoundSession {
    /// Open an independent session through the injected output.
    ///
    /// # Errors
    /// Returns an error if the host is absent or cannot open the output.
    pub fn open(ctx: &Ctx, source: Arc<dyn SoundSource>) -> Result<Self, SoundError> {
        ctx.get::<SoundHost>()
            .ok_or(SoundError::MissingHost)?
            .0
            .open(source)
    }
    /// Set the gain for this view. Zero mutes the output.
    ///
    /// # Errors
    /// Returns an error for an invalid gain or an unavailable session.
    pub fn volume(&self, volume: f32) -> Result<(), SoundError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(SoundError::InvalidVolume);
        }
        let mut state = self.0.lock().map_err(|_| SoundError::Lock)?;
        if state.source.is_none() {
            return Err(SoundError::Closed);
        }
        state.volume = volume;
        Ok(())
    }
    /// Suspend or resume the output for this view.
    ///
    /// # Errors
    /// Returns an error if the session is unavailable.
    pub fn active(&self, active: bool) -> Result<(), SoundError> {
        let mut state = self.0.lock().map_err(|_| SoundError::Lock)?;
        if state.source.is_none() {
            return Err(SoundError::Closed);
        }
        state.active = active;
        Ok(())
    }
    /// Fill one output buffer at the host playback time.
    ///
    /// # Errors
    /// Returns an error if the rate, source, or session is unavailable.
    pub fn render(&self, at: Duration, rate: u32, samples: &mut [f32]) -> Result<(), SoundError> {
        if rate == 0 {
            return Err(SoundError::InvalidRate);
        }
        let state = self.0.lock().map_err(|_| SoundError::Lock)?;
        let source = state.source.as_ref().ok_or(SoundError::Closed)?;
        if state.active {
            source.render(at, rate, samples)?;
            for value in samples {
                *value *= state.volume;
            }
        } else {
            samples.fill(0.0);
        }
        Ok(())
    }
    /// Release the source and end this output. Repeated calls succeed.
    ///
    /// # Errors
    /// Returns an error if the session lock failed.
    pub fn close(&self) -> Result<(), SoundError> {
        self.0.lock().map_err(|_| SoundError::Lock)?.source.take();
        Ok(())
    }
}
