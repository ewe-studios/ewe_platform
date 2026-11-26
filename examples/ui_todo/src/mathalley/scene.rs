//! Turns a `Game` into plain render data and hit regions.
//!
//! Pure Rust: no `wgpu`, no web types. Everything here is in the 390×844 design
//! space; `gpu.rs` letterboxes it into the real surface.
#![expect(
    clippy::cast_precision_loss,
    reason = "The reference scene converts small pixel indexes to GPU coordinates."
)]

use font8x8::UnicodeFonts;

use super::game::{Action, BannerKind, Game, LevelKind, LevelState, Phase, DESIGN_H, DESIGN_W};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub color: [f32; 4],
    pub radius: f32,
    pub angle: f32,
    padding: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub action: Action,
}

impl Hit {
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

pub struct Scene {
    pub instances: Vec<Instance>,
    pub hits: Vec<Hit>,
}

// Design tokens.
pub const BG: u32 = 0x1b_14_10;
pub const CARD: u32 = 0xfd_f3_dc;
pub const CARD_BORDER: u32 = 0xd9_b2_6a;
pub const INK: u32 = 0x3d_24_13;
pub const ACCENT: u32 = 0xe0_5a_4f;
pub const HEART: u32 = 0xe8_45_5e;
pub const GREEN: u32 = 0x3f_a4_5b;
pub const GLASS: u32 = 0xe8_f1_ef;
pub const BASE: u32 = 0xc6_3f_39;
pub const TRAIN: u32 = 0x7a_4a_2b;
pub const SKY: u32 = 0x27_46_53;
pub const GROUND: u32 = 0x2e_24_18;
pub const DIM: u32 = 0x4a_3a_2c;
pub const GOLD: u32 = 0xf2_c1_4e;

#[must_use]
pub fn srgb(hex: u32) -> [f32; 4] {
    fn channel(value: u32) -> f32 {
        let s = value as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    [
        channel((hex >> 16) & 0xff),
        channel((hex >> 8) & 0xff),
        channel(hex & 0xff),
        1.0,
    ]
}

impl Scene {
    fn new() -> Self {
        Self {
            instances: Vec::new(),
            hits: Vec::new(),
        }
    }

    fn push(&mut self, center: [f32; 2], half: [f32; 2], color: u32, radius: f32, angle: f32) {
        self.instances.push(Instance {
            center,
            half,
            color: srgb(color),
            radius,
            angle,
            padding: [0.0; 2],
        });
    }

    fn rect(&mut self, cx: f32, cy: f32, w: f32, h: f32, color: u32) {
        self.push([cx, cy], [w / 2.0, h / 2.0], color, 0.0, 0.0);
    }

    fn rounded(&mut self, cx: f32, cy: f32, w: f32, h: f32, radius: f32, color: u32) {
        self.push([cx, cy], [w / 2.0, h / 2.0], color, radius, 0.0);
    }

    fn circle(&mut self, cx: f32, cy: f32, radius: f32, color: u32) {
        self.push([cx, cy], [radius, radius], color, radius, 0.0);
    }

    fn angled(&mut self, cx: f32, cy: f32, w: f32, h: f32, color: u32, angle: f32) {
        self.push([cx, cy], [w / 2.0, h / 2.0], color, 0.0, angle);
    }

    fn hit(&mut self, cx: f32, cy: f32, w: f32, h: f32, action: Action) {
        self.hits.push(Hit {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
            action,
        });
    }
}

#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// `y` is the top of the 8px glyph cell, in design space.
pub fn push_text(
    instances: &mut Vec<Instance>,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: u32,
    align: Align,
) {
    let width = text.chars().count() as f32 * 8.0 * scale;
    let start_x = match align {
        Align::Left => x,
        Align::Center => x - width / 2.0,
        Align::Right => x - width,
    };
    let linear = srgb(color);

    for (index, raw) in text.chars().enumerate() {
        let ch = match raw {
            '×' => 'x',
            other => other.to_ascii_uppercase(),
        };
        let Some(glyph) = font8x8::BASIC_FONTS.get(ch) else {
            continue;
        };
        let glyph_x = start_x + index as f32 * 8.0 * scale;
        for (row, bits) in glyph.iter().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) == 0 {
                    continue;
                }
                instances.push(Instance {
                    center: [
                        glyph_x + column as f32 * scale + scale / 2.0,
                        y + row as f32 * scale + scale / 2.0,
                    ],
                    half: [scale / 2.0, scale / 2.0],
                    color: linear,
                    radius: 0.0,
                    angle: 0.0,
                    padding: [0.0; 2],
                });
            }
        }
    }
}

