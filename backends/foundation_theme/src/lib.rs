//! # `foundation_theme`
//!
//! WHY: A theme is a table of design tokens (colors, spacing, radii, …) and the
//! CSS those tokens imply (`:root` custom properties, a dark-mode override
//! block, utility classes). That CSS must be generated the SAME way no matter
//! how the tokens were declared — the `theme!{}` macro (compile time), the
//! `#[derive(ThemeTokens)]` form (compile time), or the [`Theme`] builder
//! (runtime). This crate is that single source of truth (spec-39 decision 021).
//!
//! WHAT: [`ThemeToken`] (one token), [`GeneratedTheme`] (token table + its CSS,
//! `const`-capable so the macro can emit it in `const` position), the runtime
//! [`Theme`] builder, and [`theme_css`] — the ONE generator.
//!
//! HOW: `no_std + alloc`. Generation is pure string assembly; the macro calls
//! [`theme_css`] at expansion time and embeds the result as a string literal,
//! the builder calls it at runtime. No dependency on the DOM/runtime crates.

#![no_std]

extern crate alloc;

pub mod curve;
pub mod palette;

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// One design token: a `name` within a `category`, a `light` value, and an
/// optional explicit `dark` value (auto-derived for colors when absent).
///
/// All fields are `Cow<'static, str>` so the same type serves the macro
/// (`Cow::Borrowed` literals, `const`-constructible) and the runtime builder
/// (`Cow::Owned`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeToken {
    /// Token name (kebab-cased; e.g. `"primary"`, `"md"`).
    pub name: Cow<'static, str>,
    /// Category (`"color"`, `"spacing"`, `"padding"`, `"margin"`, `"radius"`,
    /// `"shadow"`, `"font-size"`, `"animation"`).
    pub category: Cow<'static, str>,
    /// The light-mode (default) value.
    pub light: Cow<'static, str>,
    /// Explicit dark-mode value, if any.
    pub dark: Option<Cow<'static, str>>,
}

impl ThemeToken {
    /// `const` constructor for macro-emitted (`'static`) tokens.
    #[must_use]
    pub const fn from_static(
        name: &'static str,
        category: &'static str,
        light: &'static str,
        dark: Option<&'static str>,
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            category: Cow::Borrowed(category),
            light: Cow::Borrowed(light),
            // `Option::map` is not const; match instead.
            dark: match dark {
                Some(d) => Some(Cow::Borrowed(d)),
                None => None,
            },
        }
    }

    /// Owned constructor for the runtime builder / proc-macro host code.
    #[must_use]
    pub fn owned(
        name: impl Into<String>,
        category: impl Into<String>,
        light: impl Into<String>,
        dark: Option<String>,
    ) -> Self {
        Self {
            name: Cow::Owned(name.into()),
            category: Cow::Owned(category.into()),
            light: Cow::Owned(light.into()),
            dark: dark.map(Cow::Owned),
        }
    }
}

/// A complete theme: the token table plus the CSS it generates.
///
/// `const`-capable via [`from_static`](Self::from_static) (what `theme!{}`
/// emits); the runtime [`Theme`] builder produces the owned form. Either way
/// downstream code is uniform — [`App::theme`] takes this and injects
/// [`css`](Self::css) into `<head>`.
///
/// [`App::theme`]: https://docs.rs/foundation_wasm_ui
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedTheme {
    css: Cow<'static, str>,
    tokens: Cow<'static, [ThemeToken]>,
}

impl GeneratedTheme {
    /// `const` constructor — the `theme!{}` macro emits this with a
    /// compile-time-generated CSS literal and a static token slice.
    #[must_use]
    pub const fn from_static(css: &'static str, tokens: &'static [ThemeToken]) -> Self {
        Self {
            css: Cow::Borrowed(css),
            tokens: Cow::Borrowed(tokens),
        }
    }

    /// Build from owned tokens, generating the CSS now (the runtime path).
    #[must_use]
    pub fn from_tokens(tokens: Vec<ThemeToken>) -> Self {
        let css = theme_css(&tokens);
        Self {
            css: Cow::Owned(css),
            tokens: Cow::Owned(tokens),
        }
    }

    /// The generated stylesheet — inject this into `<head>`.
    #[must_use]
    pub fn to_css(&self) -> &str {
        &self.css
    }

