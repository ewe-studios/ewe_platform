//! Visual theme for the REPL: the colours, border and padding of the input box
//! and of the output region printed above it.
//!
//! WHY: the input area and the output region must be told apart at a glance.
//! A REPL that paints prompt, echo and response in one undifferentiated stream
//! forces the reader to parse text to work out which part they typed.
//!
//! WHAT: [`ReplTheme`] describes a boxed input area (border + padding + a
//! lighter background) sitting below an output region painted in a darker
//! shade, plus the foreground colours for prompt, body text and errors.
//!
//! HOW: colours are expressed with [`ReplColor`], a backend-neutral type that
//! covers the three things a terminal understands — inherit the terminal's own
//! colour, an ANSI palette index, or a 24-bit RGB triple. The native renderer
//! converts these into its own colour type, so no rendering backend leaks into
//! this crate's public API.

use core::fmt;
use core::time::Duration;

/// The reminder shown in the input box's bottom border by default.
///
/// Kept to the keys a user is most likely to want and least likely to guess:
/// clearing the line, adding a newline instead of submitting, and abandoning
/// the line.
const DEFAULT_HINT: &str = " ctrl+u clear · shift+enter newline · ctrl+c cancel ";

/// Frames of the default spinner: a braille dot orbiting a cell.
const DEFAULT_SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How often the default spinner advances.
const DEFAULT_SPINNER_INTERVAL: Duration = Duration::from_millis(80);

/// A colour for one part of the REPL chrome.
///
/// WHY: the crate must describe colours without binding its public API to
/// whichever terminal library renders them.
///
/// WHAT: either "leave it to the terminal", one of the 256 ANSI palette
/// indices, or a 24-bit RGB triple.
///
/// HOW: the native backend maps each variant onto its renderer's colour type.
/// Terminals that cannot do truecolour degrade [`ReplColor::Rgb`] themselves,
/// so prefer [`ReplColor::Ansi`] when targeting limited terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ReplColor {
    /// Inherit whatever the terminal already uses.
    #[default]
    Default,
    /// An index into the 256-colour ANSI palette.
    Ansi(u8),
    /// A 24-bit truecolour value.
    Rgb(u8, u8, u8),
}

impl ReplColor {
    /// Build a truecolour value.
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::Rgb(red, green, blue)
    }

    /// Build an ANSI 256-colour palette value.
    #[must_use]
    pub const fn ansi(index: u8) -> Self {
        Self::Ansi(index)
    }
}


impl fmt::Display for ReplColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Ansi(index) => write!(f, "ansi({index})"),
            Self::Rgb(red, green, blue) => write!(f, "#{red:02x}{green:02x}{blue:02x}"),
        }
    }
}

/// The glyph set used to draw the input box border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderKind {
    /// No border at all — padding and background shading only.
    None,
    /// Square corners: `┌ ─ ┐ │ └ ┘`.
    Plain,
    /// Rounded corners: `╭ ─ ╮ │ ╰ ╯`.
    #[default]
    Rounded,
    /// Heavy lines: `┏ ━ ┓ ┃ ┗ ┛`.
    Thick,
    /// Double lines: `╔ ═ ╗ ║ ╚ ╝`.
    Double,
}

impl fmt::Display for BorderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::None => "none",
            Self::Plain => "plain",
            Self::Rounded => "rounded",
            Self::Thick => "thick",
            Self::Double => "double",
        };
        f.write_str(name)
    }
}

/// Blank space held between the input box border and the text inside it.
///
/// WHY: text pressed against a border is hard to read and makes the box look
/// like a table cell rather than a field you type into.
///
/// WHAT: per-side padding in terminal cells. `left`/`right` are columns,
/// `top`/`bottom` are rows, and every padded cell is painted in the input
/// background colour so the box reads as one solid area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplPadding {
    /// Columns of padding on the left, between border and prompt.
    pub left: u16,
    /// Columns of padding on the right, between text and border.
    pub right: u16,
    /// Rows of padding above the first line of input.
    pub top: u16,
    /// Rows of padding below the last line of input.
    pub bottom: u16,
}