#[must_use]
pub fn build(game: &Game, sound_on: bool) -> Scene {
    let mut scene = Scene::new();
    scene.rect(DESIGN_W / 2.0, DESIGN_H / 2.0, DESIGN_W, DESIGN_H, BG);

    match game.phase {
        Phase::Title => title(&mut scene),
        Phase::Intro => intro(&mut scene, game),
        Phase::Playing => playing(&mut scene, game),
        Phase::RoundTally => tally(&mut scene, game),
        Phase::GameOver => game_over(&mut scene, game),
    }

    // Drawn last so its hit region wins over a full-screen tap underneath.
    sound_toggle(&mut scene, sound_on);
    scene
}

#[must_use]
pub fn hit_test(game: &Game, sound_on: bool, x: f32, y: f32) -> Option<Action> {
    build(game, sound_on)
        .hits
        .into_iter()
        .rev()
        .find(|hit| hit.contains(x, y))
        .map(|hit| hit.action)
}

fn sound_toggle(scene: &mut Scene, sound_on: bool) {
    let (label, color) = if sound_on {
        ("SOUND ON", CARD)
    } else {
        ("SOUND OFF", DIM)
    };
    push_text(
        &mut scene.instances,
        label,
        372.0,
        790.0,
        2.0,
        color,
        Align::Right,
    );
    scene.hit(292.0, 798.0, 160.0, 40.0, Action::ToggleSound);
}

fn title(scene: &mut Scene) {
    scene.hit(
        DESIGN_W / 2.0,
        DESIGN_H / 2.0,
        DESIGN_W,
        DESIGN_H,
        Action::Start,
    );

    push_text(
        &mut scene.instances,
        "MATHALLEY",
        DESIGN_W / 2.0,
        220.0,
        5.0,
        CARD,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        "TAP TO PLAY",
        DESIGN_W / 2.0,
        330.0,
        3.0,
        CARD_BORDER,
        Align::Center,
    );

    let colors = [ACCENT, GOLD, GREEN, HEART, GLASS];
    for (index, color) in colors.iter().enumerate() {
        let x = 75.0 + index as f32 * 60.0;
        scene.rounded(x, 470.0, 46.0, 66.0, 23.0, *color);
        scene.rounded(x, 470.0, 20.0, 30.0, 10.0, CARD);
    }
}

fn intro(scene: &mut Scene, game: &Game) {
    scene.hit(
        DESIGN_W / 2.0,
        DESIGN_H / 2.0,
        DESIGN_W,
        DESIGN_H,
        Action::SkipIntro,
    );

    scene.rounded(DESIGN_W / 2.0, 380.0, 320.0, 250.0, 22.0, CARD_BORDER);
    scene.rounded(DESIGN_W / 2.0, 380.0, 306.0, 236.0, 18.0, CARD);

    let (name, hint) = match game.lineup[game.level_index] {
        LevelKind::Gumball => ("GUMBALL", "PICK THE ANSWER"),
        LevelKind::Train => ("TRAIN", "LOAD THE CARRIAGES"),
    };
    push_text(
        &mut scene.instances,
        name,
        DESIGN_W / 2.0,
        330.0,
        5.0,
        INK,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        hint,
        DESIGN_W / 2.0,
        405.0,
        2.0,
        INK,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        "TAP",
        DESIGN_W / 2.0,
        465.0,
        3.0,
        ACCENT,
        Align::Center,
    );

    push_text(
        &mut scene.instances,
        &format!("ROUND {}  LEVEL {}", game.round, game.level_index + 1),
        DESIGN_W / 2.0,
        560.0,
        2.0,
        CARD_BORDER,
        Align::Center,
    );
}

fn playing(scene: &mut Scene, game: &Game) {
    hud(scene, game);
    match &game.level {
        LevelState::Gumball(gumball) => gumball_scene(scene, game, gumball),
        LevelState::Train(train) => train_scene(scene, game, train),
        LevelState::Empty => {}
    }
    banner(scene, game);
}

