/// Module implements custom html parser with reasonable speed for parsing html pages, data, templates and blocks.
use core::str::Chars;
use std::{cell, rc};

use anyhow::anyhow;
use ewe_mem::accumulator::Accumulator;
use lazy_regex::{lazy_regex, regex, regex_captures, Lazy, Regex, RegexBuilder};
use lazy_static::lazy_static;
use phf::phf_map;
use std::{any, collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing;

use crate::markup::ElementResult;

lazy_static! {
    static ref ATTRIBUTE_PAIRS: HashMap<&'static str, &'static str> = vec![
        ("{", "}"),
        ("(", ")"),
        ("[", "]"),
        ("'", "'"),
        ("\"", "\""),
        ("{{", "}}"),
        ("{{{", "}}}")
    ]
    .into_iter()
    .collect();
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParsingTagError {
    #[error("unknown tag can't parse")]
    UnknownTag,

    #[error("failed parsing text tag")]
    FailedParsing,

    #[error("Parser cound not parse content, stopped at: {0}")]
    FailedContentParsing(String),

    #[error("we expect after tag names will be a space")]
    ExpectingSpaceAfterTagName,

    #[error("Parser expected only one root element left after parsing but found more than 1")]
    UnexpectedParsingWithUnfinishedTag,

    #[error("Parser encountered tag with unexpected ending, please check your markup: {0}")]
    TagWithUnexpectedEnding(String),

    #[error("Parser encountered closing tag but never pre-allocated the start tag in stack")]
    ClosingTagHasZeroElementInStack,

    #[error("Parser encountered markup at top of stack has no markup, our invariants are broken")]
    StackedMarkupHasNoTag,

    #[error("Parser encountered a closing tag different from top markup in stack nor does not close top markup in rules: {0}")]
    ClosingTagDoesNotMatchTopMarkup(String),

    #[error("Parser failed to pop top markup in stack into children list of lower markup")]
    FailedToMoveTopMarkupIntoParentInStack,

    #[error("Attribute value starting with invalid token: {0}")]
    AttributeValueNotValidStarter(String),

    #[error("Attribute value should generally end with a space after declaration: {0}")]
    AttributeValueNotValidEnding(String),

    #[error("Invalid HTML content seen, probably not ending proper markup, cant parse: {0}")]
    InvalidHTMLContent(String),

    #[error("Invalid HTML content ending incorrectly and cant be parsed: {0}")]
    InvalidHTMLEnd(String),

    #[error("invalid state with last child Option being None, not expected")]
    LastChildWasEmptyShell,

    #[error("still expected stack to still contain a previous elem for operation")]
    ExpectedUnemptyStack,
}

pub type ParsingResult<T> = std::result::Result<T, ParsingTagError>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum SVGTags {
    A,
    Altglyph,
    Altglyphdef,
    Altglyphitem,
    Animate,
    Animatecolor,
    Animatemotion,
    Animatetransform,
    Circle,
    Clippath,
    Cursor,
    Defs,
    Desc,
    Discard,
    Ellipse,
    Feblend,
    Fecolormatrix,
    Fecomponenttransfer,
    Fecomposite,
    Feconvolvematrix,
    Fediffuselighting,
    Fedisplacementmap,
    Fedistantlight,
    Fedropshadow,
    Feflood,
    Fefunca,
    Fefuncb,
    Fefuncg,
    Fefuncr,
    Fegaussianblur,
    Feimage,
    Femerge,
    Femergenode,
    Femorphology,
    Feoffset,
    Fepointlight,
    Fespecularlighting,
    Fespotlight,
    Fetile,
    Feturbulence,
    Filter,
    Font,
    Foreignobject,
    G,
    Glyph,
    Glyphref,
    Hatch,
    Hatchpath,
    Hkern,
    Image,
    Line,
    Lineargradient,
    Marker,
    Mask,
    Mesh,
    Meshgradient,
    Meshpatch,
    Meshrow,
    Metadata,
    Mpath,
    Path,
    Tiled,
    Pattern,
    Polygon,
    Polyline,
    Radialgradient,
    Rect,
    Script,
    Set,
    Solidcolor,
    Stop,
    Style,
    Svg,
    Switch,
    Symbol,
    Text,
    Textpath,
    Title,
    Tref,
    Tspan,
    Unknown,
    Use,
    View,
    Vkern,
}

impl FromStr for SVGTags {
    type Err = ParsingTagError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_lowercase().as_str() {
            "a" => Ok(SVGTags::A),
            "altglyph" => Ok(SVGTags::Altglyph),
            "altglyphdef" => Ok(SVGTags::Altglyphdef),
            "altglyphitem" => Ok(SVGTags::Altglyphitem),
            "animate" => Ok(SVGTags::Animate),
            "animatecolor" => Ok(SVGTags::Animatecolor),
            "animatemotion" => Ok(SVGTags::Animatemotion),
            "animatetransform" => Ok(SVGTags::Animatetransform),
            "circle" => Ok(SVGTags::Circle),
            "clippath" => Ok(SVGTags::Clippath),
            "cursor" => Ok(SVGTags::Cursor),
            "defs" => Ok(SVGTags::Defs),
            "desc" => Ok(SVGTags::Desc),
            "discard" => Ok(SVGTags::Discard),
            "ellipse" => Ok(SVGTags::Ellipse),
            "feblend" => Ok(SVGTags::Feblend),
            "fecolormatrix" => Ok(SVGTags::Fecolormatrix),
            "fecomponenttransfer" => Ok(SVGTags::Fecomponenttransfer),
            "fecomposite" => Ok(SVGTags::Fecomposite),
            "feconvolvematrix" => Ok(SVGTags::Feconvolvematrix),
            "fediffuselighting" => Ok(SVGTags::Fediffuselighting),
            "fedisplacementmap" => Ok(SVGTags::Fedisplacementmap),
            "fedistantlight" => Ok(SVGTags::Fedistantlight),
            "fedropshadow" => Ok(SVGTags::Fedropshadow),
            "feflood" => Ok(SVGTags::Feflood),
            "fefunca" => Ok(SVGTags::Fefunca),
            "fefuncb" => Ok(SVGTags::Fefuncb),
            "fefuncg" => Ok(SVGTags::Fefuncg),
            "fefuncr" => Ok(SVGTags::Fefuncr),
            "fegaussianblur" => Ok(SVGTags::Fegaussianblur),
            "feimage" => Ok(SVGTags::Feimage),
            "femerge" => Ok(SVGTags::Femerge),
            "femergenode" => Ok(SVGTags::Femergenode),
            "femorphology" => Ok(SVGTags::Femorphology),
            "feoffset" => Ok(SVGTags::Feoffset),
            "fepointlight" => Ok(SVGTags::Fepointlight),
            "fespecularlighting" => Ok(SVGTags::Fespecularlighting),
            "fespotlight" => Ok(SVGTags::Fespotlight),
            "fetile" => Ok(SVGTags::Fetile),
            "feturbulence" => Ok(SVGTags::Feturbulence),
            "filter" => Ok(SVGTags::Filter),
            "font" => Ok(SVGTags::Font),
            "foreignobject" => Ok(SVGTags::Foreignobject),
            "g" => Ok(SVGTags::G),
            "glyph" => Ok(SVGTags::Glyph),
            "glyphref" => Ok(SVGTags::Glyphref),
            "hatch" => Ok(SVGTags::Hatch),
            "hatchpath" => Ok(SVGTags::Hatchpath),
            "hkern" => Ok(SVGTags::Hkern),
            "image" => Ok(SVGTags::Image),
            "line" => Ok(SVGTags::Line),
            "lineargradient" => Ok(SVGTags::Lineargradient),
            "marker" => Ok(SVGTags::Marker),
            "mask" => Ok(SVGTags::Mask),
            "mesh" => Ok(SVGTags::Mesh),
            "meshgradient" => Ok(SVGTags::Meshgradient),
            "meshpatch" => Ok(SVGTags::Meshpatch),
            "meshrow" => Ok(SVGTags::Meshrow),
            "metadata" => Ok(SVGTags::Metadata),
            "mpath" => Ok(SVGTags::Mpath),
            "path" => Ok(SVGTags::Path),
            "tiled" => Ok(SVGTags::Tiled),
            "pattern" => Ok(SVGTags::Pattern),
            "polygon" => Ok(SVGTags::Polygon),
            "polyline" => Ok(SVGTags::Polyline),
            "radialgradient" => Ok(SVGTags::Radialgradient),
            "rect" => Ok(SVGTags::Rect),
            "script" => Ok(SVGTags::Script),
            "set" => Ok(SVGTags::Set),
            "solidcolor" => Ok(SVGTags::Solidcolor),
            "stop" => Ok(SVGTags::Stop),
            "style" => Ok(SVGTags::Style),
            "svg" => Ok(SVGTags::Svg),
            "switch" => Ok(SVGTags::Switch),
            "symbol" => Ok(SVGTags::Symbol),
            "text" => Ok(SVGTags::Text),
            "textpath" => Ok(SVGTags::Textpath),
            "title" => Ok(SVGTags::Title),
            "tref" => Ok(SVGTags::Tref),
            "tspan" => Ok(SVGTags::Tspan),
            "unknown" => Ok(SVGTags::Unknown),
            "use" => Ok(SVGTags::Use),
            "view" => Ok(SVGTags::View),
            "vkern" => Ok(SVGTags::Vkern),
            _ => Err(ParsingTagError::UnknownTag),
        }
    }
}

impl Into<String> for SVGTags {
    fn into(self) -> String {
        String::from(self.tag_to_str())
    }
}

impl<'b> Into<&'b str> for SVGTags {
    fn into(self) -> &'b str {
        self.tag_to_str()
    }
}

impl SVGTags {
    pub fn is_svg_element_closed_by_closing_tag(me: SVGTags, other: SVGTags) -> bool {
        match me {
            _ => false,
        }
    }

    pub fn is_svg_element_closed_by_opening_tag(me: SVGTags, other: SVGTags) -> bool {
        match me {
            SVGTags::Animate => match other {
                SVGTags::Animate => true,
                _ => true,
            },
            _ => false,
        }
    }

    pub fn is_self_closing_tag(self) -> bool {
        match self {
            SVGTags::Path
            | SVGTags::Polygon
            | SVGTags::Rect
            | SVGTags::Circle
            | SVGTags::Animate
            | SVGTags::Animatemotion
            | SVGTags::Animatetransform => true,
            _ => false,
        }
    }

    pub fn is_auto_closed(tag: SVGTags) -> bool {
        match tag {
            // SVGTags::Path
            // | SVGTags::Polygon
            // | SVGTags::Rect
            // | SVGTags::Circle
            // | SVGTags::Animate => true,
            _ => false,
        }
    }

