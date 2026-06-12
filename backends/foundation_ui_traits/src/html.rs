//! WHY: The `html!` macro, the signal system, and the DOM-op encoders all need a
//! lightweight, dependency-free HTML tree plus a compact wire identity for tag and
//! attribute names. Known names are assigned `u16` IDs (decision G12/G13/G14) so the
//! wire carries 3 bytes instead of a string for the overwhelmingly common cases.
//!
//! WHAT: [`HtmlTag`] / [`AttrName`] (known-ID or owned-name), the known-name lookup
//! tables, the [`Html`] tree, and the [`IntoHtml`] trait with its 18 standard
//! implementations (decision 007).
//!
//! HOW: Known names live in two static tables where INDEX + 1 == ID (id 0 is
//! reserved/never assigned). The first four entries of each table are pinned by the
//! spec (`div`/`span`/`input`/`button`; `class`/`id`/`style`/`value`) — the JS
//! runtime mirrors these tables, so ORDER IS ABI: only append, never reorder.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Part;

// ─── Known tag IDs ─────────────────────────────────────────────────────────────
//
// `TAG_NAMES[i]` has id `i as u16 + 1`. The JS runtime keeps a mirrored array
// (foundation-wasm-ui.js `TAG_NAMES`) — append-only, order is ABI.

/// Known tag id for `div` (pinned by spec).
pub const TAG_DIV: u16 = 1;
/// Known tag id for `span` (pinned by spec).
pub const TAG_SPAN: u16 = 2;
/// Known tag id for `input` (pinned by spec).
pub const TAG_INPUT: u16 = 3;
/// Known tag id for `button` (pinned by spec).
pub const TAG_BUTTON: u16 = 4;

/// All known tag names; index + 1 is the wire id. Append-only — order is ABI.
pub static TAG_NAMES: &[&str] = &[
    // Pinned ids 1-4.
    "div", "span", "input", "button",
    // Document metadata & sectioning.
    "html", "head", "body", "title", "base", "link", "meta", "style", "script",
    "noscript", "template", "slot", "main", "section", "nav", "article", "aside",
    "header", "footer", "address", "h1", "h2", "h3", "h4", "h5", "h6", "hgroup",
    // Grouping content.
    "p", "hr", "pre", "blockquote", "ol", "ul", "menu", "li", "dl", "dt", "dd",
    "figure", "figcaption", "search",
    // Text-level semantics.
    "a", "em", "strong", "small", "s", "cite", "q", "dfn", "abbr", "ruby", "rt",
    "rp", "data", "time", "code", "var", "samp", "kbd", "sub", "sup", "i", "b",
    "u", "mark", "bdi", "bdo", "br", "wbr",
    // Edits.
    "ins", "del",
    // Embedded content.
    "picture", "source", "img", "iframe", "embed", "object", "video", "audio",
    "track", "map", "area", "svg", "math", "canvas",
    // Tabular data.
    "table", "caption", "colgroup", "col", "tbody", "thead", "tfoot", "tr", "td",
    "th",
    // Forms.
    "form", "label", "select", "datalist", "optgroup", "option", "textarea",
    "output", "progress", "meter", "fieldset", "legend",
    // Interactive elements.
    "details", "summary", "dialog",
];

// ─── Known attribute IDs ───────────────────────────────────────────────────────

/// Known attribute id for `class` (pinned by spec).
pub const ATTR_CLASS: u16 = 1;
/// Known attribute id for `id` (pinned by spec).
pub const ATTR_ID: u16 = 2;
/// Known attribute id for `style` (pinned by spec).
pub const ATTR_STYLE: u16 = 3;
/// Known attribute id for `value` (pinned by spec).
pub const ATTR_VALUE: u16 = 4;