fn hud(scene: &mut Scene, game: &Game) {
    for index in 0..3 {
        let color = if index < i32::from(game.lives) {
            HEART
        } else {
            DIM
        };
        heart(scene, 163.0 + index as f32 * 32.0, 38.0, 13.0, color);
    }
    push_text(
        &mut scene.instances,
        &format!("ROUND {}", game.round),
        18.0,
        20.0,
        2.0,
        CARD,
        Align::Left,
    );
    push_text(
        &mut scene.instances,
        &format!("{}", game.banked_score + game.round_score),
        372.0,
        20.0,
        2.0,
        CARD,
        Align::Right,
    );
}

fn heart(scene: &mut Scene, cx: f32, cy: f32, r: f32, color: u32) {
    scene.circle(cx - r * 0.42, cy - r * 0.12, r * 0.55, color);
    scene.circle(cx + r * 0.42, cy - r * 0.12, r * 0.55, color);
    scene.angled(
        cx,
        cy + r * 0.28,
        r * 1.05,
        r * 1.05,
        color,
        std::f32::consts::FRAC_PI_4,
    );
}

fn gumball_scene(scene: &mut Scene, game: &Game, gumball: &super::game::Gumball) {
    scene.rounded(DESIGN_W / 2.0, 350.0, 300.0, 400.0, 26.0, BASE);
    scene.circle(DESIGN_W / 2.0, 300.0, 108.0, GLASS);
    scene.rounded(DESIGN_W / 2.0, 470.0, 300.0, 92.0, 22.0, ACCENT);
    scene.rounded(DESIGN_W / 2.0, 430.0, 84.0, 44.0, 12.0, INK);
    scene.circle(DESIGN_W / 2.0, 490.0, 15.0, CARD);

    let balls = [ACCENT, GOLD, GREEN, HEART, SKY];
    for (index, color) in balls.iter().enumerate() {
        let angle = index as f32 * 1.256;
        scene.circle(
            DESIGN_W / 2.0 + angle.cos() * 58.0,
            300.0 + angle.sin() * 52.0,
            17.0,
            *color,
        );
    }

    let question = if gumball.missing {
        format!("{} X ? = {}", gumball.a, gumball.product)
    } else {
        format!("{} X {} = ?", gumball.a, gumball.b)
    };
    question_card(scene, &question);

    if game.banner.is_none() {
        options(scene, gumball.options);
    }
}

fn train_scene(scene: &mut Scene, game: &Game, train: &super::game::Train) {
    scene.rect(DESIGN_W / 2.0, 180.0, DESIGN_W, 280.0, SKY);
    scene.circle(320.0, 92.0, 28.0, GOLD);
    scene.rect(DESIGN_W / 2.0, 360.0, DESIGN_W, 120.0, GROUND);

    let carriages = train.carriages.max(1);
    let gap = 7.0;
    let engine_width = 46.0;
    let total = 330.0;
    let left = (DESIGN_W - total) / 2.0;
    let cars_left = left + engine_width + gap;
    let cars_total = total - engine_width - gap;
    let width = (cars_total - gap * (carriages - 1) as f32) / carriages as f32;
    for index in 0..carriages {
        let cx = cars_left + width / 2.0 + index as f32 * (width + gap);
        let active = index == train.index;
        let color = if active { CARD } else { TRAIN };
        scene.rounded(cx, 320.0, width, 74.0, 10.0, color);
        scene.circle(cx - width * 0.22, 366.0, 9.0, INK);
        scene.circle(cx + width * 0.22, 366.0, 9.0, INK);
        if active {
            scene.rounded(cx, 286.0, width * 0.7, 8.0, 4.0, GOLD);
        }
    }
    // Engine, at the front of the train.
    let engine_x = left + engine_width / 2.0;
    scene.rounded(engine_x, 330.0, engine_width, 64.0, 10.0, ACCENT);
    scene.rect(engine_x, 292.0, 18.0, 22.0, INK);
    scene.circle(engine_x - 12.0, 366.0, 9.0, INK);
    scene.circle(engine_x + 12.0, 366.0, 9.0, INK);

    let n = train.start_n + train.index;
    question_card(scene, &format!("{} X {} = ?", train.table, n));

    if game.banner.is_none() {
        chips(scene, &train.options);
    }
}