    fn tag_to_str<'b>(self) -> &'b str {
        match self {
            SVGTags::A => "a",
            SVGTags::Altglyph => "altGlyph",
            SVGTags::Altglyphdef => "altGlyphDef",
            SVGTags::Altglyphitem => "altGlyphItem",
            SVGTags::Animate => "animate",
            SVGTags::Animatecolor => "animateColor",
            SVGTags::Animatemotion => "animateMotion",
            SVGTags::Animatetransform => "animateTransform",
            SVGTags::Circle => "circle",
            SVGTags::Clippath => "clipPath",
            SVGTags::Cursor => "cursor",
            SVGTags::Defs => "defs",
            SVGTags::Desc => "desc",
            SVGTags::Discard => "discard",
            SVGTags::Ellipse => "ellipse",
            SVGTags::Feblend => "feBlend",
            SVGTags::Fecolormatrix => "feColorMatrix",
            SVGTags::Fecomponenttransfer => "feComponentTransfer",
            SVGTags::Fecomposite => "feComposite",
            SVGTags::Feconvolvematrix => "feConvolveMatrix",
            SVGTags::Fediffuselighting => "feDiffuseLighting",
            SVGTags::Fedisplacementmap => "feDisplacementMap",
            SVGTags::Fedistantlight => "feDistantLight",
            SVGTags::Fedropshadow => "feDropShadow",
            SVGTags::Feflood => "feFlood",
            SVGTags::Fefunca => "feFuncA",
            SVGTags::Fefuncb => "feFuncB",
            SVGTags::Fefuncg => "feFuncG",
            SVGTags::Fefuncr => "feFuncR",
            SVGTags::Fegaussianblur => "feGaussianBlur",
            SVGTags::Feimage => "feImage",
            SVGTags::Femerge => "feMerge",
            SVGTags::Femergenode => "feMergeNode",
            SVGTags::Femorphology => "feMorphology",
            SVGTags::Feoffset => "feOffset",
            SVGTags::Fepointlight => "fePointLight",
            SVGTags::Fespecularlighting => "feSpecularLighting",
            SVGTags::Fespotlight => "feSpotLight",
            SVGTags::Fetile => "feTile",
            SVGTags::Feturbulence => "feTurbulence",
            SVGTags::Filter => "filter",
            SVGTags::Font => "font",
            SVGTags::Foreignobject => "foreignObject",
            SVGTags::G => "g",
            SVGTags::Glyph => "glyph",
            SVGTags::Glyphref => "glyphRef",
            SVGTags::Hatch => "hatch",
            SVGTags::Hatchpath => "hatchpath",
            SVGTags::Hkern => "hkern",
            SVGTags::Image => "image",
            SVGTags::Line => "line",
            SVGTags::Lineargradient => "linearGradient",
            SVGTags::Marker => "marker",
            SVGTags::Mask => "mask",
            SVGTags::Mesh => "mesh",
            SVGTags::Meshgradient => "meshgradient",
            SVGTags::Meshpatch => "meshpatch",
            SVGTags::Meshrow => "meshrow",
            SVGTags::Metadata => "metadata",
            SVGTags::Mpath => "mpath",
            SVGTags::Path => "path",
            SVGTags::Tiled => "tiled",
            SVGTags::Pattern => "pattern",
            SVGTags::Polygon => "polygon",
            SVGTags::Polyline => "polyline",
            SVGTags::Radialgradient => "radialGradient",
            SVGTags::Rect => "rect",
            SVGTags::Script => "script",
            SVGTags::Set => "set",
            SVGTags::Solidcolor => "solidcolor",
            SVGTags::Stop => "stop",
            SVGTags::Style => "style",
            SVGTags::Svg => "svg",
            SVGTags::Switch => "switch",
            SVGTags::Symbol => "symbol",
            SVGTags::Text => "text",
            SVGTags::Textpath => "textPath",
            SVGTags::Title => "title",
            SVGTags::Tref => "tref",
            SVGTags::Tspan => "tspan",
            SVGTags::Unknown => "unknown",
            SVGTags::Use => "use",
            SVGTags::View => "view",
            SVGTags::Vkern => "vkern",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum HTMLTags {
    A,
    Abbr,
    Address,
    Area,
    Article,
    Aside,
    Audio,
    B,
    Base,
    Bdi,
    Bdo,
    Blockquote,
    Br,
    Button,
    Canvas,
    Caption,
    Cite,
    Code,
    Col,
    Colgroup,
    DocumentFragmentContainer,
    DocumentFragment,
    Data,
    Datalist,
    Dd,
    Del,
    Details,
    Dfn,
    Dialog,
    Div,
    Dl,
    Dt,
    Em,
    Embed,
    Fieldset,
    Figcaption,
    Figure,
    Footer,
    Form,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    Head,
    Header,
    Hgroup,
    Html,
    Hr,
    I,
    Iframe,
    Img,
    Input,
    Ins,
    Kbd,
    Label,
    Legend,
    Li,
    Link,
    Main,
    Map,
    Mark,
    Menu,
    Menuitem,
    Meta,
    Meter,
    Nav,
    Noframes,
    Noscript,
    Object,
    Ol,
    Optgroup,
    Option,
    Output,
    P,
    Param,
    Picture,
    Monospace,
    Pre,
    Progress,
    Q,
    Rp,
    Rt,
    Rtc,
    Ruby,
    S,
    Samp,
    Script,
    Section,
    Select,
    Slot,
    Small,
    Source,
    Span,
    Strong,
    Style,
    Sub,
    Summary,
    Sup,
    Table,
    Tbody,
    Td,
    Template,
    Textarea,
    Tfoot,
    Th,
    Thead,
    Time,
    Title,
    Tr,
    Track,
    U,
    Ul,
    Var,
    Video,
    Wbr,
    Xml,
    Keygen,
}

impl FromStr for HTMLTags {
    type Err = ParsingTagError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_lowercase().as_str() {
            "a" => Ok(HTMLTags::A),
            "abbr" => Ok(HTMLTags::Abbr),
            "address" => Ok(HTMLTags::Address),
            "area" => Ok(HTMLTags::Area),
            "article" => Ok(HTMLTags::Article),
            "aside" => Ok(HTMLTags::Aside),
            "audio" => Ok(HTMLTags::Audio),
            "documentfragmentcontainer" => Ok(HTMLTags::DocumentFragmentContainer),
            "documentfragment" => Ok(HTMLTags::DocumentFragment),
            "b" => Ok(HTMLTags::B),
            "base" => Ok(HTMLTags::Base),
            "bdi" => Ok(HTMLTags::Bdi),
            "bdo" => Ok(HTMLTags::Bdo),
            "blockquote" => Ok(HTMLTags::Blockquote),
            "br" => Ok(HTMLTags::Br),
            "button" => Ok(HTMLTags::Button),
            "canvas" => Ok(HTMLTags::Canvas),
            "caption" => Ok(HTMLTags::Caption),
            "cite" => Ok(HTMLTags::Cite),
            "code" => Ok(HTMLTags::Code),
            "col" => Ok(HTMLTags::Col),
            "colgroup" => Ok(HTMLTags::Colgroup),
            "data" => Ok(HTMLTags::Data),
            "datalist" => Ok(HTMLTags::Datalist),
            "dd" => Ok(HTMLTags::Dd),
            "del" => Ok(HTMLTags::Del),
            "details" => Ok(HTMLTags::Details),
            "dfn" => Ok(HTMLTags::Dfn),
            "dialog" => Ok(HTMLTags::Dialog),
            "div" => Ok(HTMLTags::Div),
            "dl" => Ok(HTMLTags::Dl),
            "dt" => Ok(HTMLTags::Dt),
            "em" => Ok(HTMLTags::Em),
            "embed" => Ok(HTMLTags::Embed),
            "fieldset" => Ok(HTMLTags::Fieldset),
            "figcaption" => Ok(HTMLTags::Figcaption),
            "figure" => Ok(HTMLTags::Figure),
            "footer" => Ok(HTMLTags::Footer),
            "form" => Ok(HTMLTags::Form),
            "h1" => Ok(HTMLTags::H1),
            "h2" => Ok(HTMLTags::H2),
            "h3" => Ok(HTMLTags::H3),
            "h4" => Ok(HTMLTags::H4),
            "h5" => Ok(HTMLTags::H5),
            "h6" => Ok(HTMLTags::H6),
            "head" => Ok(HTMLTags::Head),
            "html" => Ok(HTMLTags::Html),
            "header" => Ok(HTMLTags::Header),
            "hgroup" => Ok(HTMLTags::Hgroup),
            "hr" => Ok(HTMLTags::Hr),
            "i" => Ok(HTMLTags::I),
            "iframe" => Ok(HTMLTags::Iframe),
            "img" => Ok(HTMLTags::Img),
            "input" => Ok(HTMLTags::Input),
            "ins" => Ok(HTMLTags::Ins),
            "kbd" => Ok(HTMLTags::Kbd),
            "keygen" => Ok(HTMLTags::Kbd),
            "label" => Ok(HTMLTags::Label),
            "legend" => Ok(HTMLTags::Legend),
            "li" => Ok(HTMLTags::Li),
            "link" => Ok(HTMLTags::Link),
            "main" => Ok(HTMLTags::Main),
            "map" => Ok(HTMLTags::Map),
            "mark" => Ok(HTMLTags::Mark),
            "menu" => Ok(HTMLTags::Menu),
            "menuitem" => Ok(HTMLTags::Menuitem),
            "meta" => Ok(HTMLTags::Meta),
            "meter" => Ok(HTMLTags::Meter),
            "nav" => Ok(HTMLTags::Nav),
            "noframes" => Ok(HTMLTags::Noframes),
            "noscript" => Ok(HTMLTags::Noscript),
            "object" => Ok(HTMLTags::Object),
            "ol" => Ok(HTMLTags::Ol),
            "optgroup" => Ok(HTMLTags::Optgroup),
            "option" => Ok(HTMLTags::Option),
            "output" => Ok(HTMLTags::Output),
            "p" => Ok(HTMLTags::P),
            "param" => Ok(HTMLTags::Param),
            "picture" => Ok(HTMLTags::Picture),
            "monospace" => Ok(HTMLTags::Monospace),
            "pre" => Ok(HTMLTags::Pre),
            "progress" => Ok(HTMLTags::Progress),
            "q" => Ok(HTMLTags::Q),
            "rp" => Ok(HTMLTags::Rp),
            "rt" => Ok(HTMLTags::Rt),
            "rtc" => Ok(HTMLTags::Rtc),
            "ruby" => Ok(HTMLTags::Ruby),
            "s" => Ok(HTMLTags::S),
            "samp" => Ok(HTMLTags::Samp),
            "script" => Ok(HTMLTags::Script),
            "section" => Ok(HTMLTags::Section),
            "select" => Ok(HTMLTags::Select),
            "slot" => Ok(HTMLTags::Slot),
            "small" => Ok(HTMLTags::Small),
            "source" => Ok(HTMLTags::Source),
            "span" => Ok(HTMLTags::Span),
            "strong" => Ok(HTMLTags::Strong),
            "style" => Ok(HTMLTags::Style),
            "sub" => Ok(HTMLTags::Sub),
            "summary" => Ok(HTMLTags::Summary),
            "sup" => Ok(HTMLTags::Sup),
            "table" => Ok(HTMLTags::Table),
            "tbody" => Ok(HTMLTags::Tbody),
            "td" => Ok(HTMLTags::Td),
            "template" => Ok(HTMLTags::Template),
            "textarea" => Ok(HTMLTags::Textarea),
            "tfoot" => Ok(HTMLTags::Tfoot),
            "th" => Ok(HTMLTags::Th),
            "thead" => Ok(HTMLTags::Thead),
            "time" => Ok(HTMLTags::Time),
            "title" => Ok(HTMLTags::Title),
            "tr" => Ok(HTMLTags::Tr),
            "track" => Ok(HTMLTags::Track),
            "u" => Ok(HTMLTags::U),
            "ul" => Ok(HTMLTags::Ul),
            "var" => Ok(HTMLTags::Var),
            "video" => Ok(HTMLTags::Video),
            "wbr" => Ok(HTMLTags::Wbr),
            "xml" => Ok(HTMLTags::Xml),
            _ => Err(ParsingTagError::UnknownTag),
        }
    }
}

impl HTMLTags {
    fn tag_to_str<'b>(self) -> &'b str {
        match self {
            HTMLTags::A => "a",
            HTMLTags::Abbr => "abbr",
            HTMLTags::Address => "address",
            HTMLTags::Area => "area",
            HTMLTags::Article => "article",
            HTMLTags::Aside => "aside",
            HTMLTags::Audio => "audio",
            HTMLTags::B => "b",
            HTMLTags::Base => "base",
            HTMLTags::Bdi => "bdi",
            HTMLTags::Bdo => "bdo",
            HTMLTags::Blockquote => "blockquote",
            HTMLTags::Br => "br",
            HTMLTags::Button => "button",
            HTMLTags::Canvas => "canvas",
            HTMLTags::Caption => "caption",
            HTMLTags::Cite => "cite",
            HTMLTags::Code => "code",
            HTMLTags::Col => "col",
            HTMLTags::Colgroup => "colgroup",
            HTMLTags::Data => "data",
            HTMLTags::Datalist => "datalist",
            HTMLTags::Dd => "dd",
            HTMLTags::Del => "del",
            HTMLTags::Details => "details",
            HTMLTags::Dfn => "dfn",
            HTMLTags::Dialog => "dialog",
            HTMLTags::DocumentFragment => "documentfragment",
            HTMLTags::DocumentFragmentContainer => "documentfragmentcontainer",
            HTMLTags::Div => "div",
            HTMLTags::Dl => "dl",
            HTMLTags::Dt => "dt",
            HTMLTags::Em => "em",
            HTMLTags::Embed => "embed",
            HTMLTags::Fieldset => "fieldset",
            HTMLTags::Figcaption => "figcaption",
            HTMLTags::Figure => "figure",
            HTMLTags::Footer => "footer",
            HTMLTags::Form => "form",
            HTMLTags::H1 => "h1",
            HTMLTags::H2 => "h2",
            HTMLTags::H3 => "h3",
            HTMLTags::H4 => "h4",
            HTMLTags::H5 => "h5",
            HTMLTags::H6 => "h6",
            HTMLTags::Head => "head",
            HTMLTags::Html => "html",
            HTMLTags::Header => "header",
            HTMLTags::Hgroup => "hgroup",
            HTMLTags::Hr => "hr",
            HTMLTags::I => "i",
            HTMLTags::Iframe => "iframe",
            HTMLTags::Img => "img",
            HTMLTags::Input => "input",
            HTMLTags::Ins => "ins",
            HTMLTags::Kbd => "kbd",
            HTMLTags::Keygen => "keygen",
            HTMLTags::Label => "label",
            HTMLTags::Legend => "legend",
            HTMLTags::Li => "li",
            HTMLTags::Link => "link",
            HTMLTags::Main => "main",
            HTMLTags::Map => "map",
            HTMLTags::Mark => "mark",
            HTMLTags::Menu => "menu",
            HTMLTags::Menuitem => "menuitem",
            HTMLTags::Meta => "meta",
            HTMLTags::Meter => "meter",
            HTMLTags::Nav => "nav",
            HTMLTags::Noframes => "noframes",
            HTMLTags::Noscript => "noscript",
            HTMLTags::Object => "object",
            HTMLTags::Ol => "ol",
            HTMLTags::Optgroup => "optgroup",
            HTMLTags::Option => "option",
            HTMLTags::Output => "output",
            HTMLTags::P => "p",
            HTMLTags::Param => "param",
            HTMLTags::Picture => "picture",
            HTMLTags::Monospace => "monospace",
            HTMLTags::Pre => "pre",
            HTMLTags::Progress => "progress",
            HTMLTags::Q => "q",
            HTMLTags::Rp => "rp",
            HTMLTags::Rt => "rt",
            HTMLTags::Rtc => "rtc",
            HTMLTags::Ruby => "ruby",
            HTMLTags::S => "s",
            HTMLTags::Samp => "samp",
            HTMLTags::Script => "script",
            HTMLTags::Section => "section",
            HTMLTags::Select => "select",
            HTMLTags::Slot => "slot",
            HTMLTags::Small => "small",
            HTMLTags::Source => "source",
            HTMLTags::Span => "span",
            HTMLTags::Strong => "strong",
            HTMLTags::Style => "style",
            HTMLTags::Sub => "sub",
            HTMLTags::Summary => "summary",
            HTMLTags::Sup => "sup",
            HTMLTags::Table => "table",
            HTMLTags::Tbody => "tbody",
            HTMLTags::Td => "td",
            HTMLTags::Template => "template",
            HTMLTags::Textarea => "textarea",
            HTMLTags::Tfoot => "tfoot",
            HTMLTags::Th => "th",
            HTMLTags::Thead => "thead",
            HTMLTags::Time => "time",
            HTMLTags::Title => "title",
            HTMLTags::Tr => "tr",
            HTMLTags::Track => "track",
            HTMLTags::U => "u",
            HTMLTags::Ul => "ul",
            HTMLTags::Var => "var",
            HTMLTags::Video => "video",
            HTMLTags::Wbr => "wbr",
            HTMLTags::Xml => "xml",
        }
    }

    pub fn is_self_closing_tag(self) -> bool {
        match self {
            HTMLTags::Area
            | HTMLTags::Base
            | HTMLTags::Embed
            | HTMLTags::Hr
            | HTMLTags::Img
            | HTMLTags::Link
            | HTMLTags::Meta
            | HTMLTags::Keygen
            | HTMLTags::Param
            | HTMLTags::Track
            | HTMLTags::Source
            | HTMLTags::Input
            | HTMLTags::Col
            | HTMLTags::Wbr
            | HTMLTags::Br => true,
            _ => false,
        }
    }

    pub fn is_html_element_closed_by_closing_tag(me: HTMLTags, other: HTMLTags) -> bool {
        match me {
            HTMLTags::Li => match other {
                HTMLTags::Li | HTMLTags::Ol | HTMLTags::Ul => true,
                _ => false,
            },
            HTMLTags::A => match other {
                HTMLTags::Div => true,
                _ => false,
            },
            HTMLTags::B => match other {
                HTMLTags::Div => true,
                _ => false,
            },
            HTMLTags::I => match other {
                HTMLTags::Div => true,
                _ => false,
            },
            HTMLTags::P => match other {
                HTMLTags::Div => true,
                _ => false,
            },
            HTMLTags::Td => match other {
                HTMLTags::Table | HTMLTags::Tr => true,
                _ => false,
            },
            HTMLTags::Th => match other {
                HTMLTags::Table | HTMLTags::Tr => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_html_element_closed_by_opening_tag(me: HTMLTags, other: HTMLTags) -> bool {
        match me {
            HTMLTags::Meta => match other {
                HTMLTags::Meta => true,
                _ => true,
            },
            HTMLTags::Li => match other {
                HTMLTags::Li => true,
                _ => false,
            },
            HTMLTags::P => match other {
                HTMLTags::Div | HTMLTags::P => true,
                _ => false,
            },
            HTMLTags::B => match other {
                HTMLTags::Div => true,
                _ => false,
            },
            HTMLTags::Td => match other {
                HTMLTags::Td | HTMLTags::Th => true,
                _ => false,
            },
            HTMLTags::Th => match other {
                HTMLTags::Td | HTMLTags::Th => true,
                _ => false,
            },
            HTMLTags::H1 => match other {
                HTMLTags::H1 => true,
                _ => false,
            },
            HTMLTags::H2 => match other {
                HTMLTags::H2 => true,
                _ => false,
            },
            HTMLTags::H3 => match other {
                HTMLTags::H3 => true,
                _ => false,
            },
            HTMLTags::H4 => match other {
                HTMLTags::H4 => true,
                _ => false,
            },
            HTMLTags::H5 => match other {
                HTMLTags::H5 => true,
                _ => false,
            },
            HTMLTags::H6 => match other {
                HTMLTags::H6 => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_auto_closed(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::Meta | HTMLTags::Link => true,
            _ => false,
        }
    }

    pub fn is_block_tag(tag: HTMLTags) -> bool {
        if HTMLTags::is_table_tag(tag.clone())
            || HTMLTags::is_d_tag(tag.clone())
            || HTMLTags::is_header_tag(tag.clone())
            || HTMLTags::is_f_tag(tag.clone())
        {
            return true;
        }
        match tag {
            HTMLTags::Address
            | HTMLTags::Article
            | HTMLTags::Aside
            | HTMLTags::Blockquote
            | HTMLTags::Br
            | HTMLTags::Main
            | HTMLTags::Nav
            | HTMLTags::P
            | HTMLTags::Pre
            | HTMLTags::Section
            | HTMLTags::Hr
            | HTMLTags::Ol
            | HTMLTags::Ul
            | HTMLTags::Li => true,
            _ => false,
        }
    }

    pub fn is_table_tag(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::Tfoot
            | HTMLTags::Tbody
            | HTMLTags::Thead
            | HTMLTags::Th
            | HTMLTags::Tr
            | HTMLTags::Td
            | HTMLTags::Table => true,
            _ => false,
        }
    }

    pub fn is_block_text_tag(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::Script | HTMLTags::Noscript | HTMLTags::Style | HTMLTags::Pre => true,
            _ => false,
        }
    }

    pub fn is_f_tag(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::Form
            | HTMLTags::Footer
            | HTMLTags::Figure
            | HTMLTags::Figcaption
            | HTMLTags::Fieldset => true,
            _ => false,
        }
    }

    pub fn is_d_tag(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::Details | HTMLTags::Dialog | HTMLTags::Dd | HTMLTags::Div | HTMLTags::Dt => {
                true
            }
            _ => false,
        }
    }

    pub fn is_header_tag(tag: HTMLTags) -> bool {
        match tag {
            HTMLTags::H1
            | HTMLTags::H2
            | HTMLTags::H3
            | HTMLTags::H4
            | HTMLTags::H5
            | HTMLTags::H6
            | HTMLTags::Header
            | HTMLTags::Hgroup => true,
            _ => false,
        }
    }
}

impl Into<String> for HTMLTags {
    fn into(self) -> String {
        String::from(self.tag_to_str())
    }
}

impl<'b> Into<&'b str> for HTMLTags {
    fn into(self) -> &'b str {
        self.tag_to_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum MarkupTags {
    DocType,
    SVG(SVGTags),
    HTML(HTMLTags),
    Text(String),
    Comment(String),
    Component(String),
    Code(String),
    Rust(String),
}

impl FromStr for MarkupTags {
    type Err = ParsingTagError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(tag) = HTMLTags::from_str(s) {
            return Ok(MarkupTags::HTML(tag));
        }
        if let Ok(tag) = SVGTags::from_str(s) {
            return Ok(MarkupTags::SVG(tag));
        }
        Ok(MarkupTags::Component(s.to_owned()))
    }
}

impl MarkupTags {
    pub fn is_element_closed_by_opening_tag(me: MarkupTags, other: MarkupTags) -> bool {
        match (me, other) {
            (MarkupTags::HTML(me), MarkupTags::HTML(other)) => {
                HTMLTags::is_html_element_closed_by_opening_tag(me, other)
            }
            (MarkupTags::SVG(me), MarkupTags::SVG(other)) => {
                SVGTags::is_svg_element_closed_by_opening_tag(me, other)
            }
            _ => false,
        }
    }

    pub fn is_element_closed_by_closing_tag(me: MarkupTags, other: MarkupTags) -> bool {
        match (me, other) {
            (MarkupTags::HTML(me), MarkupTags::HTML(other)) => {
                HTMLTags::is_html_element_closed_by_closing_tag(me, other)
            }
            (MarkupTags::SVG(me), MarkupTags::SVG(other)) => {
                SVGTags::is_svg_element_closed_by_closing_tag(me, other)
            }
            _ => false,
        }
    }

    pub fn is_element_self_closing(me: MarkupTags) -> bool {
        match me {
            MarkupTags::SVG(me) => SVGTags::is_self_closing_tag(me),
            MarkupTags::HTML(me) => HTMLTags::is_self_closing_tag(me),
            _ => false,
        }
    }

    pub fn is_element_auto_closed(me: MarkupTags) -> bool {
        match me {
            MarkupTags::SVG(me) => SVGTags::is_auto_closed(me),
            MarkupTags::HTML(me) => HTMLTags::is_auto_closed(me),
            _ => false,
        }
    }

    pub fn to_string<'a>(self) -> Result<String, anyhow::Error> {
        match self {
            MarkupTags::DocType => Ok(String::from("!Doctype")),
            MarkupTags::SVG(sg) => Ok(sg.into()),
            MarkupTags::HTML(ht) => Ok(ht.into()),
            MarkupTags::Comment(text)
            | MarkupTags::Code(text)
            | MarkupTags::Rust(text)
            | MarkupTags::Text(text)
            | MarkupTags::Component(text) => Ok(text.clone()),
        }
    }

    pub fn to_str<'a>(self) -> Result<&'a str, anyhow::Error> {
        match self {
            MarkupTags::DocType => Ok("!doctype"),
            MarkupTags::SVG(sg) => Ok(sg.into()),
            MarkupTags::HTML(ht) => Ok(ht.into()),
            _ => Err(anyhow!("Cant get &str representation of {:?}", self)),
        }
    }

    pub fn is_block_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_block_tag(t),
            _ => false,
        }
    }

