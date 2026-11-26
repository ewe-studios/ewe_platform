//! The Mathalley run: rounds, lives, hidden skill, two mini-games.
//!
//! Pure Rust. No `wgpu`, no web types. Time and randomness come from
//! `foundation_compact`, so the same code runs natively in tests and on wasm.
#![expect(
    clippy::cast_precision_loss,
    reason = "The reference score uses floating-point multipliers."
)]

use std::time::Duration;

use foundation_compact::rng::{ChaCha8Rng, Rng, SeedableRng};

pub const DESIGN_W: f32 = 390.0;
pub const DESIGN_H: f32 = 844.0;
pub const LEVELS_PER_ROUND: usize = 3;
pub const MAX_ROUNDS: u32 = 3;
pub const START_LIVES: u8 = 3;

const INTRO_HOLD: Duration = Duration::from_millis(1000);
const CLEAR_HOLD: Duration = Duration::from_millis(1000);
const MISS_HOLD: Duration = Duration::from_millis(600);
const FAST_WINDOW: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Title,
    Intro,
    Playing,
    RoundTally,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelKind {
    Gumball,
    Train,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Choose(usize),
    SkipIntro,
    /// Shell-level toggle. The engine ignores it; the web shell flips its
    /// `sound_on` flag and ramps the audio master.
    ToggleSound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    Clear,
    Miss,
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub text: String,
    pub kind: BannerKind,
    pub until: Duration,
}

#[derive(Clone, Copy, Debug)]
pub struct Tally {
    pub round: u32,
    pub round_score: i64,
    pub hearts: u8,
    pub multiplier: f32,
    pub total: i64,
}

#[derive(Clone, Debug)]
pub struct Gumball {
    pub a: u32,
    pub b: u32,
    pub product: u32,
    pub options: [u32; 2],
    pub correct: usize,
    pub missing: bool,
    pub stretch: bool,
}

#[derive(Clone, Debug)]
pub struct Train {
    pub table: u32,
    pub start_n: u32,
    pub carriages: u32,
    pub index: u32,
    pub options: [u32; 3],
    pub correct: usize,
    pub stretch: bool,
    pub carriage_started: Duration,
}

#[derive(Clone, Debug)]
pub enum LevelState {
    Empty,
    Gumball(Gumball),
    Train(Train),
}

#[derive(Clone, Copy, Debug)]
enum Pending {
    Advance { at: Duration },
    GameOver { at: Duration },
}

pub struct Game {
    pub phase: Phase,
    pub round: u32,
    pub lives: u8,
    pub skill: f32,
    pub skill_at_round_start: f32,
    pub banked_score: i64,
    pub round_score: i64,
    pub streak: u32,
    pub total_answers: u32,
    pub lineup: [LevelKind; LEVELS_PER_ROUND],
    pub level_index: usize,
    pub level: LevelState,
    pub banner: Option<Banner>,
    pub tally: Option<Tally>,
    pub rng: ChaCha8Rng,
    pub now: Duration,
    pub level_started: Duration,
    intro_until: Duration,
    pending: Option<Pending>,
}

struct Question {
    a: u32,
    b: u32,
    correct_index: usize,
    correct_value: u32,
    response: f32,
}