fn question_card(scene: &mut Scene, text: &str) {
    scene.rounded(DESIGN_W / 2.0, 545.0, 340.0, 110.0, 18.0, CARD_BORDER);
    scene.rounded(DESIGN_W / 2.0, 545.0, 326.0, 96.0, 14.0, CARD);
    push_text(
        &mut scene.instances,
        text,
        DESIGN_W / 2.0,
        532.0,
        3.0,
        INK,
        Align::Center,
    );
}

fn options(scene: &mut Scene, values: [u32; 2]) {
    let xs = [105.0, 285.0];
    for (index, value) in values.iter().enumerate() {
        scene.rounded(xs[index], 690.0, 150.0, 86.0, 43.0, CARD);
        push_text(
            &mut scene.instances,
            &value.to_string(),
            xs[index],
            668.0,
            5.0,
            INK,
            Align::Center,
        );
        scene.hit(xs[index], 690.0, 150.0, 86.0, Action::Choose(index));
    }
}

fn chips(scene: &mut Scene, values: &[u32; 3]) {
    let xs = [75.0, 195.0, 315.0];
    for (index, value) in values.iter().enumerate() {
        scene.rounded(xs[index], 660.0, 110.0, 76.0, 14.0, CARD);
        push_text(
            &mut scene.instances,
            &value.to_string(),
            xs[index],
            638.0,
            4.0,
            INK,
            Align::Center,
        );
        scene.hit(xs[index], 660.0, 110.0, 76.0, Action::Choose(index));
    }
}

fn banner(scene: &mut Scene, game: &Game) {
    let Some(banner) = &game.banner else {
        return;
    };
    let color = match banner.kind {
        BannerKind::Clear => GREEN,
        BannerKind::Miss => ACCENT,
    };
    push_text(
        &mut scene.instances,
        &banner.text,
        DESIGN_W / 2.0,
        790.0,
        3.0,
        color,
        Align::Center,
    );
}

fn tally(scene: &mut Scene, game: &Game) {
    scene.hit(
        DESIGN_W / 2.0,
        DESIGN_H / 2.0,
        DESIGN_W,
        DESIGN_H,
        Action::Start,
    );
    let Some(tally) = &game.tally else {
        return;
    };

    scene.rounded(DESIGN_W / 2.0, 420.0, 340.0, 520.0, 24.0, CARD_BORDER);
    scene.rounded(DESIGN_W / 2.0, 420.0, 324.0, 504.0, 20.0, CARD);

    push_text(
        &mut scene.instances,
        &format!("ROUND {}", tally.round),
        DESIGN_W / 2.0,
        240.0,
        4.0,
        INK,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        "COMPLETE",
        DESIGN_W / 2.0,
        300.0,
        3.0,
        INK,
        Align::Center,
    );

    let rows = [
        ("ROUND SCORE", format!("{}", tally.round_score)),
        ("HEARTS", format!("{}", tally.hearts)),
        ("MULTIPLIER", format!("{:.2}X", tally.multiplier)),
        ("TOTAL", format!("{}", tally.total)),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = 340.0 + index as f32 * 58.0;
        push_text(&mut scene.instances, label, 70.0, y, 2.0, INK, Align::Left);
        push_text(
            &mut scene.instances,
            value,
            320.0,
            y,
            2.0,
            INK,
            Align::Right,
        );
    }

    push_text(
        &mut scene.instances,
        "TAP TO CONTINUE",
        DESIGN_W / 2.0,
        610.0,
        2.0,
        ACCENT,
        Align::Center,
    );
}

fn game_over(scene: &mut Scene, game: &Game) {
    scene.hit(
        DESIGN_W / 2.0,
        DESIGN_H / 2.0,
        DESIGN_W,
        DESIGN_H,
        Action::Start,
    );
    push_text(
        &mut scene.instances,
        "GAME OVER",
        DESIGN_W / 2.0,
        330.0,
        5.0,
        CARD,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        &format!("SCORE {}", game.banked_score),
        DESIGN_W / 2.0,
        440.0,
        4.0,
        GOLD,
        Align::Center,
    );
    push_text(
        &mut scene.instances,
        "TAP TO PLAY AGAIN",
        DESIGN_W / 2.0,
        540.0,
        2.0,
        CARD_BORDER,
        Align::Center,
    );
}