    pub fn is_table_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_table_tag(t),
            _ => false,
        }
    }

    pub fn is_block_text_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_block_text_tag(t),
            _ => false,
        }
    }

    pub fn is_f_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_f_tag(t),
            _ => false,
        }
    }

    pub fn is_d_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_d_tag(t),
            _ => false,
        }
    }

    pub fn is_header_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => HTMLTags::is_header_tag(t),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum AttrValue<'a> {
    Text(&'a str),  // Every other type
    Block(&'a str), // any content within 1 block { }
    Code(&'a str),  // any content within 2 block {{ }}
    Rust(&'a str),  // any content within 3 block {{{ }}}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stack<'a> {
    tag: Option<MarkupTags>,
    closed: bool,
    children: Vec<Stack<'a>>,
    attrs: Vec<(&'a str, AttrValue<'a>)>,
    start_range: Option<usize>,
    end_range: Option<usize>,
}

impl<'a> Stack<'a> {
    pub fn new(tag: MarkupTags, closed: bool, start_range: usize, end_range: usize) -> Self {
        Self {
            closed,
            tag: Some(tag),
            attrs: vec![],
            children: vec![],
            end_range: Some(end_range),
            start_range: Some(start_range),
        }
    }

    pub fn add_attr(&mut self, name: &'a str, value: AttrValue<'a>) {
        self.attrs.push((name, value))
    }

    pub fn get_tags(&self) -> Vec<MarkupTags> {
        let mut items = vec![];
        self.add_tags_to(&mut items);
        for elem in self.children.iter() {
            items.extend(elem.get_tags());
        }
        return items;
    }

    pub fn add_tags_to(&self, items: &mut Vec<MarkupTags>) {
        items.push(self.tag.clone().unwrap());
    }

    pub fn empty() -> Self {
        Self {
            tag: None,
            closed: false,
            attrs: vec![],
            children: vec![],
            end_range: Some(0),
            start_range: Some(0),
        }
    }
}

impl<'a> Default for Stack<'a> {
    fn default() -> Self {
        Self {
            tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
            closed: true,
            attrs: vec![],
            children: vec![],
            start_range: None,
            end_range: None,
        }
    }
}

pub type Stacks<'a> = Vec<Stack<'a>>;

pub type CheckTag = fn(MarkupTags) -> bool;

pub enum ParserDirective<'a> {
    Void(Stack<'a>),
    Open(Stack<'a>),
    Closed((MarkupTags, (usize, usize))),
}

pub struct HTMLParser {
    allowed_tag_symbols: &'static [char],
    check_is_block_text_tag: CheckTag,
}

static TAG_OPEN_BRACKET_CHAR: char = '<';
static TAG_CLOSED_BRACKET_CHAR: char = '>';

static VALID_TAG_NAME_SYMBOLS: &[char] = &['@', '-', '_'];
static SPACE_CHARS: &[char] = &[' ', '\n', '\t', '\r'];
static DOC_TYPE_STARTER: &[char] = &['!'];
static QUOTATIONS: &[char] = &['\'', '"'];
static DASHES: &[char] = &['-', '_', '|'];
static VALID_ATTRIBUTE_NAMING_CHARS: &[char] = &['-', '_', ':'];
static VALID_ATTRIBUTE_VALUE_CHARS: &[char] = &['"', '\'', '{', '(', '['];
static VALID_ATTRIBUTE_STARTER_SYMBOLS: &[char] = &['{', '"'];
static VALID_ATTRIBUTE_ENDER_SYMBOLS: &[char] = &['}', '"'];

static VALID_ATTRIBUTE_STARTER_SYMBOLS_STR: &[&str] = &["{", "(", "[", "\"", "'"];

static CODE_STARTER_BLOCKS: &[&str] = &["{{", "{{{"];

static EMPTY_STRING: &'static str = "";
static FRAME_FLAG_TAG: &'static str = "DocumentFragmentContainer";
static TEXT_BLOCK_STARTER: &'static str = "{";
static CODE_BLOCK_STARTER: &'static str = "{{";
static RUST_BLOCK_STARTER: &'static str = "{{{";
static SINGLE_QUOTE_STR: &'static str = "'";
static DOUBLE_QUOTE_STR: &'static str = "\"";
static SPACE_STR: &'static str = " ";
static COMMENT_DASHER: &'static str = "-";
static COMMENT_STARTER: &'static str = "<!--";
static COMMENT_ENDER: &'static str = "-->";
static QUESTION_MARK: &'static str = "?";
static XML_STARTER: &'static str = "<?";
static XML_ENDER: &'static str = "?>";
static DOCTYPE_STR: &'static str = "!doctype";
static TAG_OPEN_BRACKET: &'static str = "<";
static TAG_CLOSED_BRACKET: &'static str = ">";
static NORMAL_TAG_CLOSED_BRACKET: &'static str = "</";
static SELF_TAG_CLOSED_BRACKET: &'static str = "/>";
static FORWARD_SLASH: &'static str = "/";
static BACKWARD_SLASH: &'static str = "\\";
static ATTRIBUTE_EQUAL_SIGN: &'static str = "=";
static DOC_TYPE_STARTER_MARKER: &'static str = "!";
static TAG_NAME_SPACE_END: &'static str = " ";

///	Returns a new string where the content is wrapped in a DocumentFragmentContainer:
/// <DocumentFragmentContainer>{content}</DocumentFragmentContainer>.
///
/// This is important to allow you when dealing with an html content where
/// its not just a single root but has siblings and the parser does not handle such
/// cases where there could be more than 1 root element.
pub fn wrap_in_document_fragment_container(data: String) -> String {
    format!("<{}>{}</{}>", FRAME_FLAG_TAG, data, FRAME_FLAG_TAG)
}

fn begins_with_and_after<'a>(
    against: &'a str,
    begin_text: &'a str,
    other_cb: fn(&'a str) -> bool,
) -> bool {
    if &against[0..1] != begin_text {
        return false;
    }
    return other_cb(&against[1..]);
}

impl Default for HTMLParser {
    fn default() -> Self {
        Self::new(VALID_TAG_NAME_SYMBOLS, MarkupTags::is_block_text_tag)
    }
}

impl HTMLParser {
    pub fn with_custom_block_text_tags(handle_as_block_text: CheckTag) -> Self {
        Self::new(VALID_TAG_NAME_SYMBOLS, handle_as_block_text)
    }

    pub fn new(allowed_tag_symbols: &'static [char], handle_as_block_text: CheckTag) -> Self {
        Self {
            allowed_tag_symbols,
            check_is_block_text_tag: handle_as_block_text,
        }
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    pub fn parse<'a>(&self, input: &'a str) -> ParsingResult<Stack<'a>> {
        let mut accumulator = Accumulator::new(input);
        match self._parse(&mut accumulator) {
            Ok(t) => Ok(t),
            Err(err) => Err(ParsingTagError::FailedContentParsing(String::from(
                format!(
                    "Error({:?}): {:?}",
                    err,
                    accumulator.ppeek_at(0, 20).unwrap()
                ),
            ))),
        }
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn _parse<'a>(&self, accumulator: &mut Accumulator<'a>) -> ParsingResult<Stack<'a>> {
        let mut stacks: Vec<Stack> = vec![];
        let mut text_block_tag: Option<MarkupTags> = None;

        while let Some(next) = accumulator.peek(1) {
            ewe_logs::debug!("parse: reading next token: {:?}", next);

            // if we are in a text block tag mode then run the text block tag
            // extraction and append as a child to the top element in the stack
            // which should be the owner until we see the end tag
            if text_block_tag.is_some() {
                ewe_logs::debug!(
                    "parse: reading next token with pending text block: {:?}",
                    next
                );
                match self.parse_element_text_block(
                    text_block_tag.clone().unwrap(),
                    accumulator,
                    &mut stacks,
                ) {
                    Ok(directive) => match directive {
                        ParserDirective::Open(elem) | ParserDirective::Void(elem) => {
                            self.add_as_child_to_last(elem, &mut stacks);

                            // set back to None
                            text_block_tag.take();

                            continue;
                        }
                        ParserDirective::Closed((tag, _)) => {
                            return Err(ParsingTagError::TagWithUnexpectedEnding(
                                tag.to_string().unwrap(),
                            ));
                        }
                    },
                    Err(err) => return Err(err),
                }
            }

            // if we have space character skip it.
            if next.chars().all(|t| SPACE_CHARS.contains(&t)) {
                accumulator.peek_next();
                accumulator.skip();
                continue;
            }

            let mut pop_top_stack = false;
            match self.parse_element_from_accumulator(accumulator, &mut stacks) {
                Ok(elem) => match elem {
                    ParserDirective::Open(elem) => {
                        ewe_logs::debug!("parse: received opening tag indicator: {:?}", elem.tag);

                        let tag = elem.tag.clone().unwrap();
                        if MarkupTags::is_block_text_tag(tag.clone())
                            || (self.check_is_block_text_tag)(tag.clone())
                        {
                            text_block_tag.replace(tag);
                        }

                        stacks.push(elem);
                        continue;
                    }
                    ParserDirective::Closed((tag, (tag_end_start, tag_end_end))) => {
                        ewe_logs::debug!("parse: received closing tag indicator: {:?}", tag);

                        // do a check if we have a previous self closing element
                        // that wont be closed by this new closer.
                        loop {
                            let last_elem = stacks.get(stacks.len() - 1);
                            ewe_logs::debug!(
                                "parse: previous top stack tag {:?} with current: {:?}",
                                last_elem,
                                tag
                            );
                            match last_elem {
                                Some(last_elem) => {
                                    let last_elem_tag = last_elem.tag.clone().unwrap();
                                    if last_elem_tag != tag
                                        && MarkupTags::is_element_self_closing(
                                            last_elem_tag.clone(),
                                        )
                                    {
                                        ewe_logs::debug!("parse: previous top stack tag {:?} is self closing so can close and continue new closing check for {:?}", last_elem_tag, tag);
                                        self.pop_last_as_child_of_previous(&mut stacks);
                                        continue;
                                    }
                                    break;
                                }
                                None => {
                                    return Err(ParsingTagError::UnexpectedParsingWithUnfinishedTag)
                                }
                            }
                        }

                        // standard closing tag operation for normal behaviour
                        match stacks.last_mut() {
                            Some(parent) => {
                                match parent.tag.clone() {
                                    Some(parent_tag) => {
                                        let ptag = parent_tag.clone();
                                        let ctag = parent_tag.clone();

                                        ewe_logs::debug!("parse: reviewing ontop stack tag {:?} and child tag: {:?}", parent_tag, tag);

                                        if parent_tag != tag
                                            && !MarkupTags::is_element_closed_by_closing_tag(
                                                parent_tag, tag,
                                            )
                                        {
                                            return Err(
                                                ParsingTagError::ClosingTagDoesNotMatchTopMarkup(
                                                    String::from(format!(
                                                        "last: {:?} - tag: {:?}",
                                                        ptag, ctag
                                                    )),
                                                ),
                                            );
                                        }

                                        parent.closed = true;
                                        parent.end_range = Some(tag_end_end);
                                        pop_top_stack = true;
                                    }
                                    None => return Err(ParsingTagError::StackedMarkupHasNoTag),
                                }
                            }
                            None => return Err(ParsingTagError::ClosingTagHasZeroElementInStack),
                        }
                    }
                    ParserDirective::Void(elem) => {
                        ewe_logs::debug!("parse: received void tag: {:?}", elem.tag);
                        if stacks.len() == 0 {
                            stacks.push(elem);
                        } else {
                            match stacks.last_mut() {
                                Some(parent) => parent.children.push(elem),
                                None => continue,
                            }
                        }
                    }
                },
                Err(err) => return Err(err),
            }

            if !pop_top_stack || (pop_top_stack && stacks.len() == 1) {
                continue;
            }

            match stacks.pop() {
                Some(previous_elem) => match stacks.last_mut() {
                    Some(parent) => parent.children.push(previous_elem),
                    None => return Err(ParsingTagError::FailedToMoveTopMarkupIntoParentInStack),
                },
                None => return Err(ParsingTagError::FailedToMoveTopMarkupIntoParentInStack),
            }
        }

        if stacks.len() == 0 {
            return Err(ParsingTagError::FailedParsing);
        }

        if stacks.len() > 1 {
            return Err(ParsingTagError::UnexpectedParsingWithUnfinishedTag);
        }

        Ok(stacks.pop().unwrap())
    }

    fn add_as_child_to_last<'a>(
        &self,
        child: Stack<'a>,
        mut stacks: &mut Vec<Stack<'a>>,
    ) -> ParsingResult<()> {
        if stacks.len() == 0 {
            return Ok(());
        }

        let last = stacks.last_mut();

        match stacks.last_mut() {
            Some(parent) => {
                parent.children.push(child);
                Ok(())
            }
            None => Err(ParsingTagError::LastChildWasEmptyShell),
        }
    }

    fn pop_last_as_child_of_previous(&self, mut stacks: &mut Vec<Stack>) -> ParsingResult<()> {
        if stacks.len() == 0 {
            return Ok(());
        }

        match stacks.pop() {
            Some(last_child) => match stacks.last_mut() {
                Some(parent) => {
                    parent.children.push(last_child);
                    Ok(())
                }
                None => Err(ParsingTagError::ExpectedUnemptyStack),
            },
            None => Err(ParsingTagError::LastChildWasEmptyShell),
        }
    }

    fn is_valid_tag_name_token<'a>(&self, text: &'a str) -> bool {
        let is_alphanum = text.chars().any(char::is_alphanumeric);
        let is_allowed_symbol = text.chars().any(|t| self.allowed_tag_symbols.contains(&t));
        is_alphanum || is_allowed_symbol
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_element_from_accumulator<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        while let Some(next) = acc.peek(1) {
            ewe_logs::debug!(
                "parse_element_from_accumulator: reading next token: {:?}: {:?}",
                next,
                acc.vpeek_at(1, 5),
            );

            let comment_scan = acc.vpeek_at(0, 4).unwrap();
            if TAG_OPEN_BRACKET == next && comment_scan == COMMENT_STARTER {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: checking comment scan: {:?}",
                    comment_scan
                );
                match self.parse_comment(acc, &mut stacks) {
                    Ok(elem) => return Ok(elem),
                    Err(err) => return Err(err),
                }
            }

            let xml_starter_scan = acc.vpeek_at(0, 2).unwrap();
            if TAG_OPEN_BRACKET == next && xml_starter_scan == XML_STARTER {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: xml token scan activated: {:?}",
                    xml_starter_scan
                );
                match self.parse_xml_elem(acc, &mut stacks) {
                    Ok(elem) => return Ok(elem),
                    Err(err) => return Err(err),
                }
            }

            if TAG_OPEN_BRACKET == next
                && (acc.vpeek_at(1, 1).unwrap())
                    .chars()
                    .all(char::is_alphanumeric)
            {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: elem token scan activated: {:?}",
                    xml_starter_scan
                );
                match self.parse_elem(acc, &mut stacks) {
                    Ok(elem) => return Ok(elem),
                    Err(err) => return Err(err),
                }
            }

            if TAG_OPEN_BRACKET == next
                && begins_with_and_after(
                    acc.vpeek_at(1, 2).unwrap(),
                    DOC_TYPE_STARTER_MARKER,
                    |t| t.chars().all(char::is_alphabetic),
                )
            {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: doctype token scan activated: {:?}",
                    xml_starter_scan
                );
                match self.parse_doc_type(acc, &mut stacks) {
                    Ok(elem) => return Ok(elem),
                    Err(err) => return Err(err),
                }
            }

            if TAG_OPEN_BRACKET == next
                && begins_with_and_after(acc.vpeek_at(1, 2).unwrap(), &FORWARD_SLASH, |t| {
                    t.chars().all(char::is_alphabetic)
                })
            {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: tag closer token scan activated: {:?}",
                    xml_starter_scan
                );
                match self.parse_closing_tag(acc, &mut stacks) {
                    Ok(elem) => return Ok(elem),
                    Err(err) => return Err(err),
                }
            }

            // if we are dealing with weird case of multi starter, its a bad html
            // lets throw an error so they fix it.
            if TAG_OPEN_BRACKET == next {
                ewe_logs::debug!(
                    "parse_element_from_accumulator: double starter character: {} with block: {:?}",
                    next,
                    acc.vpeek_at(1, 1),
                );

                match acc.vpeek_at(1, 1) {
                    Some(sample) => {
                        if sample != TAG_OPEN_BRACKET {
                            acc.peek_next();

                            let mut elem = Stack::empty();
                            let (content, (tag_start, tag_end)) = acc.take_positional().unwrap();

                            elem.tag.replace(MarkupTags::Text(String::from(content)));
                            elem.start_range = Some(tag_start);
                            elem.end_range = Some(tag_end);

                            return Ok(ParserDirective::Void(elem));
                        }

                        return Err(ParsingTagError::InvalidHTMLContent(String::from(
                            acc.vpeek_at(0, acc.len()).unwrap(),
                        )));
                    }
                    None => return Err(ParsingTagError::FailedParsing),
                }
            }

            let code_block_text = acc.ppeek_at(0, 2).or(Some("")).unwrap();
            let rust_code_block_text = acc.ppeek_at(0, 3).or(Some("")).unwrap();
            ewe_logs::debug!(
                "parse_element_from_accumulator: code block checking: {} with code: {:?}, rust_code: {:?}",
                next,
                code_block_text,
                rust_code_block_text,
            );

            if TEXT_BLOCK_STARTER == next
                && (code_block_text == CODE_BLOCK_STARTER
                    || rust_code_block_text == RUST_BLOCK_STARTER)
            {
                if rust_code_block_text == RUST_BLOCK_STARTER {
                    ewe_logs::debug!("parse_element_from_accumulator: start rust code block");
                    match self.parse_code_block(rust_code_block_text, acc, &mut stacks) {
                        Ok(elem) => return Ok(elem),
                        Err(err) => return Err(err),
                    }
                }
                if code_block_text == CODE_BLOCK_STARTER {
                    ewe_logs::debug!("parse_element_from_accumulator: start non-rust code block");
                    match self.parse_code_block(code_block_text, acc, &mut stacks) {
                        Ok(elem) => return Ok(elem),
                        Err(err) => return Err(err),
                    }
                }
            }

            match self.parse_text_block(acc, &mut stacks) {
                Ok(elem) => return Ok(elem),
                Err(err) => return Err(err),
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_comment<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_comment: saw chracter: {}", next);

            let comment_ender_scan = acc.vpeek_at(0, 3).unwrap();
            ewe_logs::debug!(
                "parse_comment: commend ender scan: '{}'",
                comment_ender_scan
            );

            if comment_ender_scan == COMMENT_ENDER {
                ewe_logs::debug!(
                    "parse_comment: seen comment dashsher: {}",
                    comment_ender_scan
                );

                acc.peek_next_by(comment_ender_scan.len());

                let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                elem.tag
                    .replace(MarkupTags::Comment(String::from(tag_text)));
                elem.start_range = Some(tag_start);
                elem.end_range = Some(tag_end);

                return Ok(ParserDirective::Void(elem));
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_code_block<'c, 'd>(
        &self,
        block_starter: &'c str,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        let blocker_closer_text = ATTRIBUTE_PAIRS.get(block_starter).unwrap();

        ewe_logs::debug!(
            "parse_code_block({:?}, closer: {:?}): to scan next token: {:?}",
            block_starter,
            blocker_closer_text,
            acc.peek(1)
        );

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!(
                "parse_code_block({:?}, closer: {:?}): next token: {:?}",
                block_starter,
                blocker_closer_text,
                next
            );

            let code_closer_sample = acc.vpeek_at(0, 2).unwrap();
            let rust_code_closer_sample = acc.vpeek_at(0, 3).unwrap();

            let is_code_block = code_closer_sample == *blocker_closer_text;
            let is_rust_block = rust_code_closer_sample == *blocker_closer_text;

            if !is_code_block && !is_rust_block {
                ewe_logs::debug!(
                    "parse_code_block({:?}, closer: {:?}): is not closing tage - token: {:?} ({:?}, {:?})",
                    block_starter,
                    blocker_closer_text,
                    next,
                    code_closer_sample,
                    rust_code_closer_sample,
                );
                continue;
            }

            ewe_logs::debug!(
                "parse_code_block({:?}, closer: {:?}): saw ending token - token: {:?} ({:?}, {:?})",
                block_starter,
                blocker_closer_text,
                next,
                code_closer_sample,
                rust_code_closer_sample,
            );

            ewe_logs::debug!(
                "parse_code_block({:?}, closer: {:?}): finishing code exraction: is_rust: {:?}, is_code: {:?}",
                block_starter,
                blocker_closer_text,
                is_rust_block,
                is_code_block,
            );

            if is_rust_block {
                acc.peek_next_by(3);
            } else {
                acc.peek_next_by(2);
            }

            let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
            let code_tag = if is_rust_block {
                MarkupTags::Rust(String::from(tag_text))
            } else {
                MarkupTags::Code(String::from(tag_text))
            };

            elem.tag.replace(code_tag);
            elem.start_range = Some(tag_start);
            elem.end_range = Some(tag_end);

            return Ok(ParserDirective::Void(elem));
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_text_block<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        ewe_logs::debug!("parse_text_block: to scan next token: {:?}", acc.peek(1));

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_text_block: saw chracter: {}", next);

            if next != TAG_OPEN_BRACKET {
                continue;
            }

            // we found the starter for tags, check if
            // there is space after if so
            // then it's still a text.
            match acc.peek(1) {
                Some(text) => {
                    if text.chars().all(|t| SPACE_CHARS.contains(&t)) {
                        continue;
                    }

                    acc.unpeek_next();

                    let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                    elem.tag.replace(MarkupTags::Text(String::from(tag_text)));
                    elem.start_range = Some(tag_start);
                    elem.end_range = Some(tag_end);

                    return Ok(ParserDirective::Void(elem));
                }
                None => return Err(ParsingTagError::InvalidHTMLEnd(String::from(acc.content()))),
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_element_text_block<'c, 'd>(
        &self,
        tag: MarkupTags,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        let tag_name = tag.to_str().unwrap();

        ewe_logs::debug!("parse_element_text_block: begin with tag: {}", tag_name);

        let mut tag_name_with_closer = String::new();
        tag_name_with_closer.push_str(NORMAL_TAG_CLOSED_BRACKET);
        tag_name_with_closer.push_str(tag_name);
        tag_name_with_closer.push(TAG_CLOSED_BRACKET_CHAR);

        tag_name_with_closer = tag_name_with_closer.to_lowercase();

        ewe_logs::debug!(
            "parse_element_text_block: crafted closing tag: {}",
            tag_name_with_closer
        );

        let tag_name_with_closer_len = tag_name_with_closer.len(); // tag_name> we want the length of this

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_element_text_block: saw chracter: {}", next);

            if next != TAG_OPEN_BRACKET {
                continue;
            }

            if TAG_OPEN_BRACKET == next {
                acc.unpeek_next();

                let tag_closing = acc.vpeek_at(0, tag_name_with_closer_len).unwrap();
                ewe_logs::debug!(
                    "parse_element_text_block: checking if closing tag: '{}' against expected closer '{}'",
                    tag_closing,
                    tag_name_with_closer
                );

                if tag_closing.to_lowercase() == tag_name_with_closer {
                    ewe_logs::debug!(
                        "parse_element_text_block: seen closing tag : {}",
                        tag_closing
                    );

                    let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                    elem.tag.replace(MarkupTags::Text(String::from(tag_text)));
                    elem.start_range = Some(tag_start);
                    elem.end_range = Some(tag_end);

                    return Ok(ParserDirective::Void(elem));
                }

                acc.peek_next();
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_doc_type<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        // we already know we are looking at the starter of a tag '<'
        acc.peek_next_by(2);

        acc.take();

        let mut collected_tag = false;
        let mut collected_attrs = false;

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_doc_type: saw chracter: {}", next);

            if !next.chars().all(char::is_alphabetic) {
                if next != TAG_CLOSED_BRACKET && collected_attrs && collected_tag {
                    return Err(ParsingTagError::TagWithUnexpectedEnding(String::from(
                        acc.take().unwrap(),
                    )));
                }

                if next == TAG_CLOSED_BRACKET && collected_attrs && collected_tag {
                    ewe_logs::debug!("parse_doc_type: completed doctype: {:?}", acc.peek(1));

                    elem.closed = true;
                    return Ok(ParserDirective::Void(elem));
                }

                acc.unpeek_next();

                let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                ewe_logs::debug!(
                    "parse_doc_type: generates tagname with collected: {} ({}..{})",
                    tag_text,
                    tag_start,
                    tag_end
                );

                if !collected_tag {
                    elem.tag.replace(MarkupTags::DocType);
                    elem.end_range = Some(tag_end);
                    elem.start_range = Some(tag_start);

                    ewe_logs::debug!("parse_doc_type: generates tagname: {:?}", elem.tag);

                    collected_tag = true;
                }

                self.collect_space(acc)?;

                ewe_logs::debug!(
                    "parse_doc_type: attempt to pull attributes: {:?}",
                    acc.peek(1)
                );

                if !collected_attrs {
                    match self.parse_elem_attribute(acc, &mut elem) {
                        Ok(_) => self.collect_space(acc)?,
                        Err(err) => return Err(err),
                    };
                    collected_attrs = true;
                }

                ewe_logs::debug!(
                    "parse_doc_type: collected attributes if any: {:?}",
                    acc.peek(1)
                );
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_xml_elem<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        // we already know we are looking at the starter of a tag '<'
        acc.peek_next();
        acc.peek_next();

        // skip it after collect forward
        acc.skip();

        let mut collected_tag_name = false;
        let mut collected_attrs = false;

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_xml_elem: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            if next == QUESTION_MARK && acc.peek(1).unwrap() == TAG_CLOSED_BRACKET {
                ewe_logs::debug!("parse_xml_elem: collect tag name: {:?}", next);

                acc.peek_next();
                acc.take();

                return Ok(ParserDirective::Void(elem));
            }

            if !collected_tag_name {
                ewe_logs::debug!("parse_xml_elem: collect tag name: {:?}", next);
                acc.unpeek_next();

                let scan = acc.vpeek_at(0, 10).unwrap();
                let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();

                match MarkupTags::from_str(tag_text) {
                    Ok(tag) => {
                        elem.start_range = Some(tag_start);
                        elem.end_range = Some(tag_end);
                        elem.tag.replace(tag);
                    }
                    Err(err) => return Err(err),
                };

                ewe_logs::debug!("parse_xml_elem: generates tagname: {:?}", elem.tag);

                acc.peek_next();

                collected_tag_name = true;
            }

            if next.chars().all(char::is_whitespace) && !collected_attrs {
                ewe_logs::debug!("parse_xml_elem: collect attributes: {:?}", elem.tag);

                self.collect_space(acc)?;

                match self.parse_elem_attribute(acc, &mut elem) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }

                collected_attrs = true;

                self.collect_space(acc)?;

                continue;
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_elem<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        // we already know we are looking at the starter of a tag '<'
        acc.peek_next();

        // skip it after collect forward
        acc.skip();

        let mut collected_tag_name = false;
        let mut collected_attrs = false;

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_elem: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            if next != TAG_CLOSED_BRACKET && !next.chars().all(char::is_whitespace) {
                ewe_logs::error!(
                    "parse_elem: expected either space or '>' indicating end of start tag: current scan {:?}",
                    acc.scan()
                );

                acc.unpeek_next();

                // check if its self closing tag
                ewe_logs::error!(
                    "parse_elem: kickstart self closing scan logic {:?}",
                    acc.vpeek_at(0, 2)
                );

                if acc.vpeek_at(0, 2).unwrap() == SELF_TAG_CLOSED_BRACKET {
                    ewe_logs::error!("parse_elem: found self closing tag {:?}", acc.scan());

                    if !collected_tag_name {
                        let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();

                        match MarkupTags::from_str(tag_text) {
                            Ok(tag) => {
                                elem.start_range = Some(tag_start);
                                elem.end_range = Some(tag_end);
                                elem.tag.replace(tag);
                            }
                            Err(err) => return Err(err),
                        };
                    }

                    ewe_logs::debug!("parse_elem: generates self closing tagname: {:?}", elem.tag);

                    acc.peek_next_by(2);

                    ewe_logs::debug!("parse_elem: collecting closing text: {:?}", acc.scan());

                    acc.take();

                    ewe_logs::debug!("parse_elem: next scan peek: {:?}", acc.peek(1));

                    return Ok(ParserDirective::Void(elem));
                }

                return Err(ParsingTagError::ExpectingSpaceAfterTagName);
            }

            if !collected_tag_name {
                acc.unpeek_next();

                let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                match MarkupTags::from_str(tag_text) {
                    Ok(tag) => {
                        elem.start_range = Some(tag_start);
                        elem.end_range = Some(tag_end);
                        elem.tag.replace(tag);
                    }
                    Err(err) => return Err(err),
                };

                ewe_logs::debug!("parse_elem generates tagname: {:?}", elem.tag);

                acc.peek_next();

                collected_tag_name = true;
            }

            if next.chars().all(char::is_whitespace) && !collected_attrs {
                self.collect_space(acc)?;

                match self.parse_elem_attribute(acc, &mut elem) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }

                collected_attrs = true;

                self.collect_space(acc)?;

                continue;
            }

            if next == TAG_CLOSED_BRACKET {
                acc.take();

                if MarkupTags::is_element_auto_closed(elem.tag.clone().unwrap()) {
                    return Ok(ParserDirective::Void(elem));
                }

                return Ok(ParserDirective::Open(elem));
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn collect_space(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("collect_space: start seen token: {:?}", next);

            if next.chars().any(|t| SPACE_CHARS.contains(&t)) {
                ewe_logs::debug!("collect_space: consuming token: {:?}", next);
                continue;
            }

            ewe_logs::debug!("collect_space: non-space token: {:?}", next);

            // move backwartds
            acc.unpeek_next();

            // take the space
            acc.take();

            ewe_logs::debug!(
                "collect_space: non-space token after consumed: {:?}",
                acc.peek(1)
            );

            return Ok(());
        }
        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn is_valid_attribute_value_token<'a>(&self, token: &'a str) -> bool {
        ewe_logs::debug!("Checking if valid attribute value token: {:?}", token);
        token.chars().any(|t| {
            t.is_alphanumeric()
                || t.is_ascii_alphanumeric()
                || VALID_ATTRIBUTE_VALUE_CHARS.contains(&t)
                || VALID_ATTRIBUTE_NAMING_CHARS.contains(&t)
        })
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn dequote_str<'a>(&self, text: &'a str) -> &'a str {
        let text_len = text.len();
        ewe_logs::debug!("dequote: text: {:?} with len: {}", text, text_len);
        if (text.starts_with(SINGLE_QUOTE_STR) || text.starts_with(DOUBLE_QUOTE_STR))
            && (text.ends_with(SINGLE_QUOTE_STR) || text.ends_with(DOUBLE_QUOTE_STR))
        {
            return &text[1..(text_len - 1)];
        }
        text
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn is_valid_attribute_name<'a>(&self, token: &'a str) -> bool {
        token.chars().any(|t| {
            t.is_alphanumeric()
                || t.is_ascii_alphanumeric()
                || VALID_ATTRIBUTE_NAMING_CHARS.contains(&t)
        })
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn collect_attribute_value_alphaneumerics(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        let starter = acc.peek(1).unwrap();
        ewe_logs::debug!(
            "collect_attribute_value_alphaneumerics: value starter {:?}",
            starter
        );

        acc.peek_next();

        let is_alphaneumerics_starter = starter.chars().all(char::is_alphanumeric);
        let is_indent_starter = VALID_ATTRIBUTE_STARTER_SYMBOLS_STR.contains(&starter);

        let starter_closer = ATTRIBUTE_PAIRS
            .get(starter)
            .or(Some(&EMPTY_STRING))
            .unwrap();

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!(
                "collect_attribute_value_alphaneumerics: scanning token {:?}: starter={:?}, closer={:?}, is_alpha={:?}, is_char={:?}",
                next,
                starter,
                starter_closer,
                is_alphaneumerics_starter,
                is_indent_starter,
            );

            if is_alphaneumerics_starter && next == TAG_CLOSED_BRACKET {
                ewe_logs::debug!(
                    "collect_attribute_value_alphaneumerics: found close bracket {:?}",
                    next
                );

                // move backwartds
                acc.unpeek_next();

                return Ok(());
            }

            if is_alphaneumerics_starter && next.chars().all(char::is_whitespace) {
                ewe_logs::debug!(
                    "collect_attribute_value_alphaneumerics: is alphaneumerics and ends with a whitespace {:?}",
                    next
                );

                // move backwartds
                acc.unpeek_next();

                return Ok(());
            }

            // if we are dealing with a multiple ender token, just scan in and collect
            let double_next_token = acc.peek(1).unwrap();
            if is_indent_starter && next == *starter_closer && double_next_token == *starter_closer
            {
                ewe_logs::debug!(
                    "collect_attribute_value_alphaneumerics: token {:?} and next {:?} is closer",
                    next,
                    double_next_token,
                );

                continue;
            }

            // if we did not just escape the same starting character - then its means it's the end of
            // the attribute.
            if is_indent_starter && next == *starter_closer && double_next_token != *starter_closer
            {
                ewe_logs::debug!(
                    "collect_attribute_value_alphaneumerics: seen starter ender token {:?}, next: {:?} (starter: {:?}, closer={:?})",
                    next,
                    double_next_token,
                    starter,
                    starter_closer,
                );

                return Ok(());
            }
        }
        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn collect_attribute_name_alphaneumerics(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        while let Some(next) = acc.peek_next() {
            if self.is_valid_attribute_name(next) {
                continue;
            }

            // move backwartds
            acc.unpeek_next();

            return Ok(());
        }
        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn collect_alphaneumerics(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        while let Some(next) = acc.peek_next() {
            if next.chars().any(char::is_alphanumeric) {
                continue;
            }

            // move backwartds
            acc.unpeek_next();

            return Ok(());
        }
        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), tracing::instrument(level = "trace", skip(self)))]
    fn parse_elem_attribute<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        stack: &mut Stack<'d>,
    ) -> ParsingResult<()>
    where
        'c: 'd,
    {
        ewe_logs::debug!("parse_elem_attribute: begin from: {:?}", acc.peek(1));
        self.collect_space(acc)?;

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_elem_attribute: seen next token: {:?}", next);

            if self.is_valid_attribute_name(next) {
                continue;
            }

            // are we dealing with single attributes
            if next.chars().all(char::is_whitespace)
                || next == ATTRIBUTE_EQUAL_SIGN
                || next == TAG_CLOSED_BRACKET
            {
                ewe_logs::debug!(
                    "parse_elem_attribute: checking ready value collection: {:?}",
                    next
                );

                if next.chars().all(char::is_whitespace) || next == TAG_CLOSED_BRACKET {
                    ewe_logs::debug!("parse_elem_attribute: actually space or < (tag closer), so collect valueless attribute: {:?}", acc.scan());
                    acc.unpeek_next();

                    let attribute_name = acc.take().unwrap();

                    ewe_logs::debug!(
                        "parse_elem_attribute: collected attribute name: {:?}",
                        attribute_name
                    );

                    if attribute_name.len() != 0 {
                        stack.add_attr(attribute_name, AttrValue::Text(""));
                    }

                    return Ok(());
                }

                if next == ATTRIBUTE_EQUAL_SIGN {
                    ewe_logs::debug!("parse_elem_attribute: actually equal sign, so start collecting value: {:?}", next);

                    acc.unpeek_next();

                    let attr_name = acc.take().unwrap();

                    ewe_logs::debug!(
                        "parse_elem_attribute: collected attribute name: {:?}",
                        attr_name
                    );

                    acc.peek_next();
                    acc.take();

                    ewe_logs::debug!(
                        "parse_elem_attribute: going to next scan: {:?} -> {:?}",
                        acc.peek(1),
                        acc.scan()
                    );

                    if acc.peek(1).unwrap() == BACKWARD_SLASH {
                        acc.peek_next();
                    }

                    if !self.is_valid_attribute_value_token(acc.peek(1).unwrap()) {
                        return Err(ParsingTagError::AttributeValueNotValidStarter(
                            String::from(acc.peek(1).unwrap()),
                        ));
                    }

                    ewe_logs::debug!(
                        "parse_elem_attribute: after validating to next scan: {:?} -> {:?}",
                        acc.peek(1),
                        acc.peek(2)
                    );

                    // collect value
                    self.collect_attribute_value_alphaneumerics(acc)?;

                    let attr_value_text = acc.take().unwrap();

                    let attr_value = if attr_value_text.starts_with(SINGLE_QUOTE_STR)
                        || attr_value_text.starts_with(DOUBLE_QUOTE_STR)
                    {
                        AttrValue::Text(self.dequote_str(attr_value_text))
                    } else if attr_value_text.starts_with(RUST_BLOCK_STARTER) {
                        AttrValue::Rust(attr_value_text)
                    } else if attr_value_text.starts_with(CODE_BLOCK_STARTER) {
                        AttrValue::Code(attr_value_text)
                    } else if attr_value_text.starts_with(TEXT_BLOCK_STARTER) {
                        AttrValue::Block(attr_value_text)
                    } else {
                        AttrValue::Text(attr_value_text)
                    };

                    ewe_logs::debug!(
                        "parse_elem_attribute: collected attribute value: {:?}",
                        attr_value
                    );

                    match acc.peek(1) {
                        Some(token) => {
                            ewe_logs::debug!(
                                "parse_elem_attribute: check next token if end of attr value: {:?}",
                                token
                            );
                            if !token.chars().all(char::is_whitespace)
                                && token != TAG_CLOSED_BRACKET
                                && token != FORWARD_SLASH
                            {
                                return Err(ParsingTagError::AttributeValueNotValidEnding(
                                    String::from(token),
                                ));
                            }
                        }
                        None => {
                            return Err(ParsingTagError::AttributeValueNotValidEnding(
                                String::from(acc.vpeek_at(0, 5).unwrap()),
                            ));
                        }
                    }

                    stack.attrs.push((attr_name, attr_value));

                    ewe_logs::debug!("parse_elem_attribute: done with no failure");

                    return Ok(());
                }
            }

            // move backwartds
            acc.unpeek_next();

            // take the space
            acc.take();

            return Ok(());
        }
        Err(ParsingTagError::FailedParsing)
    }

    fn parse_closing_tag<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        stacks: &[Stack],
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        // we already know we are looking at the starter of a tag '<'
        // acc.peek_next_by(2);
        acc.peek_next();
        acc.peek_next();

        ewe_logs::debug!("parse_closing_tag: consuming token: {:?}", acc.take());

        while let Some(next) = acc.peek_next() {
            ewe_logs::debug!("parse_closing_tag: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            // move backwartds
            if next.chars().all(char::is_whitespace) {
                ewe_logs::debug!("parse_closing_tag: seen space: {:?}", next);

                acc.unpeek_next();
                self.collect_space(acc)?;

                continue;
            }

            if next != TAG_CLOSED_BRACKET {
                ewe_logs::debug!("parse_closing_tag: invalid token: {}", next);

                return Err(ParsingTagError::TagWithUnexpectedEnding(String::from(
                    acc.take().unwrap(),
                )));
            }

            acc.unpeek_next();

            let (mut tag_text, tag_positioning) = acc.take_positional().unwrap();

            // TODO(alex.ewetumo): Not sure why this occurs, it never occurs during
            // tests but only during benchmarks like the actual skipping does not work
            // I had to specific trigger till i found all weird edge cases.
            //
            // It still can not reproduce this at all in actual code run in tests even with
            // the same sample file, something is definitely weird and off here.
            if tag_text.starts_with("</")
                || tag_text.starts_with("></")
                || tag_text.starts_with("/")
            {
                if tag_text.starts_with("/") {
                    tag_text = &tag_text[1..];
                } else if tag_text.starts_with("></") {
                    tag_text = &tag_text[3..];
                } else {
                    tag_text = &tag_text[2..];
                }

                ewe_logs::debug!(
                    "parse_closing_tag: BUG!!!: found bad bug with (</, ></, /): {:?} arond area: {:?}",
                    tag_text,
                    acc.scan()
                );
            }

            let closing_tag = MarkupTags::from_str(tag_text)?;
            ewe_logs::debug!(
                "parse_closing_tag: found closer for: {:?} with next token: {:?}",
                closing_tag,
                acc.peek(1)
            );

            acc.peek_next();

            ewe_logs::debug!("parse_closing_tag: consume token : {:?}", tag_text);

            return Ok(ParserDirective::Closed((closing_tag, tag_positioning)));
        }
        Err(ParsingTagError::FailedParsing)
    }
}

#[cfg(test)]
mod html_parser_test {

    use super::*;
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_empty_string() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(""));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(54),
                attrs: vec![],
                children: vec![]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_with_double_quoted_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang=\"en\">"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(74),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_single_quoted_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang='en'>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(74),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_alphaneumeric_only_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype lang=en>"));
        let result = parser.parse(data.as_str());

        ewe_logs::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(72),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    end_range: Some(36),
                    start_range: Some(29),
                    attrs: vec![("lang", AttrValue::Text("en"))],
                    children: vec![]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_can_parse_doctype() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<!doctype>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(64),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::DocType),
                    closed: true,
                    start_range: Some(29),
                    end_range: Some(36),
                    attrs: vec![],
                    children: vec![]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_with_spaced_tag_starter() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello< </div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(72),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::HTML(HTMLTags::Div)),
                    closed: true,
                    start_range: Some(28),
                    end_range: Some(44),
                    attrs: vec![],
                    children: vec![Stack {
                        tag: Some(MarkupTags::Text("hello< ".to_string())),
                        closed: false,
                        attrs: vec![],
                        children: vec![],
                        start_range: Some(32),
                        end_range: Some(39),
                    }]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello</div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::DocumentFragmentContainer)),
                closed: true,
                start_range: Some(1),
                end_range: Some(70),
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::HTML(HTMLTags::Div)),
                    closed: true,
                    start_range: Some(28),
                    end_range: Some(42),
                    attrs: vec![],
                    children: vec![Stack {
                        tag: Some(MarkupTags::Text("hello".to_string())),
                        closed: false,
                        attrs: vec![],
                        children: vec![],
                        start_range: Some(32),
                        end_range: Some(37),
                    }]
                }]
            }
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_with_tag_starter_in_text() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello<</div>"));
        let result = parser.parse(data.as_str());

        assert!(matches!(
            result,
            ParsingResult::Err(ParsingTagError::FailedContentParsing(_))
        ));
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_special_characters() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from("<div>hello:_-</div>"));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Text(String::from("hello:_-")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children_with_self_closing_br_and_circle(
    ) {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle/></div><div><br/></div>",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_invalid_html_parsing_multiple_breakpoint_tags_with_sibling_with_bad_closer() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
                <circle/>
            </div>
            <div>
            <br>
            <br>
            </section>
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Err(_)));
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_multiple_breakpoint_tags_with_sibling_with_no_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
                <circle/>
            </div>
            <br>
            <br>
            <div>
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Div),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_multiple_breakpoint_tags_as_children() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
            <circle/>
            </div>
            <div>
            <br>
            <br>
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_multiple_breakpoint_tags() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
            <circle/>
            </div>
            <div>
            <br/>
            <br/>
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children_with_self_closing_br() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle></circle></div><div><br/></div>",
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_single_node_text_with_multiple_children() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div><circle></circle></div><div><br></div>",
        ));
        let result = parser.parse(data.as_str());

        ewe_logs::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();
        ewe_logs::debug!("ParsedTags: {:?}", parsed);

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::SVG(SVGTags::Circle),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Br),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_html_with_script_text_element() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"<div>
                <script>
                	let some_var = window.get('alex');
                    let elem = (<section>alex</section>)
                </script>
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::HTML(HTMLTags::Script),
                MarkupTags::Text(String::from("\n                \tlet some_var = window.get('alex');\n                    let elem = (<section>alex</section>)\n                ")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments_with_extended_ender() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"<div>
                <!-- This is a comment--->
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment--->")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments_with_character() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"<div>
                <!-- This is a comment-->
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment-->")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_comments() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"<div>
                <!-- This is a comment -->
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from("<!-- This is a comment -->")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_multiline_comments() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"<div>
                <!--
                    This is a comment
                -->
            </div>
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Comment(String::from(
                    "<!--\n                    This is a comment\n                -->"
                ))
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_section_tag_with_at_sign_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                <sec@tion />
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::Component(String::from("sec@tion")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_section_tag_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                <section />
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Section),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_with_div_tag_self_closing() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                <div />
            "#,
        ));
        let result = parser.parse(data.as_str());

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_can_parse_xml_starter() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <?xml version="1.0" ?>
            "#,
        ));
        let result = parser.parse(data.as_str());

        ewe_logs::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Xml),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_basic_html_parsing_style_tag() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            	<style type="text/css">
                	body {
                    	background: black;
                    }

                    h1 {
                    	color: white;
                        background-color: #43433;
                    }
                </style>
            "#,
        ));
        let result = parser.parse(data.as_str());

        ewe_logs::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Style),
                MarkupTags::Text(String::from("\n                \tbody {\n                    \tbackground: black;\n                    }\n\n                    h1 {\n                    \tcolor: white;\n                        background-color: #43433;\n                    }\n                ")),
            ],
            parsed
        )
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_variation_with_double_curly_bracket() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={{some of other vaulue}}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            ("jail", AttrValue::Code("{{some of other vaulue}}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_variation_with_curly_bracket() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={some of other vaulue}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            ("jail", AttrValue::Block("{some of other vaulue}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_can_handle_non_english_characters() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                     <html lang=\"sv\">
                        <head>
                            <title>Här kan man va</title>
                        </head>
                        <body>
                        	<boäk näme=20 age="äry" flag={ämu}>Hi</boäk>
                            <h1>Tjena världen!</h1>
                            <p>Tänkte bara informera om att Sverige är bättre än Finland i ishockey.</p>
                        </body>
                    </html>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    #[traced_test]
    #[test]
    fn test_can_parse_different_samples_that_should_be_valid() {
        let parser = HTMLParser::default();

        let test_cases = vec![
            wrap_in_document_fragment_container(String::from(
                r#"
        <svg width="600" height="600">
            <rect id="rec" x="300" y="100" width="300" height="100" style="fill:lime">
            <animate attributeName="x" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="300" to="0" />
            <animate attributeName="y" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="100" to="0" />
            <animate attributeName="width" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="300" to="800" />
            <animate attributeName="height" attributeType="XML" begin="0s" dur="6s" fill="freeze" from="100" to="300" />
            <animate attributeName="fill" attributeType="CSS" from="lime" to="red" begin="2s" dur="4s" fill="freeze" />
            </rect>
            <g transform="translate(100,100)">
            <text id="TextElement" x="0" y="0" style="font-family:Verdana;font-size:24; visibility:hidden"> It's SVG!
                <set attributeName="visibility" attributeType="CSS" to="visible" begin="1s" dur="5s" fill="freeze" />
                <animateMotion path="M 0 0 L 100 100" begin="1s" dur="5s" fill="freeze" />
                <animate attributeName="fill" attributeType="CSS" from="red" to="blue" begin="1s" dur="5s" fill="freeze" />
                <animateTransform attributeName="transform" attributeType="XML" type="rotate" from="-30" to="0" begin="1s" dur="5s" fill="freeze" />
                <animateTransform attributeName="transform" attributeType="XML" type="scale" from="1" to="3" additive="sum" begin="1s" dur="5s" fill="freeze" />
            </text>
            </g>
            Sorry, your browser does not support inline SVG.
        </svg>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1.0">
                    <title>Document</title>
                    <style>
                        body {
                            background: black;
                        }

                        h1 {
                            color: white;
                        }
                    </style>
                </head>
                <body>
                    <h1>Hello world</h1>
                    <!-- There should be more text here -->
                    <script>
                        const title = document.querySelector("h1")
                        title.innerText = "Hello from script"
                    </script>
                </body>
            </html>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                     <html lang=\"sv\">
                        <head>
                            <title>Här kan man va</title>
                        </head>
                        <body>
                            <h1>Tjena världen!</h1>
                            <p>Tänkte bara informera om att Sverige är bättre än Finland i ishockey.</p>
                        </body>
                    </html>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                <HTML>
                    <head>
                        <title>title</title>
                    </head>
                    <body>
                        <ul>
                            <li></li>
                            <li></li>
                            <li></li>
                        </ul>
                    </body>
                </hTML>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                    <div class='1'>
                        <div class='1'>
                            <div class='1'>
                                <div class='1'>
                                    <div class='1'>
                                        <div class='1'>
                                            <div class='1'>
                                                <div class='1'>
                                                    <!--this is deep-->
                                                    hello world
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                    <script>
                        const person_creator = ({ name, symtoms }) => {
                            let person = {}
                            person.name = name
                            person.symtoms = {}
                            for (symtom of symtoms) {
                                person.symtoms[symtom] = true
                            }
                            return person
                        }

                        const main = () => {
                            let name = 'mathias'
                            let symtoms = ['Dunning-Kruger', 'ACDC', 'Slacker']

                            setTimeout(() => {
                                let person = person_creator({ name, symtoms })
                                if (person.symtoms.hasOwnProperty('Dunning-Kruger')) {
                                    console.log('yeah buddy, that\'s right')
                                }
                            }, 1337)
                        }

                        main()
                    </script>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
                  <!-- comment -->
                    <!-- comment -->
                    <!DOCTYPE html>
                    <!-- comment -->
                    <!-- comment -->
                    <html>
                    <!-- comment -->
                    </html>
                    <!-- comment -->
                    <!-- comment -->
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
				hello<!--world?--><div/>
            "#,
            )),
            wrap_in_document_fragment_container(String::from(
                r#"
				hello<!--world?--><div/>
            "#,
            )),
        ];

        for test_case in test_cases {
            let result = parser.parse(test_case.as_str());
            assert!(matches!(result, ParsingResult::Ok(_)));
        }
    }

    static HTML: &'static str = include_str!("../benches/wikipedia-2020-12-21.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(HTML.to_string()));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    static HTML_BIG: &'static str = include_str!("../benches/wikipedia_on_wikipedia.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_big_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(HTML_BIG.to_string()));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    static HTML_SMALLEST: &'static str = include_str!("../benches/scraping_course.html");

    #[traced_test]
    #[test]
    fn test_html_can_handle_smallest_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(HTML_SMALLEST.to_string()));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            "<div jail={{some of other vaulue}} rust={{{some rust value}}}>hello</div>",
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            ("jail", AttrValue::Code("{{some of other vaulue}}"))
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            ("rust", AttrValue::Rust("{{{some rust value}}}"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_with_other_attributes_and_multiline_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div jail={{
                some of other vaulue
            }} name={name} rust={{{
             some rust value
             }}} addr="supply">hello</div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            ("name", AttrValue::Block("{name}"))
        );

        assert_eq!(
            *div.attrs.get(2).unwrap(),
            (
                "rust",
                AttrValue::Rust("{{{\n             some rust value\n             }}}")
            )
        );

        assert_eq!(
            *div.attrs.get(3).unwrap(),
            ("addr", AttrValue::Text("supply"))
        );
    }

    #[traced_test]
    #[test]
    fn test_html_attribute_with_rust_multiline_code_block() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div jail={{
                some of other vaulue
            }} rust={{{
             some rust value
             }}}>hello</div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        assert_eq!(
            *div.attrs.get(1).unwrap(),
            (
                "rust",
                AttrValue::Rust("{{{\n             some rust value\n             }}}")
            )
        );
    }

    #[traced_test]
    #[test]
    fn test_html_can_customize_what_is_block_text_tag() {
        let parser = HTMLParser::with_custom_block_text_tags(|tag| tag.to_str().unwrap() == "div");

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
            	<section></section>
             </div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();

        let div = parsed.children.get(0).unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Text(String::from(
                    "\n            \t<section></section>\n             "
                )),
            ],
            tags
        )
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_code_block_in_body_and_as_attribute() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div jail={{
                some of other vaulue
            }}>
             {{
                some of other vaulue
             }}
             </div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();

        let div = parsed.children.get(0).unwrap();

        assert_eq!(
            *div.attrs.get(0).unwrap(),
            (
                "jail",
                AttrValue::Code("{{\n                some of other vaulue\n            }}")
            )
        );

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
            ],
            tags
        )
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_code_block_in_body() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
             {{
                some of other vaulue
             }}
             </div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
            ],
            tags
        )
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_multiple_code_block_in_body() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
             {{
                some of other vaulue
             }}
           	 {{{
             some rust value
             }}}
             </div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        let div = parsed.children.get(0).unwrap();

        let tags = parsed.get_tags();

        assert_eq!(
            vec![
                MarkupTags::HTML(HTMLTags::DocumentFragmentContainer),
                MarkupTags::HTML(HTMLTags::Div),
                MarkupTags::Code(String::from(
                    "{{\n                some of other vaulue\n             }}"
                )),
                MarkupTags::Rust(String::from(
                    "{{{\n             some rust value\n             }}}"
                )),
            ],
            tags
        )
    }

    #[traced_test]
    #[test]
    fn test_html_text_with_multiple_code_block_in_body_with_bad_closer() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
            <div>
             {{
                some of other vaulue
             }}
           	 {{{
             some rust value
             }} }
             </div>
            "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Err(_)));
    }
}