impl Game {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let now = Duration::ZERO;
        Self {
            phase: Phase::Title,
            round: 1,
            lives: START_LIVES,
            skill: 0.10,
            skill_at_round_start: 0.10,
            banked_score: 0,
            round_score: 0,
            streak: 0,
            total_answers: 0,
            lineup: [LevelKind::Train; LEVELS_PER_ROUND],
            level_index: 0,
            level: LevelState::Empty,
            banner: None,
            tally: None,
            rng: ChaCha8Rng::seed_from_u64(seed),
            now,
            level_started: now,
            intro_until: now,
            pending: None,
        }
    }

    // ── run flow ────────────────────────────────────────────────────────────

    pub fn apply(&mut self, action: Action) {
        match (self.phase, action) {
            (Phase::Title, Action::Start) => self.start_run(),
            (Phase::Intro, Action::Start | Action::SkipIntro) => self.begin_play(),
            (Phase::Playing, Action::Choose(index)) => self.choose(index),
            (Phase::RoundTally, Action::Start) => self.continue_round(),
            (Phase::GameOver, Action::Start) => self.back_to_title(),
            _ => {}
        }
    }

    pub fn update(&mut self, now: Duration) {
        self.now = now;

        if let Some(pending) = self.pending.take() {
            match pending {
                Pending::Advance { at } if now >= at => {
                    self.banner = None;
                    self.advance_level();
                    return;
                }
                Pending::GameOver { at } if now >= at => {
                    self.banner = None;
                    self.phase = Phase::GameOver;
                    return;
                }
                other => self.pending = Some(other),
            }
        }

        if self.phase == Phase::Intro && now >= self.intro_until {
            self.begin_play();
        }

        if self
            .banner
            .as_ref()
            .is_some_and(|banner| now >= banner.until)
        {
            self.banner = None;
        }
    }

    fn start_run(&mut self) {
        self.round = 1;
        self.lives = START_LIVES;
        self.skill = 0.10;
        self.skill_at_round_start = 0.10;
        self.banked_score = 0;
        self.round_score = 0;
        self.streak = 0;
        self.total_answers = 0;
        self.tally = None;
        self.banner = None;
        self.pending = None;
        self.start_round();
    }

    fn start_round(&mut self) {
        self.skill = self.skill.max(self.skill_floor(self.round));
        self.skill_at_round_start = self.skill;
        self.round_score = 0;
        self.lineup = lineup_for(self.round);
        self.level_index = 0;
        self.start_level();
    }

    fn start_level(&mut self) {
        self.banner = None;
        self.pending = None;
        self.tally = None;
        self.level_started = self.now;
        self.level = match self.lineup[self.level_index] {
            LevelKind::Gumball => LevelState::Gumball(self.make_gumball()),
            LevelKind::Train => LevelState::Train(self.make_train()),
        };
        self.phase = Phase::Intro;
        self.intro_until = self.now + INTRO_HOLD;
    }

    fn begin_play(&mut self) {
        self.phase = Phase::Playing;
        self.level_started = self.now;
        if let LevelState::Train(train) = &mut self.level {
            train.carriage_started = self.now;
        }
    }

    fn advance_level(&mut self) {
        if self.level_index + 1 < LEVELS_PER_ROUND {
            self.level_index += 1;
            self.start_level();
        } else {
            self.finish_round();
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "The reference rounds the final score to whole points."
    )]
    fn finish_round(&mut self) {
        let total = (((self.round_score + i64::from(self.lives) * 100) as f32) * (1.0 + self.skill))
            .floor() as i64;
        self.banked_score += total;
        self.tally = Some(Tally {
            round: self.round,
            round_score: self.round_score,
            hearts: self.lives,
            multiplier: 1.0 + self.skill,
            total,
        });
        self.phase = Phase::RoundTally;
        self.level = LevelState::Empty;
        self.banner = None;
        self.pending = None;
    }

    fn continue_round(&mut self) {
        self.round += 1;
        if self.round > MAX_ROUNDS {
            self.phase = Phase::GameOver;
        } else {
            self.start_round();
        }
    }

    fn back_to_title(&mut self) {
        self.phase = Phase::Title;
        self.round = 1;
        self.lives = START_LIVES;
        self.skill = 0.10;
        self.skill_at_round_start = 0.10;
        self.banked_score = 0;
        self.round_score = 0;
        self.streak = 0;
        self.total_answers = 0;
        self.level = LevelState::Empty;
        self.banner = None;
        self.tally = None;
        self.pending = None;
        self.level_index = 0;
    }

    // ── answering ───────────────────────────────────────────────────────────

    fn choose(&mut self, index: usize) {
        if self.pending.is_some() {
            return;
        }
        let Some(question) = self.question() else {
            return;
        };
        let is_train = matches!(self.level, LevelState::Train(_));
        let option_count = if is_train { 3 } else { 2 };
        if index >= option_count {
            return;
        }

        let correct = index == question.correct_index;
        self.total_answers += 1;

        if correct {
            self.streak += 1;
            let base = if is_train { 100 } else { 200 };
            let speed = if is_train && question.response < FAST_WINDOW {
                50
            } else {
                0
            };
            let streak_bonus = i64::from(((self.streak - 1) * 10).min(50));
            self.round_score += base + speed + streak_bonus;
            self.adjust_skill(true, question.response, question.a, question.b);
        } else {
            self.streak = 0;
            self.adjust_skill(false, question.response, question.a, question.b);
            self.lives = self.lives.saturating_sub(1);
        }

        if is_train {
            self.resolve_train(correct, question.correct_value);
        } else {
            self.resolve_gumball(correct, question.correct_value);
        }
    }

    fn resolve_gumball(&mut self, correct: bool, correct_value: u32) {
        if correct {
            self.show_clear();
        } else {
            self.show_miss(correct_value);
            if self.lives == 0 {
                self.pending = Some(Pending::GameOver {
                    at: self.now + MISS_HOLD,
                });
            } else {
                self.pending = Some(Pending::Advance {
                    at: self.now + MISS_HOLD,
                });
            }
        }
    }

    fn resolve_train(&mut self, correct: bool, correct_value: u32) {
        if correct {
            let done = match &mut self.level {
                LevelState::Train(train) => {
                    train.index += 1;
                    train.index >= train.carriages
                }
                _ => return,
            };
            if done {
                self.show_clear();
            } else {
                self.deal_next_carriage();
            }
        } else {
            self.show_miss(correct_value);
            if self.lives == 0 {
                self.pending = Some(Pending::GameOver {
                    at: self.now + MISS_HOLD,
                });
            } else {
                // The same carriage stays; restart its response clock.
                if let LevelState::Train(train) = &mut self.level {
                    train.carriage_started = self.now;
                }
                self.level_started = self.now;
            }
        }
    }

    fn show_clear(&mut self) {
        let until = self.now + CLEAR_HOLD;
        self.banner = Some(Banner {
            text: "CLEAR!".to_string(),
            kind: BannerKind::Clear,
            until,
        });
        self.pending = Some(Pending::Advance { at: until });
    }

    fn show_miss(&mut self, correct_value: u32) {
        self.banner = Some(Banner {
            text: format!("ANSWER: {correct_value}"),
            kind: BannerKind::Miss,
            until: self.now + MISS_HOLD,
        });
    }

    fn question(&self) -> Option<Question> {
        match &self.level {
            LevelState::Gumball(gumball) => Some(Question {
                a: gumball.a,
                b: gumball.b,
                correct_index: gumball.correct,
                correct_value: gumball.options[gumball.correct],
                response: self.now.saturating_sub(self.level_started).as_secs_f32(),
            }),
            LevelState::Train(train) => {
                let n = train.start_n + train.index;
                Some(Question {
                    a: train.table,
                    b: n,
                    correct_index: train.correct,
                    correct_value: train.options[train.correct],
                    response: self
                        .now
                        .saturating_sub(train.carriage_started)
                        .as_secs_f32(),
                })
            }
            LevelState::Empty => None,
        }
    }

    // ── skill ───────────────────────────────────────────────────────────────

    #[must_use]
    #[expect(
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "The difficulty range bounds the whole-number table limit."
    )]
    pub fn table_cap(&self) -> u32 {
        (3.0 + (self.skill * 9.0).floor()).clamp(3.0, 12.0) as u32
    }

    #[must_use]
    pub fn skill_floor(&self, round: u32) -> f32 {
        match round {
            0 => 0.0,
            1 => 0.05,
            2 => 0.10,
            3 => 0.30,
            _ => 1.0,
        }
    }

    fn adjust_skill(&mut self, correct: bool, response: f32, a: u32, b: u32) {
        let cap = self.table_cap();
        let stretch = a.max(b) >= cap;
        let calibration = if self.total_answers <= 20 { 2.0 } else { 1.0 };

        let delta = if correct {
            if stretch {
                let speed = (1.0 - ((response - 1.5).max(0.0) / 7.5)).max(0.0);
                0.055 * speed * calibration
            } else {
                0.0
            }
        } else if stretch {
            -0.04 * calibration
        } else {
            -1.0 / 9.0
        };

        let ceiling = if self.skill_at_round_start < 0.95 {
            0.95
        } else {
            1.0
        };
        self.skill = (self.skill + delta).clamp(self.skill_floor(self.round), ceiling);
    }

    // ── questions ───────────────────────────────────────────────────────────

    fn make_gumball(&mut self) -> Gumball {
        let cap = self.table_cap();
        let t = self.range(2, cap);
        let product = t * cap;
        let missing = self.skill >= 0.50;

        let correct_value = if missing { cap } else { product };
        let distractor = if missing {
            self.factor_distractor(t, cap)
        } else {
            self.product_distractor(t, cap)
        };

        let mut options = [correct_value, distractor];
        self.shuffle(&mut options);
        let correct = options
            .iter()
            .position(|&value| value == correct_value)
            .expect("correct value is in the options");

        Gumball {
            a: t,
            b: cap,
            product,
            options,
            correct,
            missing,
            // Gumball always asks the frontier fact: one factor is the cap.
            stretch: true,
        }
    }

    fn make_train(&mut self) -> Train {
        let cap = self.table_cap();
        let table = self.range(3, cap + 1);
        let carriages = if self.skill < 0.25 { 3 } else { 6 };
        let start_n = if self.skill < 0.50 { 1 } else { 4 };
        let (options, correct, stretch) = self.train_options(table, start_n);

        Train {
            table,
            start_n,
            carriages,
            index: 0,
            options,
            correct,
            stretch,
            carriage_started: self.now,
        }
    }

    fn deal_next_carriage(&mut self) {
        let (table, n) = match &self.level {
            LevelState::Train(train) => (train.table, train.start_n + train.index),
            _ => return,
        };
        let (options, correct, stretch) = self.train_options(table, n);
        if let LevelState::Train(train) = &mut self.level {
            train.options = options;
            train.correct = correct;
            train.stretch = stretch;
            train.carriage_started = self.now;
        }
        self.level_started = self.now;
    }

    fn train_options(&mut self, table: u32, n: u32) -> ([u32; 3], usize, bool) {
        let product = table * n;
        let mut pool = distractor_pool(table, n);
        self.shuffle(&mut pool);
        let mut options = [product, pool[0], pool[1]];
        self.shuffle(&mut options);
        let correct = options
            .iter()
            .position(|&value| value == product)
            .expect("correct product is in the options");
        (options, correct, table.max(n) >= self.table_cap())
    }

    fn factor_distractor(&mut self, table: u32, cap: u32) -> u32 {
        let mut pool = vec![cap + 1];
        if cap > 2 {
            pool.push(cap - 1);
        }
        if table != cap {
            pool.push(table);
        }
        pool.retain(|&value| value > 0 && value != cap);
        self.pick(&pool)
    }

    fn product_distractor(&mut self, a: u32, b: u32) -> u32 {
        let pool = distractor_pool(a, b);
        self.pick(&pool)
    }

    fn pick(&mut self, values: &[u32]) -> u32 {
        values[(self.rng.next_u32() as usize) % values.len()]
    }

    fn range(&mut self, low: u32, high: u32) -> u32 {
        low + (self.rng.next_u32() % (high - low))
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.rng.next_u32() as usize) % (i + 1);
            items.swap(i, j);
        }
    }
}

#[must_use]
pub fn lineup_for(round: u32) -> [LevelKind; LEVELS_PER_ROUND] {
    if round % 2 == 1 {
        [LevelKind::Train, LevelKind::Train, LevelKind::Gumball]
    } else {
        [LevelKind::Gumball, LevelKind::Train, LevelKind::Gumball]
    }
}

/// Mathalley's near-miss pool for `a × b = c`, deduplicated and positive.
#[must_use]
pub fn distractor_pool(a: u32, b: u32) -> Vec<u32> {
    let c = a * b;
    let mut pool = vec![a * (b + 1), (a + 1) * b, c + 1, c - 1, c + 2, c - 2];
    if b > 2 {
        pool.push(a * (b - 1));
    }
    if a > 2 {
        pool.push((a - 1) * b);
    }
    if c >= 12 {
        let swapped = (c % 10) * 10 + c / 10;
        if swapped != c {
            pool.push(swapped);
        }
    }
    pool.retain(|&value| value > 0 && value != c);
    pool.sort_unstable();
    pool.dedup();
    while pool.len() < 2 {
        let extra = c + u32::from(!pool.is_empty()) + 1;
        if extra != c && !pool.contains(&extra) {
            pool.push(extra);
        }
    }
    pool
}
