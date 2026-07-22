//! Legibility checks for the built-in palettes.
//!
//! WHY this is a test and not a note in a doc comment: "easy on the eyes" is
//! measurable, and a palette is exactly the kind of thing someone tweaks by
//! feel months later. Every foreground here is checked against the surface it
//! is really drawn on, so a change that looks fine on one monitor cannot
//! quietly ship something unreadable on another.
//!
//! The measure is the WCAG relative-luminance contrast ratio. Body text is held
//! to AA (4.5:1); chrome that is meant to recede — borders, hints, the elapsed
//! timer — is held to a lower floor so it stays secondary without disappearing.

use foundation_repl::{ReplColor, ReplTheme};

/// Relative luminance, per WCAG 2.x.
fn luminance(color: ReplColor) -> f64 {
    let (red, green, blue) = match color {
        ReplColor::Rgb(red, green, blue) => (red, green, blue),
        other => panic!("built-in palettes should use RGB so they can be measured, got {other}"),
    };

    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };

    0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue)
}

/// Contrast ratio between two colours, always >= 1.0.
fn contrast(foreground: ReplColor, background: ReplColor) -> f64 {
    let (a, b) = (luminance(foreground), luminance(background));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Text a user reads word by word must clear WCAG AA.
const BODY_FLOOR: f64 = 4.5;

/// Chrome that is meant to recede, but still be seen.
const CHROME_FLOOR: f64 = 2.6;

/// The border only has to be findable, not readable.
const BORDER_FLOOR: f64 = 1.9;

/// How far apart the two surfaces must sit to read as separate regions.
const SURFACE_FLOOR: f64 = 1.25;

#[test]
fn every_palette_keeps_body_text_readable() {
    for (name, theme) in ReplTheme::palettes() {
        let checks = [
            ("input_foreground", theme.input_foreground, theme.input_background),
            ("prompt_foreground", theme.prompt_foreground, theme.input_background),
            ("output_foreground", theme.output_foreground, theme.output_background),
            ("error_foreground", theme.error_foreground, theme.output_background),
            (
                "activity.foreground",
                theme.activity.foreground,
                theme.input_background,
            ),
            (
                "activity.label_foreground",
                theme.activity.label_foreground,
                theme.input_background,
            ),
        ];

        for (field, foreground, background) in checks {
            let ratio = contrast(foreground, background);
            assert!(
                ratio >= BODY_FLOOR,
                "{name}: {field} ({foreground}) on {background} is {ratio:.2}:1, \
                 below the {BODY_FLOOR}:1 needed to read comfortably"
            );
        }
    }
}

#[test]
fn every_palette_keeps_secondary_chrome_visible_without_shouting() {
    for (name, theme) in ReplTheme::palettes() {
        let checks = [
            ("hint_foreground", theme.hint_foreground),
            ("activity.elapsed_foreground", theme.activity.elapsed_foreground),
        ];

        for (field, foreground) in checks {
            let ratio = contrast(foreground, theme.input_background);
            assert!(
                ratio >= CHROME_FLOOR,
                "{name}: {field} ({foreground}) is {ratio:.2}:1 against the input \
                 surface, too faint to notice"
            );
            assert!(
                ratio < BODY_FLOOR * 2.0,
                "{name}: {field} is {ratio:.2}:1 — chrome this loud competes with \
                 the text it sits under"
            );
        }

        let border = contrast(theme.border, theme.input_background);
        assert!(
            border >= BORDER_FLOOR,
            "{name}: the border is {border:.2}:1 against its own fill and would \
             disappear"
        );
    }
}

#[test]
fn every_palette_keeps_the_banner_readable() {
    for (name, theme) in ReplTheme::palettes() {
        let ratio = contrast(theme.banner_foreground, theme.output_background);
        assert!(
            ratio >= 3.0,
            "{name}: the banner is {ratio:.2}:1 and would be hard to read on \
             startup"
        );
    }
}

#[test]
fn the_input_surface_is_lighter_than_the_output_surface() {
    // The whole point of the box: the area you type into has to be the lighter
    // of the two, with output the darker shade behind it.
    for (name, theme) in ReplTheme::palettes() {
        let input = luminance(theme.input_background);
        let output = luminance(theme.output_background);

        assert!(
            input > output,
            "{name}: the input surface ({}) must be lighter than the output \
             surface ({})",
            theme.input_background,
            theme.output_background
        );

        let separation = contrast(theme.input_background, theme.output_background);
        assert!(
            separation >= SURFACE_FLOOR,
            "{name}: the two surfaces are only {separation:.2}:1 apart and would \
             blur into one region"
        );
    }
}

#[test]
fn every_palette_is_dark() {
    // These are for people who cannot stand light mode; a surface bright enough
    // to glare in a dark room is a bug, not a preference.
    for (name, theme) in ReplTheme::palettes() {
        for (field, surface) in [
            ("input_background", theme.input_background),
            ("output_background", theme.output_background),
        ] {
            let level = luminance(surface);
            assert!(
                level < 0.05,
                "{name}: {field} ({surface}) has luminance {level:.4} and is too \
                 bright for a dark-mode palette"
            );
        }
    }
}

#[test]
fn palettes_are_reachable_by_name() {
    assert_eq!(ReplTheme::by_name("nebula"), Some(ReplTheme::nebula()));
    assert_eq!(ReplTheme::by_name("ABYSS"), Some(ReplTheme::abyss()));
    assert_eq!(ReplTheme::by_name("Dusk"), Some(ReplTheme::dusk()));
    assert_eq!(ReplTheme::by_name("slate"), Some(ReplTheme::slate()));
    assert_eq!(ReplTheme::by_name("chartreuse"), None);
}

#[test]
fn the_default_is_the_pastel_palette() {
    assert_eq!(ReplTheme::default(), ReplTheme::nebula());
}

#[test]
fn every_palette_keeps_the_shared_layout_settings() {
    // Only colours differ between palettes; padding and sizing are one
    // decision, not four.
    let reference = ReplTheme::nebula();
    for (name, theme) in ReplTheme::palettes() {
        assert_eq!(theme.padding, reference.padding, "{name} changed the padding");
        assert_eq!(
            theme.max_input_rows, reference.max_input_rows,
            "{name} changed the box height limit"
        );
        assert_eq!(
            theme.output_indent, reference.output_indent,
            "{name} changed the output indent"
        );
        assert_eq!(
            theme.border_kind, reference.border_kind,
            "{name} changed the border style"
        );
    }
}