impl ReplPadding {
    /// Padding with the same value on every side.
    #[must_use]
    pub const fn uniform(value: u16) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    /// Padding with independent horizontal and vertical values.
    #[must_use]
    pub const fn symmetric(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    /// Total columns consumed by left and right padding together.
    #[must_use]
    pub const fn horizontal(&self) -> u16 {
        self.left + self.right
    }

    /// Total rows consumed by top and bottom padding together.
    #[must_use]
    pub const fn vertical(&self) -> u16 {
        self.top + self.bottom
    }
}

impl Default for ReplPadding {
    fn default() -> Self {
        Self::symmetric(2, 1)
    }
}

impl fmt::Display for ReplPadding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "l{} r{} t{} b{}",
            self.left, self.right, self.top, self.bottom
        )
    }
}

/// Look and pace of the activity indicator.
///
/// WHY: spinner glyphs a font cannot render turn into empty boxes, so the
/// frames have to be replaceable — as does the tick rate, which is both a
/// readability and a CPU-cost decision.
///
/// WHAT: the animation frames, how long each is shown, the colours of the
/// animation and its label, and whether elapsed time is shown.
///
/// HOW: frames are cycled in order. An empty frame list disables the animation
/// and leaves the label alone, which is the right setting for a terminal that
/// is being logged to a file.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use foundation_repl::ActivityStyle;
///
/// // An ASCII-only spinner for terminals without braille glyphs.
/// let style = ActivityStyle {
///     frames: ["|", "/", "-", "\\"].iter().map(|f| (*f).to_string()).collect(),
///     interval: Duration::from_millis(120),
///     ..ActivityStyle::default()
/// };
/// assert_eq!(style.frames.len(), 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityStyle {
    /// Frames cycled through, in order. Empty disables the animation.
    pub frames: Vec<String>,
    /// How long each frame is shown.
    pub interval: Duration,
    /// Colour of the animated frame and of a progress bar's filled part.
    pub foreground: ReplColor,
    /// Colour of the label beside the animation.
    pub label_foreground: ReplColor,
    /// Colour of the elapsed-time readout.
    pub elapsed_foreground: ReplColor,
    /// Whether to show how long the work has been running.
    pub show_elapsed: bool,
}

impl Default for ActivityStyle {
    fn default() -> Self {
        Self {
            frames: DEFAULT_SPINNER_FRAMES
                .iter()
                .map(|frame| (*frame).into())
                .collect(),
            interval: DEFAULT_SPINNER_INTERVAL,
            foreground: ReplColor::rgb(0x5a, 0xa0, 0x6e),
            label_foreground: ReplColor::rgb(0xc8, 0xc8, 0xc8),
            elapsed_foreground: ReplColor::rgb(0x7a, 0x7a, 0x7a),
            show_elapsed: true,
        }
    }
}

impl fmt::Display for ActivityStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ActivityStyle({} frames every {:?}, elapsed {})",
            self.frames.len(),
            self.interval,
            self.show_elapsed
        )
    }
}