    /// The parsed token table — inspectable for tooling/tests.
    #[must_use]
    pub fn tokens(&self) -> &[ThemeToken] {
        &self.tokens
    }

    /// Look up a token's light value by category + name.
    #[must_use]
    pub fn value(&self, category: &str, name: &str) -> Option<&str> {
        self.tokens
            .iter()
            .find(|t| t.category == category && t.name == name)
            .map(|t| t.light.as_ref())
    }
}

/// Ergonomic runtime builder — declare a theme from values (config-driven, no
/// macro). `build()` runs the same [`theme_css`] as the macro.
///
/// ```
/// use foundation_theme::Theme;
/// let theme = Theme::new()
///     .color("primary", "#3b82f6", Some("#60a5fa"))
///     .spacing("md", "16px")
///     .build();
/// assert!(theme.to_css().contains("--color-primary: #3b82f6;"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Theme {
    tokens: Vec<ThemeToken>,
}

impl Theme {
    /// A new, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Add a color token (`dark = None` auto-derives the dark value).
    #[must_use]
    pub fn color(
        self,
        name: impl Into<String>,
        light: impl Into<String>,
        dark: Option<&str>,
    ) -> Self {
        self.token("color", name, light, dark)
    }

    /// Add a spacing token (usable as `.p-*` and `.m-*`).
    #[must_use]
    pub fn spacing(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.token("spacing", name, value, None)
    }

    /// Add a border-radius token (`.rounded-*`).
    #[must_use]
    pub fn radius(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.token("radius", name, value, None)
    }

    /// Add a box-shadow token (`.shadow-*`).
    #[must_use]
    pub fn shadow(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.token("shadow", name, value, None)
    }

    /// Add a token in any category (escape hatch for `padding`, `margin`,
    /// `font-size`, `animation`, or custom categories).
    #[must_use]
    pub fn token(
        mut self,
        category: impl Into<String>,
        name: impl Into<String>,
        value: impl Into<String>,
        dark: Option<&str>,
    ) -> Self {
        self.tokens.push(ThemeToken::owned(
            name,
            category,
            value,
            dark.map(String::from),
        ));
        self
    }

    /// Finalize: generate the CSS and return the [`GeneratedTheme`].
    #[must_use]
    pub fn build(self) -> GeneratedTheme {
        GeneratedTheme::from_tokens(self.tokens)
    }
}

/// Built-in utility classes (feature 09 §7.2) — token-independent.
const BUILTIN_UTILITIES: &str = "\
.relative { position: relative; }\n\
.absolute { position: absolute; }\n\
.fixed { position: fixed; }\n\
.flex { display: flex; }\n\
.grid { display: grid; }\n\
.block { display: block; }\n\
.inline { display: inline; }\n\
.w-full { width: 100%; }\n\
.h-screen { height: 100vh; }\n\
.max-w-md { max-width: 768px; }\n\
.text-sm { font-size: 0.875rem; }\n\
.text-lg { font-size: 1.125rem; }\n\
.font-bold { font-weight: 700; }\n\
.text-center { text-align: center; }\n\
.border { border-width: 1px; }\n\
.hidden { display: none; }\n\
.visible { visibility: visible; }\n";

/// How a scale index `n` maps to a CSS value — the calculable formula family
/// (see the token-scale design guide, feature 09).
/// A measured unit for a scale value (feature 09 token-scale guide). The
/// `suffix` is appended to the class (`.text-16-rem`); an EMPTY suffix marks
/// the bare/default unit (`.opacity-50`, `.w-50` = `50%`) — those carry no
/// unit-tagged twin.
#[derive(Clone, Copy)]
enum Unit {
    /// Proportion `n/100` (`5 → 0.05`, `100 → 1`); bare. For opacity.
    Ratio,
    /// The number verbatim; bare. For unitless enumerations (font-weight).
    Raw,
    /// Percent `n%`; bare (the default sizing unit). For width/height.
    Pct,
    /// `npx`, suffix `px`.
    Px,
    /// `nrem`, suffix `rem`.
    Rem,
    /// `nem`, suffix `em`.
    Em,
    /// `nvh`, suffix `vh`.
    Vh,
    /// `nvw`, suffix `vw`.
    Vw,
}

