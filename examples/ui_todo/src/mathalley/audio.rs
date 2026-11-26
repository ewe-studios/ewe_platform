//! Procedural jazz, wind, and game cues. Each host supplies its playback clock.
#![expect(
    clippy::cast_precision_loss,
    reason = "The score uses floating-point sample times and waveform amplitudes."
)]

/// Beats per minute of the jazz loop. Children prefer a clear, moving pulse;
/// this is relaxed but still has a beat to tap to.
pub const BPM: f32 = 92.0;
pub const BEAT: f32 = 60.0 / BPM;
pub const BAR: f32 = BEAT * 4.0;
pub const BARS: usize = 8;

/// How far ahead the scheduler writes notes, in seconds.
pub const LOOKAHEAD: f64 = 0.2;

pub const MASTER: f32 = 0.9;
pub const MUSIC: f32 = 0.16;
pub const SFX: f32 = 0.55;
pub const AMBIENCE: f32 = 0.09;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Correct,
    Wrong,
    Intro,
    Tally,
    GameOver,
}

#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub midi: u8,
    pub at: f32,
    pub duration: f32,
    pub gain: f32,
}

impl Cue {
    /// Cue notes are pure data so their shape can be tested without a browser.
    #[must_use]
    pub fn notes(self) -> &'static [Note] {
        match self {
            Cue::Correct => &[
                Note {
                    midi: 88,
                    at: 0.00,
                    duration: 0.30,
                    gain: 0.50,
                }, // E6
                Note {
                    midi: 95,
                    at: 0.09,
                    duration: 0.35,
                    gain: 0.42,
                }, // B6
            ],
            Cue::Wrong => &[
                Note {
                    midi: 67,
                    at: 0.00,
                    duration: 0.28,
                    gain: 0.45,
                }, // G4
                Note {
                    midi: 64,
                    at: 0.12,
                    duration: 0.36,
                    gain: 0.40,
                }, // E4
            ],
            Cue::Intro => &[
                Note {
                    midi: 72,
                    at: 0.00,
                    duration: 0.28,
                    gain: 0.40,
                }, // C5
                Note {
                    midi: 76,
                    at: 0.09,
                    duration: 0.30,
                    gain: 0.42,
                }, // E5
                Note {
                    midi: 79,
                    at: 0.18,
                    duration: 0.42,
                    gain: 0.46,
                }, // G5
            ],
            Cue::Tally => &[
                Note {
                    midi: 72,
                    at: 0.00,
                    duration: 0.22,
                    gain: 0.38,
                }, // C5
                Note {
                    midi: 76,
                    at: 0.08,
                    duration: 0.22,
                    gain: 0.40,
                }, // E5
                Note {
                    midi: 79,
                    at: 0.16,
                    duration: 0.24,
                    gain: 0.42,
                }, // G5
                Note {
                    midi: 83,
                    at: 0.24,
                    duration: 0.50,
                    gain: 0.48,
                }, // B5
            ],
            Cue::GameOver => &[
                Note {
                    midi: 76,
                    at: 0.00,
                    duration: 0.30,
                    gain: 0.42,
                }, // E5
                Note {
                    midi: 72,
                    at: 0.14,
                    duration: 0.32,
                    gain: 0.40,
                }, // C5
                Note {
                    midi: 67,
                    at: 0.28,
                    duration: 0.52,
                    gain: 0.44,
                }, // G4
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Keys,
    Bass,
    Melody,
    Brush,
}

impl Voice {
    /// Peak amplitude of one note inside the music bus.
    fn level(self) -> f32 {
        match self {
            Voice::Keys => 0.10,
            Voice::Bass => 0.12,
            Voice::Melody => 0.08,
            Voice::Brush => 0.018,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Chord {
    pub root: u8,
    pub voicing: [u8; 3],
}

/// Cmaj7 | Am7 | Dm7 | G7, twice. Root in the bass, 3rd–7th–9th in the comp.
pub const CHORDS: [Chord; BARS] = [
    Chord {
        root: 36,
        voicing: [52, 59, 62],
    }, // C2  / E3 B3 D4
    Chord {
        root: 33,
        voicing: [60, 67, 71],
    }, // A1  / C4 G4 B4
    Chord {
        root: 38,
        voicing: [53, 60, 64],
    }, // D2  / F3 C4 E4
    Chord {
        root: 31,
        voicing: [59, 65, 69],
    }, // G1  / B3 F4 A4
    Chord {
        root: 36,
        voicing: [52, 59, 62],
    },
    Chord {
        root: 33,
        voicing: [60, 67, 71],
    },
    Chord {
        root: 38,
        voicing: [53, 60, 64],
    },
    Chord {
        root: 31,
        voicing: [59, 65, 69],
    },
];

#[must_use]
pub fn midi_to_freq(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(midi) - 69.0) / 12.0)
}

/// A scheduled note. `at` is an `AudioContext.current_time()` value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub at: f64,
    pub midi: u8,
    pub duration: f32,
    pub gain: f32,
    pub voice: Voice,
}

/// Writes bars ahead of the audio clock. Keeping this separate from Web Audio
/// makes the musical timing testable.
#[derive(Clone, Copy, Debug)]
pub struct Scheduler {
    bar: usize,
    next: f64,
}

impl Scheduler {
    #[must_use]
    pub fn new(start: f64) -> Self {
        Self {
            bar: 0,
            next: start,
        }
    }

    #[must_use]
    pub fn bar(&self) -> usize {
        self.bar
    }

    #[must_use]
    pub fn next_time(&self) -> f64 {
        self.next
    }

    /// If a hidden tab left the scheduler more than a bar behind, jump to the
    /// next bar boundary instead of replaying the missed bars in a burst.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The preceding guard ensures a positive count of whole bars."
    )]
    pub fn resync(&mut self, now: f64) {
        if now - self.next > f64::from(BAR) {
            let bars = ((now - self.next) / f64::from(BAR)).ceil() as usize;
            self.next += bars as f64 * f64::from(BAR);
            self.bar = (self.bar + bars) % BARS;
        }
    }

