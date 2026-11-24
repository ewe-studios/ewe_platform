//! Mathalley simulation, scene geometry, and procedural sound.
pub mod audio;
pub mod game;
#[cfg(any(feature = "mathalley-web", feature = "mobile"))]
pub mod gpu;
pub mod scene;
use game::{Action, Game, DESIGN_H, DESIGN_W};
use std::time::Duration;

/// One mounted game. The host supplies the seed and monotonic timestamps.
pub struct Session {
    pub game: Game,
    pub sound_on: bool,
    previous: Option<Duration>,
    active: bool,
}
impl Session {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            game: Game::new(seed),
            sound_on: true,
            previous: None,
            active: true,
        }
    }

    /// Name the visible game screen for the host's accessibility tree.
    #[must_use]
    pub fn screen_name(&self) -> &'static str {
        use game::{LevelState, Phase};
        match self.game.phase {
            Phase::Title => "Mathalley title",
            Phase::Intro => "Mathalley round intro",
            Phase::Playing => match self.game.level {
                LevelState::Train(_) => "Mathalley Train",
                LevelState::Gumball(_) => "Mathalley Gumball",
                LevelState::Empty => "Mathalley",
            },
            Phase::RoundTally => "Mathalley round tally",
            Phase::GameOver => "Mathalley game over",
        }
    }

    pub fn set_active(&mut self, active: bool) {
        if self.active != active {
            self.previous = None;
            self.active = active;
        }
    }

    pub fn tick(&mut self, timestamp: Duration) {
        if self.active {
            let delta = self
                .previous
                .replace(timestamp)
                .map_or(Duration::ZERO, |old| timestamp.saturating_sub(old));
            self.game.update(self.game.now.saturating_add(delta));
        }
    }

    pub fn tap(&mut self, x: f32, y: f32, width: f32, height: f32) -> Option<Action> {
        let scale = (width / DESIGN_W).min(height / DESIGN_H);
        if !self.active || !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let x = (x - (width - DESIGN_W * scale) / 2.0) / scale;
        let y = (y - (height - DESIGN_H * scale) / 2.0) / scale;
        if !(0.0..DESIGN_W).contains(&x) || !(0.0..DESIGN_H).contains(&y) {
            return None;
        }
        let action = scene::hit_test(&self.game, self.sound_on, x, y)?;
        if action == Action::ToggleSound {
            self.sound_on = !self.sound_on;
        } else {
            self.game.apply(action);
        }
        Some(action)
    }
}
#[cfg(all(target_family = "wasm", feature = "mathalley-web"))]
mod browser;
#[cfg(all(not(target_family = "wasm"), feature = "mobile"))]
mod native;

pub mod route;