/// All known attribute names; index + 1 is the wire id. Append-only — order is ABI.
pub static ATTR_NAMES: &[&str] = &[
    // Pinned ids 1-4.
    "class", "id", "style", "value",
    // Global attributes.
    "title", "lang", "dir", "hidden", "tabindex", "accesskey", "draggable",
    "contenteditable", "spellcheck", "translate", "role", "slot", "part", "is",
    // Link / resource.
    "href", "src", "srcset", "sizes", "alt", "rel", "target", "download",
    "referrerpolicy", "crossorigin", "integrity", "loading", "media", "type",
    // Forms.
    "name", "placeholder", "disabled", "readonly", "required", "checked",
    "selected", "multiple", "min", "max", "step", "pattern", "minlength",
    "maxlength", "autocomplete", "autofocus", "for", "form", "action", "method",
    "enctype", "novalidate", "accept", "rows", "cols", "wrap", "list", "size",
    // Table.
    "colspan", "rowspan", "headers", "scope",
    // Media / embedded.
    "width", "height", "controls", "autoplay", "loop", "muted", "preload",
    "poster", "playsinline",
    // Meta.
    "charset", "content", "http-equiv",
    // Misc interactive.
    "open", "label", "datetime", "cite", "data",
];

/// Resolve a known name to its wire id within `table` (index + 1).
fn lookup_id(table: &[&str], name: &str) -> Option<u16> {
    table.iter().position(|n| *n == name).map(|i| {
        // Tables are far smaller than u16::MAX; this cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let id = (i + 1) as u16;
        id
    })
}

/// Resolve a wire id back to its known name within `table`.
fn lookup_name(table: &'static [&'static str], id: u16) -> Option<&'static str> {
    if id == 0 {
        return None;
    }
    table.get(id as usize - 1).copied()
}

// ─── HtmlTag / AttrName ────────────────────────────────────────────────────────

/// A tag name: a known `u16` id (3 bytes on the wire) or an owned name (custom
/// elements). Decision G12/G13/G14.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlTag {
    /// A known tag from [`TAG_NAMES`] (id = index + 1).
    Id(u16),
    /// Any other tag (custom elements, future HTML).
    Name(Cow<'static, str>),
}

/// An attribute / property / event name: known `u16` id or owned name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrName {
    /// A known attribute from [`ATTR_NAMES`] (id = index + 1).
    Id(u16),
    /// Any other attribute (data-*, custom).
    Name(Cow<'static, str>),
}

macro_rules! impl_wire_name {
    ($ty:ident, $table:ident) => {
        impl $ty {
            /// Build from a name, preferring the known-id form.
            #[must_use]
            pub fn from_name(name: &str) -> Self {
                match lookup_id($table, name) {
                    Some(id) => Self::Id(id),
                    None => Self::Name(Cow::Owned(name.to_string())),
                }
            }

            /// Build from a `'static` name, preferring the known-id form (no
            /// allocation either way).
            #[must_use]
            pub fn from_static(name: &'static str) -> Self {
                match lookup_id($table, name) {
                    Some(id) => Self::Id(id),
                    None => Self::Name(Cow::Borrowed(name)),
                }
            }

            /// The resolved name, if known. `Id` values outside the table return
            /// `None` (a wire-corruption signal callers can surface).
            #[must_use]
            pub fn name(&self) -> Option<&str> {
                match self {
                    Self::Id(id) => lookup_name($table, *id),
                    Self::Name(name) => Some(name.as_ref()),
                }
            }

            /// The decision-010 string-column form: known ids as `"id:<n>"`,
            /// unknown names raw. The `id:` prefix is what the JS parser keys on.
            #[must_use]
            pub fn to_wire_string(&self) -> String {
                match self {
                    Self::Id(id) => alloc::format!("id:{id}"),
                    Self::Name(name) => name.to_string(),
                }
            }

            /// Parse the string-column form back: `"id:<n>"` becomes `Id`,
            /// anything else an owned `Name`.
            #[must_use]
            pub fn from_wire_str(s: &str) -> Self {
                if let Some(rest) = s.strip_prefix("id:") {
                    if let Ok(id) = rest.parse::<u16>() {
                        return Self::Id(id);
                    }
                }
                Self::Name(Cow::Owned(s.to_string()))
            }
        }

        impl core::fmt::Display for $ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.name() {
                    Some(name) => f.write_str(name),
                    None => write!(f, "id:?"),
                }
            }
        }

        impl From<&'static str> for $ty {
            fn from(name: &'static str) -> Self {
                Self::from_static(name)
            }
        }

        impl From<String> for $ty {
            fn from(name: String) -> Self {
                match lookup_id($table, &name) {
                    Some(id) => Self::Id(id),
                    None => Self::Name(Cow::Owned(name)),
                }
            }
        }
    };
}