/// Colours, border and spacing for the whole REPL surface.
///
/// WHY: the input area and the output region need to be visually distinct, and
/// which shades achieve that depends on the user's terminal — so every part is
/// configurable rather than baked in.
///
/// WHAT: the defaults render a grey-bordered box with a light-charcoal
/// interior for input, and paint responses full width in a near-black shade so
/// output is unmistakably the darker of the two regions.
///
/// HOW: pass one to [`ReplBuilder::theme`](crate::ReplBuilder::theme), or tweak
/// individual fields on [`ReplTheme::default()`]. Colours are only emitted when
/// the `colors` feature is on (it is on by default); with it off the box is
/// still drawn, just without any styling.
///
/// # Examples
///
/// ```
/// use foundation_repl::{BorderKind, ReplColor, ReplPadding, ReplTheme};
///
/// let theme = ReplTheme {
///     border: ReplColor::rgb(0x7a, 0x7a, 0x7a),
///     border_kind: BorderKind::Thick,
///     padding: ReplPadding::uniform(2),
///     ..ReplTheme::default()
/// };
/// assert_eq!(theme.padding.horizontal(), 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplTheme {
    /// Colour of the box border drawn around the input area.
    pub border: ReplColor,
    /// Glyph set used for that border.
    pub border_kind: BorderKind,
    /// Background painted across the whole input box, padding included.
    pub input_background: ReplColor,
    /// Colour of the text being typed.
    pub input_foreground: ReplColor,
    /// Colour of the prompt (and continuation prompt) inside the box.
    pub prompt_foreground: ReplColor,
    /// Background painted behind responses, full terminal width.
    pub output_background: ReplColor,
    /// Colour of response text.
    pub output_foreground: ReplColor,
    /// Colour of error text.
    pub error_foreground: ReplColor,
    /// Colour of the startup banner.
    pub banner_foreground: ReplColor,
    /// Space between the border and the text inside the box.
    pub padding: ReplPadding,
    /// Columns of indent before response text, inside the painted background.
    pub output_indent: u16,
    /// Blank shaded rows printed above and below each response block.
    pub output_margin: u16,
    /// Most text rows the box may grow to before it scrolls internally.
    ///
    /// Counts text rows only — border and padding are extra. Long or multiline
    /// input beyond this scrolls within the box so the cursor stays visible.
    pub max_input_rows: u16,
    /// Look and pace of the activity indicator.
    pub activity: ActivityStyle,
    /// Reminder drawn into the input box's bottom border.
    ///
    /// WHY it lives in the border: the keys that edit the line are worth
    /// advertising — Ctrl+U in particular is not guessable — but a reminder
    /// that cost a row of its own would push the conversation up the screen on
    /// every redraw. The bottom border is already being drawn.
    ///
    /// `None` draws no hint. Text wider than the box is truncated, so keep it
    /// short. Only the input box carries it; the activity indicator does not.
    pub hint: Option<String>,
    /// Colour of that reminder.
    pub hint_foreground: ReplColor,
}

impl ReplTheme {
    /// Deep violet-black with soft mint, sky and rose accents. The default.
    ///
    /// WHY these values: every foreground clears WCAG AA against the surface it
    /// is actually drawn on (body text lands between 9:1 and 12:1), while the
    /// accents stay desaturated enough not to glare in a dark room. The two
    /// surfaces are held about 1.3:1 apart in luminance — far enough to read as
    /// separate regions, close enough that neither looks like a pasted-in
    /// rectangle. `tests/palette.rs` asserts all of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_repl::ReplTheme;
    ///
    /// assert_eq!(ReplTheme::nebula(), ReplTheme::default());
    /// ```
    #[must_use]
    pub fn nebula() -> Self {
        Self {
            border: ReplColor::rgb(0x5f, 0x59, 0x80),
            border_kind: BorderKind::Rounded,
            input_background: ReplColor::rgb(0x26, 0x26, 0x3a),
            input_foreground: ReplColor::rgb(0xe6, 0xe1, 0xf2),
            prompt_foreground: ReplColor::rgb(0xa3, 0xe8, 0xc6),
            output_background: ReplColor::rgb(0x0d, 0x0d, 0x14),
            output_foreground: ReplColor::rgb(0xc5, 0xbf, 0xda),
            error_foreground: ReplColor::rgb(0xff, 0x9f, 0xb2),
            banner_foreground: ReplColor::rgb(0x8f, 0x88, 0xad),
            padding: ReplPadding::default(),
            output_indent: 2,
            output_margin: 1,
            max_input_rows: 10,
            activity: ActivityStyle {
                foreground: ReplColor::rgb(0x8f, 0xd9, 0xf2),
                label_foreground: ReplColor::rgb(0xd6, 0xd1, 0xe8),
                elapsed_foreground: ReplColor::rgb(0x7d, 0x76, 0x99),
                ..ActivityStyle::default()
            },
            hint: Some(DEFAULT_HINT.into()),
            hint_foreground: ReplColor::rgb(0x73, 0x6c, 0x94),
        }
    }