    /// Append every event that starts before `until`. Call once per frame with
    /// `until = current_time + LOOKAHEAD`.
    pub fn fill(&mut self, now: f64, until: f64, seed: u64, out: &mut Vec<Event>) {
        out.clear();
        while self.next < until {
            self.emit_bar(seed, out);
            self.bar = (self.bar + 1) % BARS;
            self.next += f64::from(BAR);
        }
        // A frame that arrived a little late must not schedule notes in the past.
        out.retain(|event| event.at >= now - 0.05);
    }

    fn emit_bar(&self, seed: u64, out: &mut Vec<Event>) {
        let chord = CHORDS[self.bar];
        let start = self.next;
        let beat = f64::from(BEAT);

        // Bass: root on beat 1, fifth on beat 3. Plucked, short.
        out.push(Event {
            at: start,
            midi: chord.root,
            duration: BEAT * 0.55,
            gain: 1.0,
            voice: Voice::Bass,
        });
        out.push(Event {
            at: start + beat * 2.0,
            midi: chord.root + 7,
            duration: BEAT * 0.50,
            gain: 0.85,
            voice: Voice::Bass,
        });

        // Keys: soft comps on beats 2 and 4 (one-indexed).
        for (index, midi) in chord.voicing.iter().enumerate() {
            let accent = if index == 1 { 0.75 } else { 0.9 };
            out.push(Event {
                at: start + beat,
                midi: *midi,
                duration: BEAT * 0.85,
                gain: accent,
                voice: Voice::Keys,
            });
            out.push(Event {
                at: start + beat * 3.0,
                midi: *midi,
                duration: BEAT * 0.85,
                gain: 0.75,
                voice: Voice::Keys,
            });
        }

        // Melody: a sparse two-note phrase every other bar, from C major
        // pentatonic. Simple LCG, so a run seed always produces the same tune.
        if self.bar.is_multiple_of(2) {
            let degrees = [72_u8, 74, 76, 79, 81];
            let mut rng = Lcg::new(seed ^ (self.bar as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let first = degrees[rng.next_u32() as usize % degrees.len()];
            let mut second = degrees[rng.next_u32() as usize % degrees.len()];
            if second == first {
                second =
                    degrees[(rng.next_u32() as usize % (degrees.len() - 1) + 1) % degrees.len()];
            }
            out.push(Event {
                at: start,
                midi: first,
                duration: BEAT * 0.9,
                gain: 0.8,
                voice: Voice::Melody,
            });
            out.push(Event {
                at: start + beat * 2.5,
                midi: second,
                duration: BEAT * 0.9,
                gain: 0.7,
                voice: Voice::Melody,
            });
        }

        // Brushes: a swung ride pattern, barely there. Offbeats sit at 2/3 of
        // the beat (the 2:1 swing feel).
        let offbeat = BEAT * (2.0 / 3.0);
        for step in 0..4 {
            let on = start + f64::from(step) * beat;
            let off = on + f64::from(offbeat);
            out.push(Event {
                at: on,
                midi: 0,
                duration: 0.05,
                gain: if step % 2 == 0 { 1.0 } else { 0.6 },
                voice: Voice::Brush,
            });
            out.push(Event {
                at: off,
                midi: 0,
                duration: 0.04,
                gain: 0.5,
                voice: Voice::Brush,
            });
        }
    }
}

/// Paul Kellet's economy pink-noise filter, baked into a seamless loop.
///
/// `samples` is the loop length; the tail of `samples + fade` is cross-faded
/// into the head so the loop point has no click.
#[must_use]
pub fn pink_noise(samples: usize, fade: usize, seed: u64) -> Vec<f32> {
    let mut rng = Lcg::new(seed);
    let total = samples + fade;
    let mut raw = Vec::with_capacity(total);
    let (mut b0, mut b1, mut b2, mut b3, mut b4, mut b5, mut b6) =
        (0.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    for _ in 0..total {
        let white = rng.bipolar();
        b0 = 0.99886 * b0 + white * 0.055_517_9;
        b1 = 0.99332 * b1 + white * 0.075_075_9;
        b2 = 0.96900 * b2 + white * 0.153_852;
        b3 = 0.86650 * b3 + white * 0.310_485_6;
        b4 = 0.55000 * b4 + white * 0.532_952_2;
        b5 = -0.7616 * b5 - white * 0.016_898_0;
        raw.push((b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362) * 0.11);
        b6 = white * 0.115_926;
    }

    let mut output = raw[..samples].to_vec();
    for index in 0..fade {
        let t = index as f32 / fade as f32;
        output[index] = raw[index] * t + raw[samples + index] * (1.0 - t);
    }
    let peak = output
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if peak > 0.0 {
        for sample in &mut output {
            *sample = *sample / peak * 0.8;
        }
    }
    output
}

/// Small deterministic PRNG shared by melody choices and noise generation.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    fn bipolar(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// Render the shared score into mono samples for a native audio output.
pub struct Mixer {
    scheduler: Scheduler,
    time: f64,
    seed: u64,
    pending: Vec<Event>,
    scheduled: Vec<Event>,
    noise: Vec<f32>,
    sample: usize,
}
impl Mixer {
    #[must_use]
    pub fn new(seed: u64, rate: u32) -> Self {
        Self {
            scheduler: Scheduler::new(0.0),
            time: 0.0,
            seed,
            pending: Vec::new(),
            scheduled: Vec::new(),
            noise: pink_noise(rate as usize, rate as usize / 32, seed),
            sample: 0,
        }
    }
    pub fn cue(&mut self, cue: Cue) {
        self.pending.extend(cue.notes().iter().map(|n| Event {
            at: self.time + f64::from(n.at),
            midi: n.midi,
            duration: n.duration,
            gain: n.gain * SFX,
            voice: Voice::Melody,
        }));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "The output waveform uses 32-bit samples."
    )]
    pub fn fill(&mut self, rate: u32, output: &mut [f32], audible: bool) {
        if rate == 0 {
            output.fill(0.0);
            return;
        }
        let step = 1.0 / f64::from(rate);
        let end = self.time + output.len() as f64 * step;
        self.scheduler
            .fill(self.time, end, self.seed, &mut self.scheduled);
        self.pending
            .extend(self.scheduled.iter().map(|event| Event {
                gain: event.gain * MUSIC * event.voice.level(),
                ..*event
            }));
        for value in output {
            let breeze = self.noise.get(self.sample).copied().unwrap_or_default();
            let mut mix = breeze * AMBIENCE;
            for n in &self.pending {
                let elapsed = (self.time - n.at) as f32;
                if elapsed < 0.0 || elapsed >= n.duration {
                    continue;
                }
                let envelope = (elapsed / 0.009).min(1.0) * (1.0 - elapsed / n.duration).powi(2);
                let phase = std::f32::consts::TAU * midi_to_freq(n.midi) * elapsed;
                let wave = match n.voice {
                    Voice::Brush => breeze,
                    Voice::Keys => {
                        0.55 * phase.sin()
                            + 0.35 * (phase * 1.0035).sin()
                            + 0.1 * (phase * 2.0).sin()
                    }
                    Voice::Bass => 0.75 * phase.sin() + 0.15 * (phase * 2.0).sin(),
                    Voice::Melody => 0.85 * phase.sin() + 0.12 * (phase * 4.0).sin(),
                };
                mix += n.gain * envelope * wave;
            }
            *value = if audible {
                (mix * MASTER).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            self.time += step;
            self.sample = (self.sample + 1) % self.noise.len().max(1);
        }
        self.pending
            .retain(|n| n.at + f64::from(n.duration) > self.time);
    }
}

use super::game::{BannerKind, Game, Phase};
use foundation_context::Ctx;
use foundation_ui_runtime::sound::{SoundError, SoundSession, SoundSource};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Samples {
    mixer: Mixer,
    previous: Option<Duration>,
    elapsed: Duration,
}
struct ScoreSource(Mutex<Samples>);
impl SoundSource for ScoreSource {
    fn render(&self, at: Duration, rate: u32, out: &mut [f32]) -> Result<(), SoundError> {
        let mut samples = self.0.lock().map_err(|_| SoundError::Lock)?;
        if let Some(previous) = samples.previous.replace(at) {
            if let Some(delta) = at.checked_sub(previous) {
                samples.elapsed = samples.elapsed.saturating_add(delta);
            } else {
                // A restarted output device can begin a new playback clock.
                samples.elapsed = Duration::from_secs_f64(samples.mixer.time);
            }
        }
        let time = samples.elapsed.as_secs_f64();
        let mixer = &mut samples.mixer;
        mixer.time = time;
        mixer.scheduler.resync(time);
        mixer
            .pending
            .retain(|n| n.at + f64::from(n.duration) > time);
        mixer.fill(rate, out, true);
        Ok(())
    }
}

/// The game owns the score. The host controls playback time and output.
pub struct Track {
    source: Arc<ScoreSource>,
    pub output: SoundSession,
    seen_phase: Phase,
    seen_banner: Option<Duration>,
}
impl Track {
    /// # Errors
    /// Returns an error for an invalid sample rate or missing sound host.
    pub fn new(ctx: &Ctx, seed: u64, rate: u32) -> Result<Self, SoundError> {
        if rate == 0 {
            return Err(SoundError::InvalidRate);
        }
        let source = Arc::new(ScoreSource(Mutex::new(Samples {
            mixer: Mixer::new(seed, rate),
            previous: None,
            elapsed: Duration::ZERO,
        })));
        let output = SoundSession::open(ctx, source.clone())?;
        Ok(Self {
            source,
            output,
            seen_phase: Phase::Title,
            seen_banner: None,
        })
    }
    /// # Errors
    /// Returns an error if the score lock fails.
    pub fn observe(&mut self, game: &Game) -> Result<(), SoundError> {
        let mut samples = self.source.0.lock().map_err(|_| SoundError::Lock)?;
        if game.phase != self.seen_phase {
            match game.phase {
                Phase::Intro => samples.mixer.cue(Cue::Intro),
                Phase::RoundTally => samples.mixer.cue(Cue::Tally),
                Phase::GameOver => samples.mixer.cue(Cue::GameOver),
                _ => {}
            }
            self.seen_phase = game.phase;
        }
        let until = game.banner.as_ref().map(|b| b.until);
        if self.seen_banner != until {
            if let Some(b) = &game.banner {
                samples.mixer.cue(match b.kind {
                    BannerKind::Clear => Cue::Correct,
                    BannerKind::Miss => Cue::Wrong,
                });
            }
            self.seen_banner = until;
        }
        Ok(())
    }
}