impl_wire_name!(HtmlTag, TAG_NAMES);
impl_wire_name!(AttrName, ATTR_NAMES);

// ─── Html ──────────────────────────────────────────────────────────────────────

/// Lightweight DOM tree generated by the `html!` macro.
///
/// Not the arena-allocated `Fragment` from `foundation_html` — a simpler tree for
/// client-side use. `tag == None` makes a text node (when `text` is set) or a
/// tagless grouping node (when only `children` are set).
#[derive(Clone, Debug, PartialEq)]
pub struct Html {
    /// Element tag; `None` for text / grouping nodes.
    pub tag: Option<HtmlTag>,
    /// Attribute name/value pairs.
    pub attributes: Vec<(AttrName, Cow<'static, str>)>,
    /// Child nodes.
    pub children: Vec<Html>,
    /// Text content for text nodes.
    pub text: Option<Cow<'static, str>>,
    /// Reactive binding descriptors emitted by the `html!` macro.
    pub parts: Vec<Part>,
    /// The root's REGISTERED wire id when this tree was mounted by the
    /// reactive `html!` form (spec-42 feature 00) — how `mount_fragment`
    /// recognizes an already-mounted fragment and splices it by reference
    /// (`AppendChild`) instead of rebuilding it. Pure trees carry `None`.
    pub runtime_id: Option<u32>,
}

impl Html {
    /// An empty tagless node.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tag: None,
            attributes: Vec::new(),
            children: Vec::new(),
            text: None,
            parts: Vec::new(),
            runtime_id: None,
        }
    }

    /// A text node.
    #[must_use]
    pub fn text(content: impl Into<Cow<'static, str>>) -> Self {
        Self {
            text: Some(content.into()),
            ..Self::new()
        }
    }

    /// Returns true if this is a text node (no tag, has text content).
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.tag.is_none() && self.text.is_some()
    }

    /// Returns true if this is an element node (has a tag).
    #[must_use]
    pub fn is_element(&self) -> bool {
        self.tag.is_some()
    }
}

impl Default for Html {
    fn default() -> Self {
        Self::new()
    }
}

// ─── IntoHtml ──────────────────────────────────────────────────────────────────

/// Converts a value into an [`Html`] fragment.
///
/// Implemented for all common types (decision 007) so `{expr}` in `html!` works
/// without manual conversion.
pub trait IntoHtml {
    fn into_html(self) -> Html;
}

impl IntoHtml for Html {
    fn into_html(self) -> Html {
        self
    }
}

impl IntoHtml for Vec<Html> {
    fn into_html(self) -> Html {
        Html {
            children: self,
            ..Html::new()
        }
    }
}

impl IntoHtml for Option<Html> {
    fn into_html(self) -> Html {
        self.unwrap_or_default()
    }
}

impl IntoHtml for &str {
    fn into_html(self) -> Html {
        Html::text(self.to_string())
    }
}

impl IntoHtml for String {
    fn into_html(self) -> Html {
        Html::text(self)
    }
}

impl IntoHtml for &Html {
    fn into_html(self) -> Html {
        self.clone()
    }
}

// Primitives render through Display into a text node.
macro_rules! impl_into_html_for_display {
    ($($ty:ty),*) => {
        $(
            impl IntoHtml for $ty {
                fn into_html(self) -> Html {
                    Html::text(self.to_string())
                }
            }
        )*
    };
}

impl_into_html_for_display!(usize, isize, u8, u16, u32, u64, i8, i16, i32, i64, bool, f32, f64);