    /// Warm near-black with peach and amber accents.
    ///
    /// The same contrast discipline as [`ReplTheme::nebula`], shifted warm for
    /// anyone who finds blue-cast palettes harsh late at night.
    #[must_use]
    pub fn dusk() -> Self {
        Self {
            border: ReplColor::rgb(0x6b, 0x5c, 0x56),
            input_background: ReplColor::rgb(0x2d, 0x27, 0x24),
            input_foreground: ReplColor::rgb(0xf0, 0xe6, 0xe0),
            prompt_foreground: ReplColor::rgb(0xff, 0xc9, 0xa3),
            output_background: ReplColor::rgb(0x12, 0x10, 0x0f),
            output_foreground: ReplColor::rgb(0xd8, 0xc9, 0xc2),
            error_foreground: ReplColor::rgb(0xff, 0x9e, 0x9e),
            banner_foreground: ReplColor::rgb(0xa8, 0x92, 0x89),
            activity: ActivityStyle {
                foreground: ReplColor::rgb(0xff, 0xd6, 0xa5),
                label_foreground: ReplColor::rgb(0xe8, 0xdc, 0xd5),
                elapsed_foreground: ReplColor::rgb(0x8a, 0x77, 0x70),
                ..ActivityStyle::default()
            },
            hint_foreground: ReplColor::rgb(0x82, 0x6f, 0x66),
            ..Self::nebula()
        }
    }

    /// Deep teal-black with aqua and mint accents.
    ///
    /// The coolest of the three, and the highest contrast — the easiest to read
    /// on a dim or washed-out display.
    #[must_use]
    pub fn abyss() -> Self {
        Self {
            border: ReplColor::rgb(0x3d, 0x5a, 0x61),
            input_background: ReplColor::rgb(0x1a, 0x2a, 0x30),
            input_foreground: ReplColor::rgb(0xdc, 0xee, 0xf2),
            prompt_foreground: ReplColor::rgb(0xa5, 0xf0, 0xd4),
            output_background: ReplColor::rgb(0x08, 0x0f, 0x12),
            output_foreground: ReplColor::rgb(0xb8, 0xd4, 0xda),
            error_foreground: ReplColor::rgb(0xff, 0xa8, 0xb8),
            banner_foreground: ReplColor::rgb(0x7f, 0xa3, 0xab),
            activity: ActivityStyle {
                foreground: ReplColor::rgb(0x9e, 0xe0, 0xff),
                label_foreground: ReplColor::rgb(0xd0, 0xe6, 0xec),
                elapsed_foreground: ReplColor::rgb(0x6b, 0x8e, 0x96),
                ..ActivityStyle::default()
            },
            hint_foreground: ReplColor::rgb(0x4f, 0x70, 0x78),
            ..Self::nebula()
        }
    }

    /// The neutral greyscale this crate shipped before the pastel palettes.
    ///
    /// Kept for terminals with a heavily customised colour scheme, where a
    /// palette that picks its own hues fights whatever is already there.
    ///
    /// Its input surface is `#262626` rather than the `#1e1e1e` it originally
    /// shipped with: the two surfaces used to sit only 1.19:1 apart, close
    /// enough that the box and the output behind it blurred into one region.
    #[must_use]
    pub fn slate() -> Self {
        Self {
            border: ReplColor::rgb(0x5a, 0x5a, 0x5a),
            input_background: ReplColor::rgb(0x26, 0x26, 0x26),
            input_foreground: ReplColor::rgb(0xe6, 0xe6, 0xe6),
            prompt_foreground: ReplColor::rgb(0x5a, 0xa0, 0x6e),
            output_background: ReplColor::rgb(0x0a, 0x0a, 0x0a),
            output_foreground: ReplColor::rgb(0xc8, 0xc8, 0xc8),
            error_foreground: ReplColor::rgb(0xdc, 0x5a, 0x5a),
            banner_foreground: ReplColor::rgb(0x8a, 0x8a, 0x8a),
            activity: ActivityStyle {
                foreground: ReplColor::rgb(0x5a, 0xa0, 0x6e),
                label_foreground: ReplColor::rgb(0xc8, 0xc8, 0xc8),
                elapsed_foreground: ReplColor::rgb(0x7a, 0x7a, 0x7a),
                ..ActivityStyle::default()
            },
            hint_foreground: ReplColor::rgb(0x6a, 0x6a, 0x6a),
            ..Self::nebula()
        }
    }