impl Unit {
    /// Class suffix; empty = bare/default (no `-unit` twin emitted).
    fn suffix(self) -> &'static str {
        match self {
            Unit::Ratio | Unit::Raw | Unit::Pct => "",
            Unit::Px => "px",
            Unit::Rem => "rem",
            Unit::Em => "em",
            Unit::Vh => "vh",
            Unit::Vw => "vw",
        }
    }

    /// The CSS value for index `n` under this unit.
    fn value(self, n: u32) -> String {
        match self {
            Unit::Ratio => match n {
                0 => String::from("0"),
                100 => String::from("1"),
                p => alloc::format!("0.{p:02}"),
            },
            Unit::Raw => alloc::format!("{n}"),
            Unit::Pct => alloc::format!("{n}%"),
            Unit::Px => alloc::format!("{n}px"),
            Unit::Rem => alloc::format!("{n}rem"),
            Unit::Em => alloc::format!("{n}em"),
            Unit::Vh => alloc::format!("{n}vh"),
            Unit::Vw => alloc::format!("{n}vw"),
        }
    }
}

/// One generated utility scale: `.{prefix}-{n}` (bare, primary unit) plus
/// `.{prefix}-{n}-{unit}` for every unit-tagged variant, over `start..=end`
/// stepping by `step`. `units[0]` is the primary (drives the bare class).
struct Scale {
    prefix: &'static str,
    property: &'static str,
    start: u32,
    end: u32,
    step: u32,
    units: &'static [Unit],
}

/// The calculable scale table (feature 09 token-scale design guide). Every
/// value is a pure function of its index `n`. Spatial/text scales carry
/// `px/rem/em/vh` variants so users pick the measured property
/// (`.text-56-rem`, `.h-50-vh`); the bare class uses the primary unit.
const SCALES: &[Scale] = &[
    Scale { prefix: "opacity", property: "opacity", start: 0, end: 100, step: 5, units: &[Unit::Ratio] },
    Scale { prefix: "w", property: "width", start: 0, end: 100, step: 5, units: &[Unit::Pct, Unit::Vw, Unit::Vh] },
    Scale { prefix: "h", property: "height", start: 0, end: 100, step: 5, units: &[Unit::Pct, Unit::Vh, Unit::Vw] },
    Scale { prefix: "p", property: "padding", start: 0, end: 64, step: 4, units: &[Unit::Px, Unit::Rem] },
    Scale { prefix: "m", property: "margin", start: 0, end: 64, step: 4, units: &[Unit::Px, Unit::Rem] },
    Scale { prefix: "gap", property: "gap", start: 0, end: 64, step: 4, units: &[Unit::Px, Unit::Rem] },
    Scale { prefix: "text", property: "font-size", start: 8, end: 72, step: 2, units: &[Unit::Px, Unit::Rem, Unit::Em, Unit::Vh] },
    Scale { prefix: "border", property: "border-width", start: 0, end: 8, step: 1, units: &[Unit::Px, Unit::Rem, Unit::Em] },
    Scale { prefix: "font", property: "font-weight", start: 100, end: 900, step: 100, units: &[Unit::Raw] },
];

/// Append every generated scale. Named tokens (`.p-md`, `.text-primary`) come
/// from the theme's own tokens; these numeric variants (`.p-16`, `.p-16-rem`,
/// `.opacity-50`, `.text-56-rem`) coexist by namespace (numbers vs names never
/// collide). Each step emits the bare primary class plus one class per
/// unit-tagged variant.
fn append_scales(css: &mut String) {
    for scale in SCALES {
        let mut n = scale.start;
        while n <= scale.end {
            // Bare class from the primary unit (`.text-16`, `.w-50`, `.opacity-50`).
            let primary = scale.units[0];
            let _ = writeln!(
                css,
                ".{}-{n} {{ {}: {}; }}",
                scale.prefix,
                scale.property,
                primary.value(n)
            );
            // Unit-tagged variants (`.text-16-rem`, `.h-50-vh`, …).
            for unit in scale.units {
                let suffix = unit.suffix();
                if !suffix.is_empty() {
                    let _ = writeln!(
                        css,
                        ".{}-{n}-{suffix} {{ {}: {}; }}",
                        scale.prefix,
                        scale.property,
                        unit.value(n)
                    );
                }
            }
            n += scale.step;
        }
    }
}

/// The variable prefix for a category (`--{prefix}-{name}`).
fn var_prefix(category: &str) -> &str {
    category
}