    /// Every named palette, for callers that want to offer a choice.
    #[must_use]
    pub fn palettes() -> [(&'static str, Self); 4] {
        [
            ("nebula", Self::nebula()),
            ("dusk", Self::dusk()),
            ("abyss", Self::abyss()),
            ("slate", Self::slate()),
        ]
    }

    /// Look a palette up by name, case-insensitively.
    ///
    /// The non-committal form: returns `None` for an unknown name and leaves
    /// the caller to decide what that means. [`ReplTheme::named`] and
    /// [`ReplTheme::from_env`] are the two opinionated forms.
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        Self::palettes()
            .into_iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, theme)| theme)
    }

    /// The palette called `name`, panicking if there is no such palette.
    ///
    /// WHY it panics where [`ReplTheme::from_env`] does not: this name comes
    /// from the program author, baked into the call site, so an unknown one is a
    /// typo that will never work and should surface on the first run rather
    /// than silently drawing a palette nobody chose.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_repl::ReplTheme;
    ///
    /// assert_eq!(ReplTheme::named("abyss"), ReplTheme::abyss());
    /// ```
    ///
    /// # Panics
    /// Panics if `name` is not one of [`ReplTheme::palettes`], listing the
    /// names that would have worked.
    #[must_use]
    pub fn named(name: &str) -> Self {
        Self::by_name(name).unwrap_or_else(|| {
            panic!(
                "unknown REPL palette {name:?}; available palettes: {}",
                Self::palette_names()
            )
        })
    }

    /// The palette named by the `variable` environment variable.
    ///
    /// WHY it does not panic where [`ReplTheme::named`] does: this name comes
    /// from whoever is running the program, and refusing to start over a
    /// mistyped colour scheme turns a cosmetic preference into an outage. An
    /// unknown value is logged and the default is used.
    ///
    /// Unset means "no preference" and yields [`ReplTheme::default`]. Use
    /// [`ReplTheme::from_env_or`] to fall back to something else.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_repl::ReplTheme;
    ///
    /// // Nothing set, so the default is used.
    /// assert_eq!(ReplTheme::from_env("REPL_THEME_DOC_EXAMPLE"), ReplTheme::default());
    /// ```
    #[must_use]
    pub fn from_env(variable: &str) -> Self {
        Self::from_env_or(variable, Self::default())
    }

    /// The palette named by the `variable` environment variable, or `fallback`.
    ///
    /// Behaves like [`ReplTheme::from_env`] but lets an application pick its
    /// own house palette as the starting point instead of the crate default.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_repl::ReplTheme;
    ///
    /// let theme = ReplTheme::from_env_or("REPL_THEME_DOC_EXAMPLE_2", ReplTheme::dusk());
    /// assert_eq!(theme, ReplTheme::dusk());
    /// ```
    #[must_use]
    pub fn from_env_or(variable: &str, fallback: Self) -> Self {
        let Ok(name) = std::env::var(variable) else {
            return fallback;
        };

        // An empty or whitespace-only value is what an unset shell variable
        // looks like after `FOO= cmd`, so treat it as "no preference" too.
        if name.trim().is_empty() {
            return fallback;
        }

        Self::by_name(name.trim()).unwrap_or_else(|| {
            tracing::warn!(
                "unknown palette {name:?} in {variable}; using the fallback. \
                 Available palettes: {}",
                Self::palette_names()
            );
            fallback
        })
    }

    /// The known palette names, comma separated, for diagnostics.
    fn palette_names() -> String {
        Self::palettes()
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Default for ReplTheme {
    fn default() -> Self {
        Self::nebula()
    }
}

impl fmt::Display for ReplTheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReplTheme(border {} {}, input bg {}, output bg {}, padding {}, max rows {})",
            self.border_kind,
            self.border,
            self.input_background,
            self.output_background,
            self.padding,
            self.max_input_rows
        )
    }
}