/// The ONE CSS generator: `:root` custom properties, a dark `@media` override
/// block (colors only — explicit values verbatim, missing ones auto-derived),
/// per-category utility classes, and the built-in utility set.
#[must_use]
pub fn theme_css(tokens: &[ThemeToken]) -> String {
    let var_name = |t: &ThemeToken| {
        let mut s = String::from("--");
        s.push_str(var_prefix(&t.category));
        s.push('-');
        s.push_str(&t.name);
        s
    };

    let mut css = String::from(":root {\n");
    for token in tokens {
        let _ = writeln!(css, "  {}: {};", var_name(token), token.light);
    }
    css.push_str("}\n");

    // Dark block: colors only — explicit where given, auto-derived otherwise.
    let mut dark = String::new();
    for token in tokens.iter().filter(|t| t.category == "color") {
        let value = match &token.dark {
            Some(d) => Some(d.as_ref().into()),
            None => auto_dark(&token.light),
        };
        if let Some(value) = value {
            let _ = writeln!(dark, "    {}: {};", var_name(token), value);
        }
    }
    if !dark.is_empty() {
        css.push_str("@media (prefers-color-scheme: dark) {\n  :root {\n");
        css.push_str(&dark);
        css.push_str("  }\n}\n");
    }

    // Utility classes per category (feature 09 §7.1).
    for token in tokens {
        let var = var_name(token);
        let name = &token.name;
        match token.category.as_ref() {
            "color" => {
                let _ = writeln!(css, ".bg-{name} {{ background-color: var({var}); }}");
                let _ = writeln!(css, ".text-{name} {{ color: var({var}); }}");
                let _ = writeln!(css, ".border-{name} {{ border-color: var({var}); }}");
            }
            "spacing" => {
                let _ = writeln!(css, ".p-{name} {{ padding: var({var}); }}");
                let _ = writeln!(css, ".m-{name} {{ margin: var({var}); }}");
            }
            "padding" => {
                let _ = writeln!(css, ".p-{name} {{ padding: var({var}); }}");
            }
            "margin" => {
                let _ = writeln!(css, ".m-{name} {{ margin: var({var}); }}");
            }
            "radius" => {
                let _ = writeln!(css, ".rounded-{name} {{ border-radius: var({var}); }}");
            }
            "shadow" => {
                let _ = writeln!(css, ".shadow-{name} {{ box-shadow: var({var}); }}");
            }
            "font-size" => {
                let _ = writeln!(css, ".text-{name} {{ font-size: var({var}); }}");
            }
            "animation" => {
                let _ = writeln!(css, ".duration-{name} {{ transition-duration: var({var}); }}");
            }
            _ => {}
        }
    }
    css.push_str(BUILTIN_UTILITIES);
    append_scales(&mut css);
    css
}

/// Auto-derive a dark value from a light `#rrggbb` (or `#rgb`) color at ~80%
/// luminance (decision 020's worked example within rounding). Non-hex values
/// return `None` — no auto-generation for non-colors or `rgb()/var()` forms.
#[must_use]
pub fn auto_dark(light: &str) -> Option<String> {
    let hex = light.strip_prefix('#')?;
    let component = |slice: &str| u8::from_str_radix(slice, 16).ok();
    let rgb: Option<[u8; 3]> = match hex.len() {
        3 => {
            let mut out = [0u8; 3];
            for (i, c) in hex.chars().enumerate() {
                let mut buf = [0u8; 8];
                let s = c.encode_utf8(&mut buf);
                // duplicate the nibble: `f` -> `ff`
                let doubled = u8::from_str_radix(&alloc::format!("{s}{s}"), 16).ok()?;
                out[i] = doubled;
            }
            Some(out)
        }
        6 => Some([
            component(&hex[0..2])?,
            component(&hex[2..4])?,
            component(&hex[4..6])?,
        ]),
        _ => None,
    };
    let rgb = rgb?;
    // 80% luminance, integer round-half-up (no_std has no f32::round): the
    // worked example #10b981 -> #0d9467 holds exactly.
    #[allow(clippy::cast_possible_truncation)]
    let dark = |v: u8| ((u16::from(v) * 8 + 5) / 10) as u8;
    Some(alloc::format!(
        "#{:02x}{:02x}{:02x}",
        dark(rgb[0]),
        dark(rgb[1]),
        dark(rgb[2])
    ))
}
