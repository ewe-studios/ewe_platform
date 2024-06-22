// Module implements the parser has defined in: https://github.com/taoqf/node-html-parser

use core::str::Chars;
use std::{cell, rc};

use anyhow::anyhow;
use lazy_regex::{lazy_regex, regex, regex_captures, Lazy, Regex, RegexBuilder};
use lazy_static::lazy_static;
use phf::phf_map;
use std::{any, collections::HashMap, str::FromStr};
use thiserror::Error;
use tracing::trace;

use crate::markup::{ElementResult, Markup};

lazy_static! {
    static ref ATTRIBUTE_PAIRS: HashMap<&'static str, &'static str> =
        vec![("{", "}"), ("(", ")"), ("[", "]"), ("'", "'"), ("\"", "\""),]
            .into_iter()
            .collect();
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParsingTagError {
    #[error("unknown tag can't parse")]
    UnknownTag,

    #[error("failed parsing text tag")]
    FailedParsing,

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

    #[error("Parser encountered a closing tag different from top markup in stack nor does not close top markup in rules")]
    ClosingTagDoesNotMatchTopMarkup,

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
            MarkupTags::Comment(text) | MarkupTags::Text(text) | MarkupTags::Component(text) => {
                Ok(text.clone())
            }
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

#[derive(Clone, Debug)]
pub struct Accumulator<'a> {
    content: &'a str,
    pos: usize,
    peek_pos: usize,
}

impl<'a> Accumulator<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            pos: 0,
            peek_pos: 0,
        }
    }

    /// Returns the total length of the string being accumulated on.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// peek_rem_len returns the remaining count of strings
    /// left from the current peeks's cursor.
    pub fn peek_rem_len(&self) -> usize {
        (&self.content[self.peek_pos..]).len()
    }

    /// rem_len returns the remaining count of strings
    /// left from the current position's cursor
    /// regardless of where the peek cursor is at.
    pub fn rem_len(&self) -> usize {
        (&self.content[self.pos..]).len()
    }

    /// resets resets the location of the cursors for both read and peek to 0.
    /// Basically moving them to the start position.
    pub fn reset(&mut self) {
        self.reset_to(0)
    }

    /// reset_to lets you reset the position of the cursor for both
    /// position and peek to the to value.
    pub fn reset_to(&mut self, to: usize) {
        self.pos = to;
        self.peek_pos = to;
    }

    /// skip will skip all the contents of the accumulator up to
    /// the current position of the peek cursor.
    pub fn skip(&mut self) {
        self.pos = self.peek_pos
    }

    /// peek pulls the next token at the current peek position
    /// cursor which will
    pub fn peek(&mut self, by: usize) -> Option<&'a str> {
        self.peek_slice(by)
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a mount of step and returns the next
    /// token string.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn peek_next_by(&mut self, by: usize) -> Option<&'a str> {
        if let Some(res) = self.peek_slice(by) {
            self.peek_pos = self.ensure_character_boundary_index(self.peek_pos + by);
            return Some(res);
        }
        None
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor till the end.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn scan_remaining(&mut self) -> Option<&'a str> {
        Some(&self.content[self.pos..])
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor and the peek cursor so you can
    /// pull the string right at the current range.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn scan(&mut self) -> Option<&'a str> {
        Some(&self.content[self.pos..self.peek_pos])
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a step and returns the next
    /// token string.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn peek_next(&mut self) -> Option<&'a str> {
        if let Some(res) = self.peek_slice(1) {
            self.peek_pos = self.ensure_character_boundary_index(self.peek_pos + 1);
            return Some(res);
        }
        None
    }

    /// unpeek_next reverses the last forward move of the peek
    /// cursor by -1.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn unpeek_next(&mut self) -> Option<&'a str> {
        if let Some(res) = self.unpeek_slice(1) {
            return Some(res);
        }
        None
    }

    /// unpeek_slice lets you reverse the peek cursor position
    /// by a certain amount to reverse the forward movement.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    fn unpeek_slice(&mut self, by: usize) -> Option<&'a str> {
        if self.peek_pos == 0 {
            return None;
        }

        // unpeek only works when we are higher then current pos cursor.
        // it should have no effect when have not moved forward
        if self.peek_pos > self.pos {
            self.peek_pos = self.inverse_ensure_character_boundary_index(self.peek_pos - 1);
        }

        let new_peek_pos = self.ensure_character_boundary_index(self.peek_pos + by);
        Some(&self.content[self.peek_pos..new_peek_pos])
    }

    /// ppeek_at allows you to do a non-permant position cursor adjustment
    /// by taking the current position cursor index with an adjustment
    /// where we add the `from` (pos + from) to get the new
    /// position to start from and `to` is added (pos + from + to)
    /// the position to end at, if the total is more than the length of the string
    /// then its adjusted to be the string last index for the slice.
    ///
    /// It's a nice way to get to see whats at a given position without changing
    /// the current location of the peek cursor.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    fn ppeek_at(&mut self, from: usize, to: usize) -> Option<&'a str> {
        let new_peek_pos = self.ensure_character_boundary_index(self.pos + from);
        let mut until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        until_pos = self.ensure_character_boundary_index(until_pos);

        Some(&self.content[new_peek_pos..until_pos])
    }

    /// vpeek_at allows you to do a non-permant peek cursor adjustment
    /// by taking the current peek cursor position with an adjustment
    /// where we add the `from` (peek_cursor + from) to get the new
    /// position to start from and `to` is added (peek_cursor + from + to)
    /// the position to end at, if the total is more than the length of the string
    /// then its adjusted to be the string last index for the slice.
    ///
    /// It's a nice way to get to see whats at a given position without changing
    /// the current location of the peek cursor.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    fn vpeek_at(&mut self, from: usize, to: usize) -> Option<&'a str> {
        let mut new_peek_pos = self.peek_pos + from;
        let mut until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        // ensure we are always at the char boundary
        new_peek_pos = self.ensure_character_boundary_index(new_peek_pos);
        until_pos = self.ensure_character_boundary_index(until_pos);

        tracing::debug!(
            "Check if we are out of char boundary: start: {}:{}, end: {}:{}",
            new_peek_pos,
            self.content.is_char_boundary(new_peek_pos),
            until_pos,
            self.content.is_char_boundary(until_pos)
        );
        Some(&self.content[new_peek_pos..until_pos])
    }

    fn inverse_ensure_character_boundary_index(&self, current_index: usize) -> usize {
        let mut next_index = current_index;
        // ensure we are always at the char boundary
        loop {
            if !self.content.is_char_boundary(next_index) {
                next_index -= 1;
                continue;
            }
            break;
        }
        return next_index;
    }

    fn ensure_character_boundary_index(&self, current_index: usize) -> usize {
        let mut next_index = current_index;
        // ensure we are always at the char boundary
        loop {
            if !self.content.is_char_boundary(next_index) {
                next_index += 1;
                continue;
            }
            break;
        }
        return next_index;
    }

    /// peek_slice allows you to peek forward by an amount
    /// from the current peek cursor position.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    fn peek_slice(&mut self, by: usize) -> Option<&'a str> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }
        let mut from = self.ensure_character_boundary_index(self.peek_pos);
        let mut until_pos = self.ensure_character_boundary_index(self.peek_pos + by);
        Some(&self.content[from..until_pos])
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position with adjustment on `by` amount i.e
    /// str[position_cursor..(peek_cursor + by_value)].
    ///
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn take_with_amount(&mut self, by: usize) -> Option<(&'a str, (usize, usize))> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }

        let mut until_pos = if self.peek_pos + by > self.content.len() {
            self.content.len()
        } else {
            self.peek_pos + by
        };

        if self.pos >= self.content.len() {
            return None;
        }

        let from = self.ensure_character_boundary_index(self.pos);
        until_pos = self.ensure_character_boundary_index(until_pos);
        let position = (from, until_pos);

        tracing::debug!(
            "take_with_amount: content len: {} with pos: {}, peek_pos: {}, by: {}, until: {}",
            self.content.len(),
            self.pos,
            self.peek_pos,
            by,
            until_pos,
        );

        let content_slice = &self.content[from..until_pos];

        tracing::debug!(
            "take_with_amount: sliced worked from: {}, by: {}, till loc: {} with text: '{}'",
            self.pos,
            by,
            until_pos,
            content_slice,
        );

        let res = Some((content_slice, position));
        self.pos = self.peek_pos;
        res
    }

    /// take_positional returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e str[position_cursor...peek_cursor].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    pub fn take_positional(&mut self) -> Option<(&'a str, (usize, usize))> {
        self.take_with_amount(0)
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e str[position_cursor...peek_cursor].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    pub fn take(&mut self) -> Option<&'a str> {
        match self.take_with_amount(0) {
            Some((text, _)) => Some(text),
            None => None,
        }
    }
}

#[cfg(test)]
mod accumulator_tests {
    use super::*;

    #[test]
    fn test_can_use_accumulator_to_peek_next_character() {
        let mut accumulator = Accumulator::new("hello");
        assert_eq!("h", accumulator.peek(1).unwrap());
        assert_eq!("h", accumulator.peek(1).unwrap());
    }

    #[test]
    fn test_can_use_accumulator_to_peek_two_characters_away() {
        let mut accumulator = Accumulator::new("hello");
        assert_eq!("he", accumulator.peek(2).unwrap());
    }

    #[test]
    fn test_can_virtual_peek_ahead_without_changing_peek_cursor() {
        let mut accumulator = Accumulator::new("hello");

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());

        assert_eq!("llo", accumulator.vpeek_at(0, 3).unwrap()); // from peek cursor till 3 ahead
        assert_eq!("lo", accumulator.vpeek_at(1, 3).unwrap()); // from 1 character ahead of peek cursor

        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());
        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_to_accumulate_more_seen_text() {
        let mut accumulator = Accumulator::new("hello");

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());

        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_and_take_text_then_continue_peeking() {
        let mut accumulator = Accumulator::new("hello");

        assert_eq!(5, accumulator.len());

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());

        assert_eq!(5, accumulator.len());
        assert_eq!(5, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!("hel", accumulator.take().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!(None, accumulator.peek_next());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!("lo", accumulator.take().unwrap());

        assert_eq!(0, accumulator.rem_len());

        assert_eq!(None, accumulator.peek_next());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stack<'a> {
    tag: Option<MarkupTags>,
    closed: bool,
    children: Vec<Stack<'a>>,
    attrs: Vec<(&'a str, &'a str)>,
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

    pub fn add_attr(&mut self, name: &'a str, value: &'a str) {
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
    pub handle_as_block_text: CheckTag,
}

// https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
static MARKUP_PATTERN_REGEXP: Lazy<Regex> = lazy_regex!(
    r#"<!--[\s\S]*?-->|<(\/?)([a-zA-Z][-.:0-9_a-zA-Z]*)((?:\s+[^>]*?(?:(?:'[^']*')|(?:"[^"]*"))?)*)\s*(\/?)>"#
);

// use with ".."/g"
static ATTRIBUTE_PATTERN: Lazy<Regex> =
    lazy_regex!(r#"/(?:^|\s)(id|class)\s*=\s*((?:'[^']*')|(?:"[^"]*")|\S+)"#i); // use with /../gi

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

static EMPTY_STRING: &'static str = "";
static FRAME_FLAG_TAG: &'static str = "DocumentFragmentContainer";
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
        Self {
            allowed_tag_symbols: VALID_TAG_NAME_SYMBOLS,
            handle_as_block_text: MarkupTags::is_block_text_tag,
        }
    }
}

impl HTMLParser {
    pub fn new(allowed_tag_symbols: &'static [char], handle_as_block_text: CheckTag) -> Self {
        Self {
            allowed_tag_symbols,
            handle_as_block_text,
        }
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    pub fn parse<'a>(&self, input: &'a str) -> ParsingResult<Stack<'a>> {
        let mut accumulator = Accumulator::new(input);

        let mut stacks: Vec<Stack> = vec![];
        let mut text_block_tag: Option<MarkupTags> = None;

        while let Some(next) = accumulator.peek(1) {
            tracing::debug!("parse: reading next token: {:?}", next);

            // if we are in a text block tag mode then run the text block tag
            // extraction and append as a child to the top element in the stack
            // which should be the owner until we see the end tag
            if text_block_tag.is_some() {
                tracing::debug!(
                    "parse: reading next token with pending text block: {:?}",
                    next
                );
                match self.parse_element_text_block(
                    text_block_tag.clone().unwrap(),
                    &mut accumulator,
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
            match self.parse_element_from_accumulator(&mut accumulator, &mut stacks) {
                Ok(elem) => match elem {
                    ParserDirective::Open(elem) => {
                        tracing::debug!("parse: received opening tag indicator: {:?}", elem.tag);

                        let tag = elem.tag.clone().unwrap();
                        if MarkupTags::is_block_text_tag(tag.clone()) {
                            text_block_tag.replace(tag);
                        }

                        stacks.push(elem);
                        continue;
                    }
                    ParserDirective::Closed((tag, (tag_end_start, tag_end_end))) => {
                        tracing::debug!("parse: received closing tag indicator: {:?}", tag);

                        // do a check if we have a previous self closing element
                        // that wont be closed by this new closer.
                        loop {
                            let last_elem = stacks.get(stacks.len() - 1);
                            tracing::debug!(
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
                                        tracing::debug!("parse: previous top stack tag {:?} is self closing so can close and continue new closing check for {:?}", last_elem_tag, tag);
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
                                        tracing::debug!("parse: reviewing ontop stack tag {:?} and child tag: {:?}", parent_tag, tag);
                                        if parent_tag != tag
                                            && !MarkupTags::is_element_closed_by_closing_tag(
                                                parent_tag, tag,
                                            )
                                        {
                                            return Err(
                                                ParsingTagError::ClosingTagDoesNotMatchTopMarkup,
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
                        tracing::debug!("parse: received void tag: {:?}", elem.tag);
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn parse_element_from_accumulator<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        while let Some(next) = acc.peek(1) {
            tracing::debug!(
                "parse_element_from_accumulator: reading next token: {:?}: {:?}",
                next,
                acc.vpeek_at(1, 5),
            );

            let comment_scan = acc.vpeek_at(0, 4).unwrap();
            if TAG_OPEN_BRACKET == next && comment_scan == COMMENT_STARTER {
                tracing::debug!(
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
                tracing::debug!(
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
                tracing::debug!(
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
                tracing::debug!(
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
                tracing::debug!(
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
                tracing::debug!(
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

            match self.parse_text_block(acc, &mut stacks) {
                Ok(elem) => return Ok(elem),
                Err(err) => return Err(err),
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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
            tracing::debug!("parse_comment: saw chracter: {}", next);

            let comment_ender_scan = acc.vpeek_at(0, 3).unwrap();
            tracing::debug!(
                "parse_comment: commend ender scan: '{}'",
                comment_ender_scan
            );

            if comment_ender_scan == COMMENT_ENDER {
                tracing::debug!(
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn parse_text_block<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        mut stacks: &mut Vec<Stack>,
    ) -> ParsingResult<ParserDirective<'d>>
    where
        'c: 'd,
    {
        let mut elem = Stack::empty();

        tracing::debug!("parse_text_block: to scan next token: {:?}", acc.peek(1));

        while let Some(next) = acc.peek_next() {
            tracing::debug!("parse_text_block: saw chracter: {}", next);

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
                None => return Err(ParsingTagError::InvalidHTMLEnd(String::from(acc.content))),
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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

        tracing::debug!("parse_element_text_block: begin with tag: {}", tag_name);

        let mut tag_name_with_closer = String::new();
        tag_name_with_closer.push_str(NORMAL_TAG_CLOSED_BRACKET);
        tag_name_with_closer.push_str(tag_name);
        tag_name_with_closer.push(TAG_CLOSED_BRACKET_CHAR);

        tag_name_with_closer = tag_name_with_closer.to_lowercase();

        tracing::debug!(
            "parse_element_text_block: crafted closing tag: {}",
            tag_name_with_closer
        );

        let tag_name_with_closer_len = tag_name_with_closer.len(); // tag_name> we want the length of this

        while let Some(next) = acc.peek_next() {
            tracing::debug!("parse_element_text_block: saw chracter: {}", next);

            if next != TAG_OPEN_BRACKET {
                continue;
            }

            if TAG_OPEN_BRACKET == next {
                acc.unpeek_next();

                let tag_closing = acc.vpeek_at(0, tag_name_with_closer_len).unwrap();
                tracing::debug!(
                    "parse_element_text_block: checking if closing tag: '{}' against expected closer '{}'",
                    tag_closing,
                    tag_name_with_closer
                );

                if tag_closing.to_lowercase() == tag_name_with_closer {
                    tracing::debug!(
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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

        tracing::debug!(
            "parse_doc_type: kickstart parser after taking part: {:?}",
            acc.take()
        );

        let mut collected_tag = false;
        let mut collected_attrs = false;

        while let Some(next) = acc.peek_next() {
            tracing::debug!("parse_doc_type: saw chracter: {}", next);

            if !next.chars().all(char::is_alphabetic) {
                if next != TAG_CLOSED_BRACKET && collected_attrs && collected_tag {
                    return Err(ParsingTagError::TagWithUnexpectedEnding(String::from(
                        acc.take().unwrap(),
                    )));
                }

                if next == TAG_CLOSED_BRACKET && collected_attrs && collected_tag {
                    tracing::debug!("parse_doc_type: completed doctype: {:?}", acc.peek(1));

                    elem.closed = true;
                    return Ok(ParserDirective::Void(elem));
                }

                acc.unpeek_next();

                let (tag_text, (tag_start, tag_end)) = acc.take_positional().unwrap();
                tracing::debug!(
                    "parse_doc_type: generates tagname with collected: {} ({}..{})",
                    tag_text,
                    tag_start,
                    tag_end
                );

                if !collected_tag {
                    elem.tag.replace(MarkupTags::DocType);
                    elem.end_range = Some(tag_end);
                    elem.start_range = Some(tag_start);

                    tracing::debug!("parse_doc_type: generates tagname: {:?}", elem.tag);

                    collected_tag = true;
                }

                self.collect_space(acc)?;

                tracing::debug!(
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

                tracing::debug!(
                    "parse_doc_type: collected attributes if any: {:?}",
                    acc.peek(1)
                );
            }
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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
            tracing::debug!("parse_xml_elem: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            if next == QUESTION_MARK && acc.peek(1).unwrap() == TAG_CLOSED_BRACKET {
                tracing::debug!("parse_xml_elem: collect tag name: {:?}", next);

                acc.peek_next();
                acc.take();

                return Ok(ParserDirective::Void(elem));
            }

            if !collected_tag_name {
                tracing::debug!("parse_xml_elem: collect tag name: {:?}", next);
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

                tracing::debug!("parse_xml_elem: generates tagname: {:?}", elem.tag);

                acc.peek_next();

                collected_tag_name = true;
            }

            if next.chars().all(char::is_whitespace) && !collected_attrs {
                tracing::debug!("parse_xml_elem: collect attributes: {:?}", elem.tag);

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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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
            tracing::debug!("parse_elem: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            if next != TAG_CLOSED_BRACKET && !next.chars().all(char::is_whitespace) {
                tracing::error!(
                    "parse_elem: expected either space or '>' indicating end of start tag: current scan {:?}",
                    acc.scan()
                );

                acc.unpeek_next();

                // check if its self closing tag
                tracing::error!(
                    "parse_elem: kickstart self closing scan logic {:?}",
                    acc.vpeek_at(0, 2)
                );

                if acc.vpeek_at(0, 2).unwrap() == SELF_TAG_CLOSED_BRACKET {
                    tracing::error!("parse_elem: found self closing tag {:?}", acc.scan());

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

                    tracing::debug!("parse_elem: generates self closing tagname: {:?}", elem.tag);

                    acc.peek_next_by(2);

                    tracing::debug!("parse_elem: collecting closing text: {:?}", acc.scan());

                    acc.take();

                    tracing::debug!("parse_elem: next scan peek: {:?}", acc.peek(1));

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

                tracing::debug!("parse_elem generates tagname: {:?}", elem.tag);

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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn collect_space(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        while let Some(next) = acc.peek_next() {
            tracing::debug!("collect_space: start seen token: {:?}", next);

            if next.chars().any(|t| SPACE_CHARS.contains(&t)) {
                tracing::debug!("collect_space: consuming token: {:?}", next);
                continue;
            }

            tracing::debug!("collect_space: non-space token: {:?}", next);

            // move backwartds
            acc.unpeek_next();

            // take the space
            acc.take();

            tracing::debug!(
                "collect_space: non-space token after consumed: {:?}",
                acc.peek(1)
            );

            return Ok(());
        }
        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn is_valid_attribute_value_token<'a>(&self, token: &'a str) -> bool {
        tracing::debug!("Checking if valid attribute value token: {:?}", token);
        token.chars().any(|t| {
            t.is_alphanumeric()
                || t.is_ascii_alphanumeric()
                || VALID_ATTRIBUTE_VALUE_CHARS.contains(&t)
                || VALID_ATTRIBUTE_NAMING_CHARS.contains(&t)
        })
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn dequote_str<'a>(&self, text: &'a str) -> &'a str {
        let text_len = text.len();
        tracing::debug!("dequote: text: {:?} with len: {}", text, text_len);
        if (text.starts_with(SINGLE_QUOTE_STR) || text.starts_with(DOUBLE_QUOTE_STR))
            && (text.ends_with(SINGLE_QUOTE_STR) || text.ends_with(DOUBLE_QUOTE_STR))
        {
            return &text[1..(text_len - 1)];
        }
        text
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn is_valid_attribute_name<'a>(&self, token: &'a str) -> bool {
        token.chars().any(|t| {
            t.is_alphanumeric()
                || t.is_ascii_alphanumeric()
                || VALID_ATTRIBUTE_NAMING_CHARS.contains(&t)
        })
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn collect_attribute_value_alphaneumerics(&self, acc: &mut Accumulator) -> ParsingResult<()> {
        let starter = acc.peek(1).unwrap();
        tracing::debug!(
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
            tracing::debug!(
                "collect_attribute_value_alphaneumerics: scanning token {:?}: starter={:?}, closer={:?}, is_alpha={:?}, is_char={:?}",
                next,
                starter,
                starter_closer,
                is_alphaneumerics_starter,
                is_indent_starter,
            );

            if is_alphaneumerics_starter && next == TAG_CLOSED_BRACKET {
                tracing::debug!(
                    "collect_attribute_value_alphaneumerics: found close bracket {:?}",
                    next
                );

                // move backwartds
                acc.unpeek_next();

                return Ok(());
            }

            if is_alphaneumerics_starter && next.chars().all(char::is_whitespace) {
                tracing::debug!(
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
                tracing::debug!(
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
                tracing::debug!(
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
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

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn parse_elem_attribute<'c, 'd>(
        &self,
        acc: &mut Accumulator<'c>,
        stack: &mut Stack<'d>,
    ) -> ParsingResult<()>
    where
        'c: 'd,
    {
        tracing::debug!("parse_elem_attribute: begin from: {:?}", acc.peek(1));
        self.collect_space(acc)?;

        while let Some(next) = acc.peek_next() {
            tracing::debug!("parse_elem_attribute: seen next token: {:?}", next);

            if self.is_valid_attribute_name(next) {
                continue;
            }

            // are we dealing with single attributes
            if next.chars().all(char::is_whitespace)
                || next == ATTRIBUTE_EQUAL_SIGN
                || next == TAG_CLOSED_BRACKET
            {
                tracing::debug!(
                    "parse_elem_attribute: checking ready value collection: {:?}",
                    next
                );

                if next.chars().all(char::is_whitespace) || next == TAG_CLOSED_BRACKET {
                    tracing::debug!("parse_elem_attribute: actually space or < (tag closer), so collect valueless attribute: {:?}", acc.scan());
                    acc.unpeek_next();

                    let attribute_name = acc.take().unwrap();

                    tracing::debug!(
                        "parse_elem_attribute: collected attribute name: {:?}",
                        attribute_name
                    );

                    if attribute_name.len() != 0 {
                        stack.add_attr(attribute_name, "");
                    }

                    return Ok(());
                }

                if next == ATTRIBUTE_EQUAL_SIGN {
                    tracing::debug!("parse_elem_attribute: actually equal sign, so start collecting value: {:?}", next);

                    acc.unpeek_next();

                    let attr_name = acc.take().unwrap();

                    tracing::debug!(
                        "parse_elem_attribute: collected attribute name: {:?}",
                        attr_name
                    );

                    acc.peek_next();
                    acc.take();

                    tracing::debug!(
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

                    tracing::debug!(
                        "parse_elem_attribute: after validating to next scan: {:?} -> {:?}",
                        acc.peek(1),
                        acc.peek(2)
                    );

                    // collect value
                    self.collect_attribute_value_alphaneumerics(acc)?;

                    let attr_value = self.dequote_str(acc.take().unwrap());

                    tracing::debug!(
                        "parse_elem_attribute: collected attribute value: {:?}",
                        attr_value
                    );

                    match acc.peek(1) {
                        Some(token) => {
                            tracing::debug!(
                                "parse_elem_attribute: check next token if end of attr value: {:?}",
                                token
                            );
                            if !token.chars().all(char::is_whitespace)
                                && token != TAG_CLOSED_BRACKET
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

                    tracing::debug!("parse_elem_attribute: done with no failure");

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
        acc.peek_next_by(2);

        tracing::debug!("parse_closing_tag: consuming token: {:?}", acc.take());

        while let Some(next) = acc.peek_next() {
            tracing::debug!("parse_closing_tag: saw chracter: {}", next);

            if self.is_valid_tag_name_token(next) {
                continue;
            }

            // move backwartds
            if next.chars().all(char::is_whitespace) {
                tracing::debug!("parse_closing_tag: seen space: {:?}", next);

                acc.unpeek_next();
                self.collect_space(acc)?;

                continue;
            }

            if next != TAG_CLOSED_BRACKET {
                tracing::debug!("parse_closing_tag: invalid token: {}", next);

                return Err(ParsingTagError::TagWithUnexpectedEnding(String::from(
                    acc.take().unwrap(),
                )));
            }

            acc.unpeek_next();

            let (tag_text, tag_positioning) = acc.take_positional().unwrap();
            let closing_tag = MarkupTags::from_str(tag_text)?;
            tracing::debug!(
                "parse_closing_tag: found closer for: {:?} with next token: {:?}",
                closing_tag,
                acc.peek(1)
            );

            acc.peek_next();

            tracing::debug!("parse_closing_tag: consume token : {:?}", tag_text);

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
                    attrs: vec![("lang", "en")],
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
                    attrs: vec![("lang", "en")],
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

        tracing::debug!("Result: {:?}", result);

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
                    attrs: vec![("lang", "en")],
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
            ParsingResult::Err(ParsingTagError::InvalidHTMLContent(_))
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

        tracing::debug!("Result: {:?}", result);

        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap().get_tags();
        tracing::debug!("ParsedTags: {:?}", parsed);

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

        tracing::debug!("Result: {:?}", result);

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

        tracing::debug!("Result: {:?}", result);

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
            ("jail", "{{some of other vaulue}}")
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
            ("jail", "{some of other vaulue}")
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

    #[traced_test]
    #[test]
    fn test_html_can_handle_wikipedia_page() {
        let parser = HTMLParser::default();

        let data = wrap_in_document_fragment_container(String::from(
            r#"
                <!DOCTYPE html>
                <html class="client-nojs" lang="en" dir="ltr">

                <head>
                    <meta charset="UTF-8" />
                    <title>Wikipedia, the free encyclopedia</title>
                    <script>
                        document.documentElement.className = "client-js";
                        RLCONF = {
                            "wgBreakFrames": !1,
                            "wgSeparatorTransformTable": ["", ""],
                            "wgDigitTransformTable": ["", ""],
                            "wgDefaultDateFormat": "dmy",
                            "wgMonthNames": ["", "January", "February", "March", "April", "May", "June", "July", "August",
                                "September", "October", "November", "December"
                            ],
                            "wgRequestId": "X@DCQgpAEJkABA9bDMoAAABX",
                            "wgCSPNonce": !1,
                            "wgCanonicalNamespace": "",
                            "wgCanonicalSpecialPageName": !1,
                            "wgNamespaceNumber": 0,
                            "wgPageName": "Main_Page",
                            "wgTitle": "Main Page",
                            "wgCurRevisionId": 987965326,
                            "wgRevisionId": 987965326,
                            "wgArticleId": 15580374,
                            "wgIsArticle": !0,
                            "wgIsRedirect": !1,
                            "wgAction": "view",
                            "wgUserName": null,
                            "wgUserGroups": ["*"],
                            "wgCategories": [],
                            "wgPageContentLanguage": "en",
                            "wgPageContentModel": "wikitext",
                            "wgRelevantPageName": "Main_Page",
                            "wgRelevantArticleId": 15580374,
                            "wgIsProbablyEditable": !1,
                            "wgRelevantPageIsProbablyEditable": !1,
                            "wgRestrictionEdit": ["sysop"],
                            "wgRestrictionMove": ["sysop"],
                            "wgIsMainPage": !0,
                            "wgMediaViewerOnClick": !0,
                            "wgMediaViewerEnabledByDefault": !0,
                            "wgPopupsReferencePreviews": !1,
                            "wgPopupsConflictsWithNavPopupGadget": !1,
                            "wgPopupsConflictsWithRefTooltipsGadget": !0,
                            "wgVisualEditor": {
                                "pageLanguageCode": "en",
                                "pageLanguageDir": "ltr",
                                "pageVariantFallbacks": "en"
                            },
                            "wgMFDisplayWikibaseDescriptions": {
                                "search": !0,
                                "nearby": !0,
                                "watchlist": !0,
                                "tagline": !1
                            },
                            "wgWMESchemaEditAttemptStepOversample": !1,
                            "wgULSCurrentAutonym": "English",
                            "wgNoticeProject": "wikipedia",
                            "wgCentralAuthMobileDomain": !1,
                            "wgEditSubmitButtonLabelPublish": !0,
                            "wgULSPosition": "interlanguage",
                            "wgWikibaseItemId": "Q5296"
                        };
                        RLSTATE = {
                            "ext.globalCssJs.user.styles": "ready",
                            "site.styles": "ready",
                            "noscript": "ready",
                            "user.styles": "ready",
                            "ext.globalCssJs.user": "ready",
                            "user": "ready",
                            "user.options": "loading",
                            "ext.categoryTree.styles": "ready",
                            "skins.vector.styles.legacy": "ready",
                            "ext.visualEditor.desktopArticleTarget.noscript": "ready",
                            "ext.uls.interlanguage": "ready",
                            "ext.wikimediaBadges": "ready"
                        };
                        RLPAGEMODULES = ["ext.categoryTree",
                            "site", "mediawiki.page.ready", "skins.vector.legacy.js", "ext.gadget.ReferenceTooltips",
                            "ext.gadget.charinsert", "ext.gadget.extra-toolbar-buttons", "ext.gadget.refToolbar",
                            "ext.gadget.switcher", "ext.centralauth.centralautologin", "mmv.head", "mmv.bootstrap.autostart",
                            "ext.popups", "ext.visualEditor.desktopArticleTarget.init", "ext.visualEditor.targetLoader",
                            "ext.eventLogging", "ext.wikimediaEvents", "ext.navigationTiming", "ext.uls.interface",
                            "ext.cx.eventlogging.campaigns", "ext.centralNotice.geoIP", "ext.centralNotice.startUp"
                        ];
                    </script>
                    <script>
                        (RLQ = window.RLQ || []).push(function () {
                            mw.loader.implement("user.options@1hzgi", function ($, jQuery, require, module) {
                                /*@nomin*/
                                mw.user.tokens.set({
                                    "patrolToken": "+\\",
                                    "watchToken": "+\\",
                                    "csrfToken": "+\\"
                                });
                            });
                        });
                    </script>
                    <link rel="stylesheet"
                        href="/w/load.php?lang=en&amp;modules=ext.categoryTree.styles%7Cext.uls.interlanguage%7Cext.visualEditor.desktopArticleTarget.noscript%7Cext.wikimediaBadges%7Cskins.vector.styles.legacy&amp;only=styles&amp;skin=vector" />
                    <script async="" src="/w/load.php?lang=en&amp;modules=startup&amp;only=scripts&amp;raw=1&amp;skin=vector"></script>
                    <meta name="ResourceLoaderDynamicStyles" content="" />
                    <link rel="stylesheet" href="/w/load.php?lang=en&amp;modules=site.styles&amp;only=styles&amp;skin=vector" />
                    <meta name="generator" content="MediaWiki 1.36.0-wmf.22" />
                    <meta name="referrer" content="origin" />
                    <meta name="referrer" content="origin-when-crossorigin" />
                    <meta name="referrer" content="origin-when-cross-origin" />
                    <meta property="og:image"
                        content="https://upload.wikimedia.org/wikipedia/en/f/f2/Chien_Courant_Italien_A_Poil_Ras.jpg" />
                    <link rel="preconnect" href="//upload.wikimedia.org" />
                    <link rel="alternate" media="only screen and (max-width: 720px)" href="//en.m.wikipedia.org/wiki/Main_Page" />
                    <link rel="alternate" type="application/atom+xml" title="Wikipedia picture of the day feed"
                        href="/w/api.php?action=featuredfeed&amp;feed=potd&amp;feedformat=atom" />
                    <link rel="alternate" type="application/atom+xml" title="Wikipedia featured articles feed"
                        href="/w/api.php?action=featuredfeed&amp;feed=featured&amp;feedformat=atom" />
                    <link rel="alternate" type="application/atom+xml" title="Wikipedia &quot;On this day...&quot; feed"
                        href="/w/api.php?action=featuredfeed&amp;feed=onthisday&amp;feedformat=atom" />
                    <link rel="apple-touch-icon" href="/static/apple-touch/wikipedia.png" />
                    <link rel="shortcut icon" href="/static/favicon/wikipedia.ico" />
                    <link rel="search" type="application/opensearchdescription+xml" href="/w/opensearch_desc.php"
                        title="Wikipedia (en)" />
                    <link rel="EditURI" type="application/rsd+xml" href="//en.wikipedia.org/w/api.php?action=rsd" />
                    <link rel="license" href="//creativecommons.org/licenses/by-sa/3.0/" />
                    <link rel="canonical" href="https://en.wikipedia.org/wiki/Main_Page" />
                    <link rel="dns-prefetch" href="//login.wikimedia.org" />
                    <link rel="dns-prefetch" href="//meta.wikimedia.org" />
                </head>

                <body
                    class="mediawiki ltr sitedir-ltr mw-hide-empty-elt ns-0 ns-subject page-Main_Page rootpage-Main_Page skin-vector action-view skin-vector-legacy">
                    <div id="mw-page-base" class="noprint"></div>
                    <div id="mw-head-base" class="noprint"></div>
                    <div id="content" class="mw-body" role="main">
                        <a id="top"></a>
                        <div id="siteNotice" class="mw-body-content">
                            <!-- CentralNotice -->
                        </div>
                        <div class="mw-indicators mw-body-content">
                        </div>
                        <h1 id="firstHeading" class="firstHeading" lang="en">Main Page</h1>
                        <div id="bodyContent" class="mw-body-content">
                            <div id="siteSub" class="noprint">From Wikipedia, the free encyclopedia</div>
                            <div id="contentSub"></div>
                            <div id="contentSub2"></div>

                            <div id="jump-to-nav"></div>
                            <a class="mw-jump-link" href="\#mw-head">Jump to navigation</a>
                            <a class="mw-jump-link" href="\#searchInput">Jump to search</a>
                            <div id="mw-content-text" lang="en" dir="ltr" class="mw-content-ltr">
                                <div class="mw-parser-output">
                                    <style data-mw-deduplicate="TemplateStyles:r979842806">
                                        .mw-parser-output #mp-topbanner {
                                            clear: both;
                                            position: relative;
                                            box-sizing: border-box;
                                            width: 100%;
                                            margin-top: 1.2em;
                                            min-width: 47em;
                                            border-color: #ddd;
                                            background-color: #f9f9f9;
                                            white-space: nowrap
                                        }

                                        .mw-parser-output .mp-bordered,
                                        .mw-parser-output .mp-h2,
                                        body.skin-timeless .mw-parser-output .mp-h2 {
                                            border-width: 1px;
                                            border-style: solid
                                        }

                                        .mw-parser-output #mp-topbanner,
                                        .mw-parser-output .mp-h2,
                                        .mw-parser-output #mp-left {
                                            color: #000
                                        }

                                        .mw-parser-output #mp-welcomecount {
                                            margin: 0.4em;
                                            width: 22em;
                                            text-align: center
                                        }

                                        .mw-parser-output #mp-welcome {
                                            font-size: 162%;
                                            padding: 0.1em
                                        }

                                        .mw-parser-output #mp-free {
                                            font-size: 95%
                                        }

                                        .mw-parser-output #articlecount {
                                            font-size: 85%
                                        }

                                        .mw-parser-output #mp-portals {
                                            position: absolute;
                                            right: -1em;
                                            top: 50%;
                                            margin-top: -2.4em;
                                            width: 38%;
                                            min-width: 25em;
                                            font-size: 95%
                                        }

                                        .mw-parser-output #mp-portals li {
                                            position: absolute;
                                            left: 0;
                                            top: 0
                                        }

                                        .mw-parser-output #mp-portals .portal-hmid {
                                            left: 33%
                                        }

                                        .mw-parser-output #mp-portals .portal-hright {
                                            left: 66%
                                        }

                                        .mw-parser-output #mp-portals .portal-vmid {
                                            top: 1.6em
                                        }

                                        .mw-parser-output #mp-portals .portal-vbot {
                                            top: 3.2em
                                        }

                                        .mw-parser-output .portal-hright.portal-vbot {
                                            font-weight: bold
                                        }

                                        .mw-parser-output #mp-banner {
                                            margin-top: 4px;
                                            padding: 0.5em;
                                            background-color: #fffaf5;
                                            border-color: #f2e0ce
                                        }

                                        .mw-parser-output .mp-h2,
                                        body.skin-timeless .mw-parser-output .mp-h2 {
                                            margin: 0.5em;
                                            padding: 0.2em 0.4em;
                                            font-size: 120%;
                                            font-weight: bold;
                                            font-family: inherit
                                        }

                                        .mw-parser-output h2.mp-h2::after {
                                            border: none
                                        }

                                        .mw-parser-output .mp-later {
                                            font-size: 85%;
                                            font-weight: normal
                                        }

                                        .mw-parser-output #mp-upper {
                                            width: 100%;
                                            margin-top: 4px;
                                            margin-bottom: 0;
                                            border-spacing: 0;
                                            border-collapse: separate
                                        }

                                        .mw-parser-output #mp-upper .mid-table {
                                            border-color: transparent
                                        }

                                        .mw-parser-output #mp-left {
                                            width: 55%;
                                            border-color: #cef2e0;
                                            background: #f5fffa
                                        }

                                        .mw-parser-output #mp-right {
                                            width: 45%;
                                            border-color: #cedff2;
                                            background: #f5faff
                                        }

                                        .mw-parser-output #mp-left,
                                        .mw-parser-output #mp-right {
                                            padding: 0;
                                            vertical-align: top
                                        }

                                        .mw-parser-output #mp-left .mp-h2 {
                                            background: #cef2e0;
                                            border-color: #a3bfb1
                                        }

                                        .mw-parser-output #mp-right .mp-h2 {
                                            background: #cedff2;
                                            border-color: #a3b0bf
                                        }

                                        .mw-parser-output #mp-tfa,
                                        .mw-parser-output #mp-dyk,
                                        .mw-parser-output #mp-itn,
                                        .mw-parser-output #mp-otd,
                                        .mw-parser-output #mp-other-lower>div {
                                            padding: 0.1em 0.6em
                                        }

                                        .mw-parser-output #mp-dyk-h2,
                                        .mw-parser-output #mp-otd-h2 {
                                            clear: both
                                        }

                                        .mw-parser-output #mp-middle {
                                            margin-top: 4px;
                                            border-color: #f2cedd;
                                            background: #fff5fa
                                        }

                                        .mw-parser-output #mp-middle,
                                        .mw-parser-output #mp-lower,
                                        .mw-parser-output #mp-other-lower {
                                            overflow: auto
                                        }

                                        .mw-parser-output #mp-tfl-h2 {
                                            background: #f2cedd;
                                            border-color: #bfa3af
                                        }

                                        .mw-parser-output #mp-tfl {
                                            padding: 0.3em 0.7em
                                        }

                                        .mw-parser-output #mp-lower {
                                            margin-top: 4px;
                                            border-color: #ddcef2;
                                            background: #faf5ff
                                        }

                                        .mw-parser-output #mp-tfp-h2 {
                                            background: #ddcef2;
                                            border-color: #afa3bf
                                        }

                                        .mw-parser-output #mp-tfp {
                                            margin: 0.1em 0.4em 0.6em
                                        }

                                        .mw-parser-output #mp-other-lower {
                                            padding: 0;
                                            border-color: #e2e2e2;
                                            margin-top: 4px
                                        }

                                        .mw-parser-output #mp-dyk,
                                        .mw-parser-output #mp-otd,
                                        .mw-parser-output #mp-other-lower {
                                            padding-bottom: 0.5em
                                        }

                                        .mw-parser-output #mp-other-lower .mp-h2 {
                                            background: #eee;
                                            border-color: #ddd;
                                            color: #222
                                        }

                                        @media(max-width:875px) {
                                            body.skin--responsive .mw-parser-output #mp-welcomecount {
                                                width: auto
                                            }

                                            body.skin--responsive .mw-parser-output #mp-topbanner {
                                                min-width: 0;
                                                white-space: normal
                                            }

                                            body.skin--responsive .mw-parser-output #mp-portals {
                                                display: block;
                                                position: static;
                                                width: auto;
                                                min-width: 0;
                                                text-align: center;
                                                border-top: 1px solid #ddd;
                                                padding: 0.4em 0;
                                                margin: 0 0.4em
                                            }

                                            body.skin--responsive .mw-parser-output #mp-portals li {
                                                position: static;
                                                display: inline;
                                                padding: 0 5px
                                            }

                                            body.skin--responsive .mw-parser-output #mp-topbanner .portal-hright {
                                                white-space: nowrap
                                            }

                                            body.skin--responsive .mw-parser-output table,
                                            body.skin--responsive .mw-parser-output tr,
                                            body.skin--responsive .mw-parser-output td,
                                            body.skin--responsive .mw-parser-output tbody {
                                                display: block !important;
                                                width: 100% !important;
                                                box-sizing: border-box
                                            }

                                            body.skin--responsive .mw-parser-output #mp-tfp tr:first-child td:first-child a {
                                                text-align: center;
                                                display: table;
                                                margin: 0 auto
                                            }
                                        }
                                    </style>
                                    <div id="mp-topbanner" class="mp-bordered">
                                        <div id="mp-welcomecount">
                                            <div id="mp-welcome">Welcome to <a href="/wiki/Wikipedia" title="Wikipedia">Wikipedia</a>,
                                            </div>
                                            <div id="mp-free">the <a href="/wiki/Free_content" title="Free content">free</a> <a
                                                    href="/wiki/Encyclopedia" title="Encyclopedia">encyclopedia</a> that <a
                                                    href="/wiki/Help:Introduction_to_Wikipedia"
                                                    title="Help:Introduction to Wikipedia">anyone can edit</a>.</div>
                                            <div id="articlecount"><a href="/wiki/Special:Statistics"
                                                    title="Special:Statistics">6,212,504</a> articles in <a
                                                    href="/wiki/English_language" title="English language">English</a></div>
                                        </div>
                                        <ul id="mp-portals">
                                            <li><a href="/wiki/Portal:The_arts" title="Portal:The arts">The arts</a></li>
                                            <li class="portal-vmid"><a href="/wiki/Portal:Biography"
                                                    title="Portal:Biography">Biography</a></li>
                                            <li class="portal-vbot"><a href="/wiki/Portal:Geography"
                                                    title="Portal:Geography">Geography</a></li>
                                            <li class="portal-hmid"><a href="/wiki/Portal:History" title="Portal:History">History</a>
                                            </li>
                                            <li class="portal-hmid portal-vmid"><a href="/wiki/Portal:Mathematics"
                                                    title="Portal:Mathematics">Mathematics</a></li>
                                            <li class="portal-hmid portal-vbot"><a href="/wiki/Portal:Science"
                                                    title="Portal:Science">Science</a></li>
                                            <li class="portal-hright"><a href="/wiki/Portal:Society" title="Portal:Society">Society</a>
                                            </li>
                                            <li class="portal-hright portal-vmid"><a href="/wiki/Portal:Technology"
                                                    title="Portal:Technology">Technology</a></li>
                                            <li class="portal-hright portal-vbot"><a href="/wiki/Wikipedia:Contents/Portals"
                                                    title="Wikipedia:Contents/Portals">All portals</a></li>
                                        </ul>
                                    </div>
                                    <table role="presentation" id="mp-upper">
                                        <tbody>
                                            <tr>
                                                <td id="mp-left" class="MainPageBG mp-bordered">
                                                    <h2 id="mp-tfa-h2" class="mp-h2"><span
                                                            id="From_today.27s_featured_article"></span><span class="mw-headline"
                                                            id="From_today's_featured_article">From today's featured article</span></h2>
                                                    <div id="mp-tfa">
                                                        <div id="mp-tfa-img" style="float: left; margin: 0.5em 0.9em 0.4em 0em;">
                                                            <div class="thumbinner mp-thumb"
                                                                style="background: transparent; border: none; padding: 0; max-width: 118px;">
                                                                <a href="/wiki/File:Sergo_Orjonikidze.jpg" class="image"
                                                                    title="Sergo Ordzhonikidze"><img alt="Sergo Ordzhonikidze"
                                                                        src="//upload.wikimedia.org/wikipedia/commons/thumb/6/6f/Sergo_Orjonikidze.jpg/118px-Sergo_Orjonikidze.jpg"
                                                                        decoding="async" width="118" height="165"
                                                                        srcset="//upload.wikimedia.org/wikipedia/commons/thumb/6/6f/Sergo_Orjonikidze.jpg/177px-Sergo_Orjonikidze.jpg 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/6/6f/Sergo_Orjonikidze.jpg/236px-Sergo_Orjonikidze.jpg 2x"
                                                                        data-file-width="2270" data-file-height="3170" /></a></div>
                                                        </div>
                                                        <p><b><a href="/wiki/Sergo_Ordzhonikidze" title="Sergo Ordzhonikidze">Sergo
                                                                    Ordzhonikidze</a></b> (1886–1937) was a <a href="/wiki/Bolsheviks"
                                                                title="Bolsheviks">Bolshevik</a> and <a href="/wiki/Soviet_Union"
                                                                title="Soviet Union">Soviet</a> politician from <a
                                                                href="/wiki/Georgia_within_the_Russian_Empire"
                                                                title="Georgia within the Russian Empire">Georgia</a>. Joining the
                                                            Bolsheviks at a young age, he became an important figure and was arrested
                                                            repeatedly. After the Bolsheviks came to power in 1917, he oversaw the
                                                            invasions <a href="/wiki/Red_Army_invasion_of_Azerbaijan"
                                                                title="Red Army invasion of Azerbaijan">of Azerbaijan</a>, <a
                                                                href="/wiki/Red_Army_invasion_of_Armenia"
                                                                title="Red Army invasion of Armenia">of Armenia</a>, and <a
                                                                href="/wiki/Red_Army_invasion_of_Georgia"
                                                                title="Red Army invasion of Georgia">of Georgia</a>. He backed their
                                                            union into the <a
                                                                href="/wiki/Transcaucasian_Socialist_Federative_Soviet_Republic"
                                                                title="Transcaucasian Socialist Federative Soviet Republic">Transcaucasian
                                                                Socialist Federative Soviet Republic</a> in 1922, one of the original
                                                            Soviet republics, and served as its <a href="/wiki/Secretary_(title)"
                                                                title="Secretary (title)">first secretary</a> until 1926. He then
                                                            oversaw Soviet economic production and led a massive overhaul; he
                                                            implemented <a
                                                                href="/wiki/Five-year_plans_for_the_national_economy_of_the_Soviet_Union"
                                                                title="Five-year plans for the national economy of the Soviet Union">five-year
                                                                plans</a>, helped create the <a href="/wiki/Stakhanovite_movement"
                                                                title="Stakhanovite movement">Stakhanovite movement</a> and was named to
                                                            the <a href="/wiki/Politburo" title="Politburo">Politburo</a>. He was
                                                            reluctant to join the campaign against so-called <a
                                                                href="/wiki/Wrecking_(Soviet_Union)"
                                                                title="Wrecking (Soviet Union)">wreckers</a> and saboteurs in the early
                                                            1930s, causing friction with <a href="/wiki/Joseph_Stalin"
                                                                title="Joseph Stalin">Joseph Stalin</a>. Before a meeting where he was
                                                            expected to denounce workers, Ordzhonikidze shot himself. He was
                                                            posthumously honoured, and several towns and cities in the Soviet Union were
                                                            named after him. (<b><a href="/wiki/Sergo_Ordzhonikidze"
                                                                    title="Sergo Ordzhonikidze">Full&#160;article...</a></b>)
                                                        </p>
                                                        <div class="tfa-recent" style="text-align: right;">
                                                            Recently featured: <div class="hlist hlist-separated inline">
                                                                <ul>
                                                                    <li><i><a href="/wiki/Oxenfree" title="Oxenfree">Oxenfree</a></i>
                                                                    </li>
                                                                    <li><i><a href="/wiki/Achelousaurus"
                                                                                title="Achelousaurus">Achelousaurus</a></i></li>
                                                                    <li><a href="/wiki/Mount_Takahe" title="Mount Takahe">Mount
                                                                            Takahe</a></li>
                                                                </ul>
                                                            </div>
                                                        </div>
                                                        <div class="tfa-footer hlist hlist-separated noprint"
                                                            style="text-align: right;">
                                                            <ul>
                                                                <li><b><a href="/wiki/Wikipedia:Today%27s_featured_article/December_2020"
                                                                            title="Wikipedia:Today&#39;s featured article/December 2020">Archive</a></b>
                                                                </li>
                                                                <li><b><a href="https://lists.wikimedia.org/mailman/listinfo/daily-article-l"
                                                                            class="extiw" title="mail:daily-article-l">By email</a></b>
                                                                </li>
                                                                <li><b><a href="/wiki/Wikipedia:Featured_articles"
                                                                            title="Wikipedia:Featured articles">More featured
                                                                            articles</a></b></li>
                                                            </ul>
                                                        </div>
                                                    </div>
                                                    <h2 id="mp-dyk-h2" class="mp-h2"><span class="mw-headline" id="Did_you_know_...">Did
                                                            you know&#160;...</span></h2>
                                                    <div id="mp-dyk">
                                                        <div class="dyk-img" style="float: right; margin-left: 0.5em;">
                                                            <div class="thumbinner mp-thumb"
                                                                style="background: transparent; border: none; padding: 0; max-width: 154px;">
                                                                <a href="/wiki/File:Chien_Courant_Italien_A_Poil_Ras.jpg" class="image"
                                                                    title="Short-haired Segugio Italiano"><img
                                                                        alt="Short-haired Segugio Italiano"
                                                                        src="//upload.wikimedia.org/wikipedia/en/thumb/f/f2/Chien_Courant_Italien_A_Poil_Ras.jpg/154px-Chien_Courant_Italien_A_Poil_Ras.jpg"
                                                                        decoding="async" width="154" height="127"
                                                                        srcset="//upload.wikimedia.org/wikipedia/en/thumb/f/f2/Chien_Courant_Italien_A_Poil_Ras.jpg/231px-Chien_Courant_Italien_A_Poil_Ras.jpg 1.5x, //upload.wikimedia.org/wikipedia/en/thumb/f/f2/Chien_Courant_Italien_A_Poil_Ras.jpg/308px-Chien_Courant_Italien_A_Poil_Ras.jpg 2x"
                                                                        data-file-width="513" data-file-height="423" /></a>
                                                                <div class="thumbcaption"
                                                                    style="padding: 0.25em 0; word-wrap: break-word;">Short-haired
                                                                    Segugio Italiano</div>
                                                            </div>
                                                        </div>
                                                        <ul>
                                                            <li>... that the <b><a href="/wiki/Segugio_Italiano"
                                                                        title="Segugio Italiano">Segugio Italiano</a></b> <i>(example
                                                                    pictured)</i> was highly prized during the <a
                                                                    href="/wiki/Italian_Renaissance" title="Italian Renaissance">Italian
                                                                    Renaissance</a>, being used in elaborate hunts with large numbers of
                                                                servants and followers mounted on horseback?</li>
                                                            <li>... that <b><a href="/wiki/Jenna_Ellis" title="Jenna Ellis">Jenna
                                                                        Ellis</a></b> was a stern critic of <a href="/wiki/Donald_Trump"
                                                                    title="Donald Trump">Donald Trump</a> before she became his legal
                                                                adviser?</li>
                                                            <li>... that Australian pop band <b><a href="/wiki/Autumn_(Australian_band)"
                                                                        title="Autumn (Australian band)">Autumn</a></b> was one of four
                                                                artists co-credited with a number-one hit in the Australian charts on
                                                                31&#160;October&#160;1970, each with a version of the song "<a
                                                                    href="/wiki/Yellow_River_(song)" title="Yellow River (song)">Yellow
                                                                    River</a>"?</li>
                                                            <li>... that <a href="/wiki/Bernie_Sanders" title="Bernie Sanders">Bernie
                                                                    Sanders</a> <b><a href="/wiki/1981_Burlington_mayoral_election"
                                                                        title="1981 Burlington mayoral election">won the
                                                                        election</a></b> to become Mayor of <a
                                                                    href="/wiki/Burlington,_Vermont"
                                                                    title="Burlington, Vermont">Burlington, Vermont</a>, by ten votes in
                                                                1981?</li>
                                                            <li>... that during the <a
                                                                    href="/wiki/Occupation_of_Poland_(1939%E2%80%931945)"
                                                                    title="Occupation of Poland (1939–1945)">Nazi occupation of
                                                                    Poland</a>, <b><a href="/wiki/Halina_Kwiatkowska"
                                                                        title="Halina Kwiatkowska">Halina Kwiatkowska</a></b> acted in
                                                                an underground theatre alongside <a href="/wiki/Pope_John_Paul_II"
                                                                    title="Pope John Paul II">a future pope</a>?</li>
                                                            <li>... that in his book <i><b><a
                                                                            href="/wiki/Thematic_Origins_of_Scientific_Thought"
                                                                            title="Thematic Origins of Scientific Thought">Thematic
                                                                            Origins of Scientific Thought: Kepler to
                                                                            Einstein</a></b></i>, <a href="/wiki/Gerald_Holton"
                                                                    title="Gerald Holton">Gerald Holton</a> argues that philosophy from
                                                                <i><a href="/wiki/Either/Or" title="Either/Or">Either/Or</a></i>
                                                                influenced <a href="/wiki/Niels_Bohr" title="Niels Bohr">Niels
                                                                    Bohr</a>'s concept of <a href="/wiki/Complementarity_(physics)"
                                                                    title="Complementarity (physics)">complementarity</a>?</li>
                                                            <li>... that <b><a href="/wiki/Robert_Hammerstiel"
                                                                        title="Robert Hammerstiel">Robert Hammerstiel</a></b> wrapped
                                                                Vienna's Ringturm tower in a painting showing stations of human life in
                                                                simplified and brightly coloured figures?</li>
                                                            <li>... that the marine worm <i><b><a href="/wiki/Neanthes_arenaceodentata"
                                                                            title="Neanthes arenaceodentata">Neanthes
                                                                            arenaceodentata</a></b></i> is both an environmental monitor
                                                                and a caring father?</li>
                                                        </ul>
                                                        <div class="dyk-footer hlist hlist-separated noprint"
                                                            style="margin-top: 0.5em; text-align: right;">
                                                            <ul>
                                                                <li><b><a href="/wiki/Wikipedia:Recent_additions"
                                                                            title="Wikipedia:Recent additions">Archive</a></b></li>
                                                                <li><b><a href="/wiki/Help:Your_first_article"
                                                                            title="Help:Your first article">Start a new article</a></b>
                                                                </li>
                                                                <li><b><a href="/wiki/Template_talk:Did_you_know"
                                                                            title="Template talk:Did you know">Nominate an
                                                                            article</a></b></li>
                                                            </ul>
                                                        </div>
                                                    </div>
                                                </td>
                                                <td class="mp-bordered mid-table">
                                                </td>
                                                <td id="mp-right" class="MainPageBG mp-bordered">
                                                    <h2 id="mp-itn-h2" class="mp-h2"><span class="mw-headline" id="In_the_news">In the
                                                            news</span></h2>
                                                    <div id="mp-itn">
                                                        <style data-mw-deduplicate="TemplateStyles:r976327607">
                                                            .mw-parser-output .itn-special {
                                                                margin: 0px 1px 4px 1px;
                                                                padding: 5px;
                                                                text-align: center;
                                                                border-style: solid;
                                                                background: #fafcfe;
                                                                border: 1px solid #a3b0bf
                                                            }
                                                        </style>
                                                        <div class="itn-special hlist hlist-separated">
                                                            <dl>
                                                                <dt><a href="/wiki/COVID-19_pandemic" title="COVID-19 pandemic"><span
                                                                            class="nowrap">COVID-19</span> pandemic</a></dt>
                                                                <dd><a href="/wiki/Coronavirus_disease_2019"
                                                                        title="Coronavirus disease 2019">Disease</a></dd>
                                                                <dd><a href="/wiki/Severe_acute_respiratory_syndrome_coronavirus_2"
                                                                        title="Severe acute respiratory syndrome coronavirus 2">Virus</a>
                                                                </dd>
                                                                <dd><a href="/wiki/COVID-19_pandemic_by_country_and_territory"
                                                                        title="COVID-19 pandemic by country and territory"><span
                                                                            class="nowrap">By location</span></a></dd>
                                                                <dd><a href="/wiki/Impact_of_the_COVID-19_pandemic"
                                                                        title="Impact of the COVID-19 pandemic">Impact</a></dd>
                                                                <dd><a href="/wiki/COVID-19_vaccine"
                                                                        title="COVID-19 vaccine">Vaccines</a></dd>
                                                                <dd><a href="/wiki/Portal:Coronavirus_disease_2019"
                                                                        title="Portal:Coronavirus disease 2019">Portal</a></dd>
                                                            </dl>
                                                        </div>
                                                        <div role="figure" class="itn-img"
                                                            style="float: right; margin-left: 0.5em; margin-top: 0.2em;">
                                                            <div class="thumbinner mp-thumb"
                                                                style="background: transparent; border: none; padding: 0; max-width: 157px;">
                                                                <a href="/wiki/File:Change-5.png" class="image"
                                                                    title="Chang&#39;e 5"><img alt="Chang&#39;e 5"
                                                                        src="//upload.wikimedia.org/wikipedia/commons/thumb/c/c4/Change-5.png/157px-Change-5.png"
                                                                        decoding="async" width="157" height="125"
                                                                        srcset="//upload.wikimedia.org/wikipedia/commons/thumb/c/c4/Change-5.png/236px-Change-5.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/c/c4/Change-5.png/314px-Change-5.png 2x"
                                                                        data-file-width="682" data-file-height="543" /></a>
                                                                <div class="thumbcaption"
                                                                    style="padding: 0.25em 0; word-wrap: break-word; text-align: left;">
                                                                    Chang'e 5</div>
                                                            </div>
                                                        </div>
                                                        <ul>
                                                            <li>The <b><a href="/wiki/Chang%27e_5" title="Chang&#39;e 5">Chang'e
                                                                        5</a></b> <i>(illustration shown)</i> <a
                                                                    href="/wiki/Sample-return_mission"
                                                                    title="Sample-return mission">sample-return mission</a> returns <a
                                                                    href="/wiki/Moon_rock" title="Moon rock">lunar material</a> to
                                                                Earth.</li>
                                                            <li><a href="/wiki/Nana_Akufo-Addo" title="Nana Akufo-Addo">Nana
                                                                    Akufo-Addo</a> is <b><a href="/wiki/2020_Ghanaian_general_election"
                                                                        title="2020 Ghanaian general election">re-elected</a></b> for a
                                                                second term as <a href="/wiki/President_of_Ghana"
                                                                    title="President of Ghana">President of Ghana</a>.</li>
                                                            <li>In motorsport, <a href="/wiki/S%C3%A9bastien_Ogier"
                                                                    title="Sébastien Ogier">Sébastien Ogier</a> and <a
                                                                    href="/wiki/Julien_Ingrassia" title="Julien Ingrassia">Julien
                                                                    Ingrassia</a> win <b><a href="/wiki/2020_World_Rally_Championship"
                                                                        title="2020 World Rally Championship">the World Rally
                                                                        Championship</a></b>, while <a href="/wiki/Hyundai_Motorsport"
                                                                    title="Hyundai Motorsport">Hyundai</a> win the manufacturers' title.
                                                            </li>
                                                            <li><i><b><a href="/wiki/Hayabusa2" title="Hayabusa2">Hayabusa2</a></b></i>
                                                                successfully returns samples collected from asteroid <a
                                                                    href="/wiki/162173_Ryugu" title="162173 Ryugu">162173 Ryugu</a> to
                                                                Earth.</li>
                                                        </ul>
                                                        <div class="itn-footer" style="margin-top: 0.5em;">
                                                            <div><b><a href="/wiki/Portal:Current_events"
                                                                        title="Portal:Current events">Ongoing</a></b>: <div
                                                                    class="hlist hlist-separated inline">
                                                                    <ul>
                                                                        <li><a href="/wiki/2020_Indian_farmers%27_protest"
                                                                                title="2020 Indian farmers&#39; protest">Indian farmers'
                                                                                protest</a></li>
                                                                        <li><a href="/wiki/Tigray_conflict"
                                                                                title="Tigray conflict">Tigray conflict</a></li>
                                                                    </ul>
                                                                </div>
                                                            </div>
                                                            <div><b><a href="/wiki/Deaths_in_2020" title="Deaths in 2020">Recent
                                                                        deaths</a></b>&#58; <div class="hlist hlist-separated inline">
                                                                    <ul>
                                                                        <li><a href="/wiki/%C3%92scar_Ribas_Reig"
                                                                                title="Òscar Ribas Reig">Òscar Ribas Reig</a></li>
                                                                        <li><a href="/wiki/Jerry_Relph" title="Jerry Relph">Jerry
                                                                                Relph</a></li>
                                                                        <li><a href="/wiki/R._N._Shetty" title="R. N. Shetty">R. N.
                                                                                Shetty</a></li>
                                                                        <li><a href="/wiki/Michael_Jeffery"
                                                                                title="Michael Jeffery">Michael Jeffery</a></li>
                                                                        <li><a href="/wiki/John_Barnard_Jenkins"
                                                                                title="John Barnard Jenkins">John Barnard Jenkins</a>
                                                                        </li>
                                                                        <li><a href="/wiki/Flavio_Cotti" title="Flavio Cotti">Flavio
                                                                                Cotti</a></li>
                                                                    </ul>
                                                                </div>
                                                            </div>
                                                        </div>
                                                        <div class="itn-footer hlist hlist-separated noprint"
                                                            style="text-align: right;">
                                                            <ul>
                                                                <li><b><a href="/wiki/Wikipedia:In_the_news/Candidates"
                                                                            title="Wikipedia:In the news/Candidates">Nominate an
                                                                            article</a></b></li>
                                                            </ul>
                                                        </div>
                                                    </div>
                                                    <h2 id="mp-otd-h2" class="mp-h2"><span class="mw-headline" id="On_this_day">On this
                                                            day</span></h2>
                                                    <div id="mp-otd">
                                                        <p><b><a href="/wiki/December_21" title="December 21">December 21</a></b>: <b><a
                                                                    href="/wiki/December_solstice" title="December solstice">December
                                                                    solstice</a></b> (10:03&#160;<a
                                                                href="/wiki/Coordinated_Universal_Time"
                                                                title="Coordinated Universal Time">UTC</a>, 2020); <b><a
                                                                    href="/wiki/Yule" title="Yule">Yule</a></b> begins
                                                        </p>
                                                        <div style="float:right;margin-left:0.5em;" id="mp-otd-img">
                                                            <div style="float:right;margin-left:0.5em;" id="mp-otd-img">
                                                                <div class="thumbinner mp-thumb"
                                                                    style="background: transparent; border: none; padding: 0; max-width: 165px;">
                                                                    <a href="/wiki/File:US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg"
                                                                        class="image" title="Grumman F-14 Tomcat"><img
                                                                            alt="Grumman F-14 Tomcat"
                                                                            src="//upload.wikimedia.org/wikipedia/commons/thumb/f/f7/US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg/165px-US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg"
                                                                            decoding="async" width="165" height="119"
                                                                            srcset="//upload.wikimedia.org/wikipedia/commons/thumb/f/f7/US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg/248px-US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/f/f7/US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg/330px-US_Navy_051105-F-5480T-005_An_F-14D_Tomcat_conducts_a_mission_over_the_Persian_Gulf-region.jpg 2x"
                                                                            data-file-width="1796" data-file-height="1297" /></a>
                                                                    <div class="thumbcaption"
                                                                        style="padding: 0.25em 0; word-wrap: break-word;">Grumman F-14
                                                                        Tomcat</div>
                                                                </div>
                                                            </div>
                                                        </div>
                                                        <ul>
                                                            <li><a href="/wiki/1620" title="1620">1620</a> – The <a
                                                                    href="/wiki/Pilgrims_(Plymouth_Colony)"
                                                                    title="Pilgrims (Plymouth Colony)">Pilgrims</a> aboard the <i><a
                                                                        href="/wiki/Mayflower" title="Mayflower">Mayflower</a></i>
                                                                landed at present-day <a href="/wiki/Plymouth,_Massachusetts"
                                                                    title="Plymouth, Massachusetts">Plymouth, Massachusetts</a>,
                                                                establishing the <b><a href="/wiki/Plymouth_Colony"
                                                                        title="Plymouth Colony">Plymouth Colony</a></b>.</li>
                                                            <li><a href="/wiki/1872" title="1872">1872</a> – <b><a
                                                                        href="/wiki/HMS_Challenger_(1858)"
                                                                        title="HMS Challenger (1858)">HMS&#160;<i>Challenger</i></a></b>
                                                                departed <a href="/wiki/Portsmouth" title="Portsmouth">Portsmouth</a> on
                                                                <a href="/wiki/Challenger_expedition" title="Challenger expedition">a
                                                                    scientific expedition</a> that laid the foundations of <a
                                                                    href="/wiki/Oceanography" title="Oceanography">oceanography</a>.
                                                            </li>
                                                            <li><a href="/wiki/1937" title="1937">1937</a> – <i><b><a
                                                                            href="/wiki/Snow_White_and_the_Seven_Dwarfs_(1937_film)"
                                                                            title="Snow White and the Seven Dwarfs (1937 film)">Snow
                                                                            White and the Seven Dwarfs</a></b></i>, the first
                                                                full-length <a href="/wiki/Cel" title="Cel">cel</a>-animated feature in
                                                                film history, premiered at the <a href="/wiki/Carthay_Circle_Theatre"
                                                                    title="Carthay Circle Theatre">Carthay Circle Theatre</a> in Los
                                                                Angeles.</li>
                                                            <li><a href="/wiki/1970" title="1970">1970</a> – The <b><a
                                                                        href="/wiki/Grumman_F-14_Tomcat"
                                                                        title="Grumman F-14 Tomcat">Grumman <span
                                                                            class="nowrap">F-14</span> Tomcat</a></b> <i>(example
                                                                    pictured)</i>, the primary <a href="/wiki/Fighter_aircraft"
                                                                    title="Fighter aircraft">fighter aircraft</a> of the U.S. Navy for
                                                                nearly 30&#160;years, made its first flight.</li>
                                                            <li><a href="/wiki/1995" title="1995">1995</a> – In accordance with the <a
                                                                    href="/wiki/Oslo_II_Accord" title="Oslo II Accord">Oslo&#160;II
                                                                    Accord</a>, Israeli troops withdrew from <b><a
                                                                        href="/wiki/Bethlehem" title="Bethlehem">Bethlehem</a></b> in
                                                                preparation for the transfer of control to the <a
                                                                    href="/wiki/Palestinian_National_Authority"
                                                                    title="Palestinian National Authority">Palestinian National
                                                                    Authority</a>.</li>
                                                        </ul>
                                                        <div class="hlist hlist-separated" style="margin-top: 0.5em;">
                                                            <ul>
                                                                <li><b><a href="/wiki/Ali_ibn_Muhammad_ibn_al-Walid"
                                                                            title="Ali ibn Muhammad ibn al-Walid">Ali ibn Muhammad ibn
                                                                            al-Walid</a></b> (<abbr title="died">d.</abbr>&#160;1215)
                                                                </li>
                                                                <li><b><a href="/wiki/Maud_Gonne" title="Maud Gonne">Maud Gonne</a></b>
                                                                    (<abbr title="born">b.</abbr>&#160;1866)</li>
                                                                <li><b><a href="/wiki/Iris_Cummings" title="Iris Cummings">Iris
                                                                            Cummings</a></b> (<abbr title="born">b.</abbr>&#160;1920)
                                                                </li>
                                                            </ul>
                                                        </div>
                                                        <div style="margin-top: 0.5em;">
                                                            More anniversaries: <div class="hlist hlist-separated inline nowraplinks">
                                                                <ul>
                                                                    <li><a href="/wiki/December_20" title="December 20">December 20</a>
                                                                    </li>
                                                                    <li><b><a href="/wiki/December_21" title="December 21">December
                                                                                21</a></b></li>
                                                                    <li><a href="/wiki/December_22" title="December 22">December 22</a>
                                                                    </li>
                                                                </ul>
                                                            </div>
                                                        </div>
                                                        <div class="otd-footer hlist hlist-separated noprint"
                                                            style="text-align: right;">
                                                            <ul>
                                                                <li><b><a href="/wiki/Wikipedia:Selected_anniversaries/December"
                                                                            title="Wikipedia:Selected anniversaries/December">Archive</a></b>
                                                                </li>
                                                                <li><b><a href="https://lists.wikimedia.org/mailman/listinfo/daily-article-l"
                                                                            class="extiw" title="mail:daily-article-l">By email</a></b>
                                                                </li>
                                                                <li><b><a href="/wiki/List_of_days_of_the_year"
                                                                            title="List of days of the year">List of days of the
                                                                            year</a></b></li>
                                                            </ul>
                                                        </div>
                                                    </div>
                                                </td>
                                            </tr>
                                        </tbody>
                                    </table>
                                    <div id="mp-middle" class="MainPageBG mp-bordered">
                                        <div id="mp-center">
                                            <h2 id="mp-tfl-h2" class="mp-h2"><span id="From_today.27s_featured_list"></span><span
                                                    class="mw-headline" id="From_today's_featured_list">From today's featured
                                                    list</span></h2>
                                            <div id="mp-tfl">
                                                <div id="mp-tfl-img" style="float:right;margin:0.5em 0 0.4em 0.9em;">
                                                    <div class="thumbinner mp-thumb"
                                                        style="background: transparent; border: none; padding: 0; max-width: 114px;">
                                                        <a href="/wiki/File:Procyon_lotor_(raccoon).jpg" class="image"
                                                            title="Raccoon"><img alt="Raccoon"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/8/8b/Procyon_lotor_%28raccoon%29.jpg/114px-Procyon_lotor_%28raccoon%29.jpg"
                                                                decoding="async" width="114" height="171"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/8/8b/Procyon_lotor_%28raccoon%29.jpg/171px-Procyon_lotor_%28raccoon%29.jpg 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/8/8b/Procyon_lotor_%28raccoon%29.jpg/228px-Procyon_lotor_%28raccoon%29.jpg 2x"
                                                                data-file-width="400" data-file-height="600" /></a>
                                                        <div class="thumbcaption" style="padding: 0.25em 0; word-wrap: break-word;">
                                                            Raccoon</div>
                                                    </div>
                                                </div>
                                                <p><b><a href="/wiki/List_of_procyonids" title="List of procyonids">Procyonids</a></b>
                                                    are members of <a href="/wiki/Procyonidae" title="Procyonidae">Procyonidae</a>, a <a
                                                        href="/wiki/Family_(biology)" title="Family (biology)">family</a> of <a
                                                        href="/wiki/Mammal" title="Mammal">mammals</a> in the <a
                                                        href="/wiki/Order_(biology)" title="Order (biology)">order</a> <a
                                                        href="/wiki/Carnivora" title="Carnivora">Carnivora</a>. The family includes <a
                                                        href="/wiki/Procyon_(genus)" title="Procyon (genus)">raccoons</a>, <a
                                                        href="/wiki/Coati" title="Coati">coatis</a>, <a href="/wiki/Bassaricyon"
                                                        title="Bassaricyon">olingos</a>, <a href="/wiki/Kinkajou"
                                                        title="Kinkajou">kinkajous</a>, <a href="/wiki/Ring-tailed_cat"
                                                        title="Ring-tailed cat">ring-tailed cats</a>, and <a href="/wiki/Cacomistle"
                                                        title="Cacomistle">cacomistles</a>, and many other <a
                                                        href="/wiki/Neontology#Extant_taxa_versus_extinct_taxa"
                                                        title="Neontology">extant</a> and <a href="/wiki/Extinction"
                                                        title="Extinction">extinct</a> mammals. They are native to North and South
                                                    America, though the <a href="/wiki/Raccoon" title="Raccoon">common raccoon</a>
                                                    <i>(pictured)</i> has been introduced to Europe, western Asia, and Japan. Procyonid
                                                    habitats are generally forests, though some are found in shrublands and grasslands
                                                    as well. The fourteen species of Procyonidae are split into six <a
                                                        href="/wiki/Genus" title="Genus">genera</a>, which are not currently grouped
                                                    into named <a href="/wiki/Cladistics" title="Cladistics">clades</a>. Procyonidae is
                                                    believed to have diverged as a separate family within Carnivora around
                                                    22.6&#160;million years ago. Procyonidae includes forty extinct species placed in
                                                    the six extant and nineteen extinct genera, though due to ongoing research and
                                                    discoveries the exact number and categorization is not fixed. (<b><a
                                                            href="/wiki/List_of_procyonids"
                                                            title="List of procyonids">Full&#160;list...</a></b>)
                                                </p>
                                                <div class="tfl-recent" style="text-align: right;">
                                                    Recently featured: <div class="hlist inline">
                                                        <ul>
                                                            <li><a href="/wiki/2018_in_cue_sports" title="2018 in cue sports">2018 in
                                                                    cue sports</a></li>
                                                            <li><a href="/wiki/List_of_national_forests_of_the_United_States"
                                                                    title="List of national forests of the United States">National
                                                                    forests of the United States</a></li>
                                                            <li><a href="/wiki/Orson_Welles_filmography"
                                                                    title="Orson Welles filmography">Orson Welles filmography</a></li>
                                                        </ul>
                                                    </div>
                                                </div>
                                                <div class="tfl-footer hlist noprint" style="text-align: right;">
                                                    <ul>
                                                        <li><b><a href="/wiki/Wikipedia:Today%27s_featured_list/December_2020"
                                                                    title="Wikipedia:Today&#39;s featured list/December 2020">Archive</a></b>
                                                        </li>
                                                        <li><b><a href="/wiki/Wikipedia:Featured_lists"
                                                                    title="Wikipedia:Featured lists">More featured lists</a></b></li>
                                                    </ul>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                    <div id="mp-lower" class="MainPageBG mp-bordered">
                                        <div id="mp-bottom">
                                            <h2 id="mp-tfp-h2" class="mp-h2"><span id="Today.27s_featured_picture"></span><span
                                                    class="mw-headline" id="Today's_featured_picture">Today's featured picture</span>
                                            </h2>
                                            <div id="mp-tfp">
                                                <table role="presentation"
                                                    style="margin:0 3px 3px; width:100%; box-sizing:border-box; text-align:left; background-color:transparent; border-collapse:collapse;">
                                                    <tbody>
                                                        <tr>
                                                            <td style="padding:0 0.9em 0 0; width:300px;"><a
                                                                    href="/wiki/File:The_artist_Anne_Vallayer-Coster.jpg" class="image"
                                                                    title="Anne Vallayer-Coster"><img alt="Anne Vallayer-Coster"
                                                                        src="//upload.wikimedia.org/wikipedia/commons/thumb/f/fb/The_artist_Anne_Vallayer-Coster.jpg/300px-The_artist_Anne_Vallayer-Coster.jpg"
                                                                        decoding="async" width="300" height="379"
                                                                        srcset="//upload.wikimedia.org/wikipedia/commons/thumb/f/fb/The_artist_Anne_Vallayer-Coster.jpg/450px-The_artist_Anne_Vallayer-Coster.jpg 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/f/fb/The_artist_Anne_Vallayer-Coster.jpg/600px-The_artist_Anne_Vallayer-Coster.jpg 2x"
                                                                        data-file-width="2375" data-file-height="3000" /></a>
                                                            </td>
                                                            <td style="padding:0 6px 0 0">
                                                                <p><b><a href="/wiki/Anne_Vallayer-Coster"
                                                                            title="Anne Vallayer-Coster">Anne Vallayer-Coster</a></b>
                                                                    (21&#160;December&#160;1744&#160;– 28&#160;February&#160;1818) was
                                                                    an 18th-century French painter, best known for her <a
                                                                        href="/wiki/Still_life" title="Still life">still-life</a> works.
                                                                    When she was 26, she was admitted to the prestigious <a
                                                                        href="/wiki/Acad%C3%A9mie_royale_de_peinture_et_de_sculpture"
                                                                        title="Académie royale de peinture et de sculpture">Académie
                                                                        royale de peinture et de sculpture</a>; Vallayer-Coster was one
                                                                    of only four women to be accepted into the Académie before the <a
                                                                        href="/wiki/French_Revolution" title="French Revolution">French
                                                                        Revolution</a>, in a period when men dominated the profession.
                                                                    By 1780, she had come under the patronage of <a
                                                                        href="/wiki/Marie_Antoinette" title="Marie Antoinette">Marie
                                                                        Antoinette</a>, after which her career flourished. This 1783
                                                                    oil-on-canvas portrait, showing Vallayer-Coster at work, is by the
                                                                    Swedish painter <a href="/wiki/Alexander_Roslin"
                                                                        title="Alexander Roslin">Alexander Roslin</a>. The painting is
                                                                    in the collection of the <a href="/wiki/Crocker_Art_Museum"
                                                                        title="Crocker Art Museum">Crocker Art Museum</a> in <a
                                                                        href="/wiki/Sacramento,_California"
                                                                        title="Sacramento, California">Sacramento, California</a>.
                                                                </p>
                                                                <p style="text-align:left;"><small>Painting credit: <a
                                                                            href="/wiki/Alexander_Roslin"
                                                                            title="Alexander Roslin">Alexander Roslin</a></small></p>
                                                                <div class="potd-recent" style="text-align:right;">
                                                                    Recently featured: <div class="hlist hlist-separated inline">
                                                                        <ul>
                                                                            <li><a href="/wiki/Template:POTD/2020-12-20"
                                                                                    title="Template:POTD/2020-12-20">Violet-backed
                                                                                    starling</a></li>
                                                                            <li><a href="/wiki/Template:POTD/2020-12-19"
                                                                                    title="Template:POTD/2020-12-19">NGC&#160;6357</a>
                                                                            </li>
                                                                            <li><a href="/wiki/Template:POTD/2020-12-18"
                                                                                    title="Template:POTD/2020-12-18">English Gothic
                                                                                    architecture</a></li>
                                                                        </ul>
                                                                    </div>
                                                                </div>
                                                                <div class="potd-footer hlist hlist-separated noprint"
                                                                    style="text-align:right;">
                                                                    <ul>
                                                                        <li><b><a href="/wiki/Wikipedia:Picture_of_the_day/Archive"
                                                                                    title="Wikipedia:Picture of the day/Archive">Archive</a></b>
                                                                        </li>
                                                                        <li><b><a href="/wiki/Wikipedia:Featured_pictures"
                                                                                    title="Wikipedia:Featured pictures">More featured
                                                                                    pictures</a></b></li>
                                                                    </ul>
                                                                </div>
                                                            </td>
                                                        </tr>
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    </div>
                                    <div id="mp-other-lower" class="mp-bordered">
                                        <h2 id="mp-other" class="mp-h2"><span class="mw-headline" id="Other_areas_of_Wikipedia">Other
                                                areas of Wikipedia</span></h2>
                                        <div id="mp-other-content">
                                            <ul>
                                                <li><b><a href="/wiki/Wikipedia:Community_portal"
                                                            title="Wikipedia:Community portal">Community portal</a></b> – Bulletin
                                                    board, projects, resources and activities covering a wide range of Wikipedia areas.
                                                </li>
                                                <li><b><a href="/wiki/Wikipedia:Help_desk" title="Wikipedia:Help desk">Help desk</a></b>
                                                    – Ask questions about using Wikipedia.</li>
                                                <li><b><a href="/wiki/Wikipedia:Local_Embassy" title="Wikipedia:Local Embassy">Local
                                                            embassy</a></b> – For Wikipedia-related communication in languages other
                                                    than English.</li>
                                                <li><b><a href="/wiki/Wikipedia:Reference_desk"
                                                            title="Wikipedia:Reference desk">Reference desk</a></b> – Serving as virtual
                                                    librarians, Wikipedia volunteers tackle your questions on a wide range of subjects.
                                                </li>
                                                <li><b><a href="/wiki/Wikipedia:News" title="Wikipedia:News">Site news</a></b> –
                                                    Announcements, updates, articles and press releases on Wikipedia and the Wikimedia
                                                    Foundation.</li>
                                                <li><b><a href="/wiki/Wikipedia:Village_pump" title="Wikipedia:Village pump">Village
                                                            pump</a></b> – For discussions about Wikipedia itself, including areas for
                                                    technical issues and policies.</li>
                                            </ul>
                                        </div>
                                        <h2 id="mp-sister" class="mp-h2"><span id="Wikipedia.27s_sister_projects"></span><span
                                                class="mw-headline" id="Wikipedia's_sister_projects">Wikipedia's sister projects</span>
                                        </h2>
                                        <div id="mp-sister-content">
                                            <style data-mw-deduplicate="TemplateStyles:r961577119">
                                                .mw-parser-output #sister-projects-list {
                                                    text-align: left;
                                                    background: transparent;
                                                    margin: 1px;
                                                    display: flex;
                                                    flex-wrap: wrap
                                                }

                                                .mw-parser-output #sister-projects-list>div {
                                                    width: 33%;
                                                    min-width: 20em;
                                                    white-space: nowrap;
                                                    display: inline-block;
                                                    flex: 1 0 25%
                                                }

                                                .mw-parser-output #sister-projects-list>div>div {
                                                    display: inline-block;
                                                    vertical-align: middle;
                                                    padding: 6px 4px
                                                }

                                                .mw-parser-output #sister-projects-list>div>div:first-child {
                                                    min-width: 50px;
                                                    text-align: center
                                                }
                                            </style>
                                            <p>Wikipedia is hosted by the <a href="/wiki/Wikimedia_Foundation"
                                                    title="Wikimedia Foundation">Wikimedia Foundation</a>, a non-profit organization
                                                that also hosts a range of other <a
                                                    href="https://wikimediafoundation.org/our-work/wikimedia-projects/" class="extiw"
                                                    title="foundationsite:our-work/wikimedia-projects/">projects</a>:
                                            </p>
                                            <div id="sister-projects-list" class="layout plainlinks">
                                                <div>
                                                    <div> <a href="https://commons.wikimedia.org/wiki/" title="Commons"><img
                                                                alt="Commons"
                                                                src="//upload.wikimedia.org/wikipedia/en/thumb/4/4a/Commons-logo.svg/31px-Commons-logo.svg.png"
                                                                decoding="async" width="31" height="42"
                                                                srcset="//upload.wikimedia.org/wikipedia/en/thumb/4/4a/Commons-logo.svg/47px-Commons-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/en/thumb/4/4a/Commons-logo.svg/62px-Commons-logo.svg.png 2x"
                                                                data-file-width="1024" data-file-height="1376" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://commons.wikimedia.org/">Commons</a></b> <br /> Free media
                                                        repository </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://www.mediawiki.org/wiki/" title="MediaWiki"><img
                                                                alt="MediaWiki"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Mediawiki-logo.png/35px-Mediawiki-logo.png"
                                                                decoding="async" width="35" height="26"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Mediawiki-logo.png/53px-Mediawiki-logo.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/3/3d/Mediawiki-logo.png/70px-Mediawiki-logo.png 2x"
                                                                data-file-width="135" data-file-height="102" /></a> </div>
                                                    <div> <b><a class="external text" href="https://mediawiki.org/">MediaWiki</a></b>
                                                        <br /> Wiki software development </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://meta.wikimedia.org/wiki/" title="Meta-Wiki"><img
                                                                alt="Meta-Wiki"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/7/75/Wikimedia_Community_Logo.svg/35px-Wikimedia_Community_Logo.svg.png"
                                                                decoding="async" width="35" height="35"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/7/75/Wikimedia_Community_Logo.svg/53px-Wikimedia_Community_Logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/7/75/Wikimedia_Community_Logo.svg/70px-Wikimedia_Community_Logo.svg.png 2x"
                                                                data-file-width="900" data-file-height="900" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://meta.wikimedia.org/">Meta-Wiki</a></b> <br /> Wikimedia
                                                        project coordination </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikibooks.org/wiki/" title="Wikibooks"><img
                                                                alt="Wikibooks"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikibooks-logo.svg/35px-Wikibooks-logo.svg.png"
                                                                decoding="async" width="35" height="35"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikibooks-logo.svg/53px-Wikibooks-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikibooks-logo.svg/70px-Wikibooks-logo.svg.png 2x"
                                                                data-file-width="300" data-file-height="300" /></a> </div>
                                                    <div> <b><a class="external text" href="https://en.wikibooks.org/">Wikibooks</a></b>
                                                        <br /> Free textbooks and manuals </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://www.wikidata.org/wiki/" title="Wikidata"><img alt="Wikidata"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/f/ff/Wikidata-logo.svg/47px-Wikidata-logo.svg.png"
                                                                decoding="async" width="47" height="26"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/f/ff/Wikidata-logo.svg/71px-Wikidata-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/f/ff/Wikidata-logo.svg/94px-Wikidata-logo.svg.png 2x"
                                                                data-file-width="1050" data-file-height="590" /></a> </div>
                                                    <div> <b><a class="external text" href="https://www.wikidata.org/">Wikidata</a></b>
                                                        <br /> Free knowledge base </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikinews.org/wiki/" title="Wikinews"><img alt="Wikinews"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/2/24/Wikinews-logo.svg/51px-Wikinews-logo.svg.png"
                                                                decoding="async" width="51" height="28"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/2/24/Wikinews-logo.svg/77px-Wikinews-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/2/24/Wikinews-logo.svg/102px-Wikinews-logo.svg.png 2x"
                                                                data-file-width="759" data-file-height="415" /></a> </div>
                                                    <div> <b><a class="external text" href="https://en.wikinews.org/">Wikinews</a></b>
                                                        <br /> Free-content news </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikiquote.org/wiki/" title="Wikiquote"><img
                                                                alt="Wikiquote"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikiquote-logo.svg/35px-Wikiquote-logo.svg.png"
                                                                decoding="async" width="35" height="41"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikiquote-logo.svg/53px-Wikiquote-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/f/fa/Wikiquote-logo.svg/70px-Wikiquote-logo.svg.png 2x"
                                                                data-file-width="300" data-file-height="355" /></a> </div>
                                                    <div> <b><a class="external text" href="https://en.wikiquote.org/">Wikiquote</a></b>
                                                        <br /> Collection of quotations </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikisource.org/wiki/" title="Wikisource"><img
                                                                alt="Wikisource"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/4/4c/Wikisource-logo.svg/35px-Wikisource-logo.svg.png"
                                                                decoding="async" width="35" height="37"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/4/4c/Wikisource-logo.svg/53px-Wikisource-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/4/4c/Wikisource-logo.svg/70px-Wikisource-logo.svg.png 2x"
                                                                data-file-width="410" data-file-height="430" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://en.wikisource.org/">Wikisource</a></b> <br /> Free-content
                                                        library </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://species.wikimedia.org/wiki/" title="Wikispecies"><img
                                                                alt="Wikispecies"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/d/df/Wikispecies-logo.svg/35px-Wikispecies-logo.svg.png"
                                                                decoding="async" width="35" height="41"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/d/df/Wikispecies-logo.svg/53px-Wikispecies-logo.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/d/df/Wikispecies-logo.svg/70px-Wikispecies-logo.svg.png 2x"
                                                                data-file-width="941" data-file-height="1103" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://species.wikimedia.org/">Wikispecies</a></b> <br />
                                                        Directory of species </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikiversity.org/wiki/" title="Wikiversity"><img
                                                                alt="Wikiversity"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/0/0b/Wikiversity_logo_2017.svg/41px-Wikiversity_logo_2017.svg.png"
                                                                decoding="async" width="41" height="34"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/0/0b/Wikiversity_logo_2017.svg/62px-Wikiversity_logo_2017.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/0/0b/Wikiversity_logo_2017.svg/82px-Wikiversity_logo_2017.svg.png 2x"
                                                                data-file-width="626" data-file-height="512" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://en.wikiversity.org/">Wikiversity</a></b> <br /> Free
                                                        learning resources </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wikivoyage.org/wiki/" title="Wikivoyage"><img
                                                                alt="Wikivoyage"
                                                                src="//upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Wikivoyage-Logo-v3-icon.svg/35px-Wikivoyage-Logo-v3-icon.svg.png"
                                                                decoding="async" width="35" height="35"
                                                                srcset="//upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Wikivoyage-Logo-v3-icon.svg/53px-Wikivoyage-Logo-v3-icon.svg.png 1.5x, //upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Wikivoyage-Logo-v3-icon.svg/70px-Wikivoyage-Logo-v3-icon.svg.png 2x"
                                                                data-file-width="193" data-file-height="193" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://en.wikivoyage.org/">Wikivoyage</a></b> <br /> Free travel
                                                        guide </div>
                                                </div>
                                                <div>
                                                    <div> <a href="https://en.wiktionary.org/wiki/" title="Wiktionary"><img
                                                                alt="Wiktionary"
                                                                src="//upload.wikimedia.org/wikipedia/en/thumb/0/06/Wiktionary-logo-v2.svg/35px-Wiktionary-logo-v2.svg.png"
                                                                decoding="async" width="35" height="35"
                                                                srcset="//upload.wikimedia.org/wikipedia/en/thumb/0/06/Wiktionary-logo-v2.svg/53px-Wiktionary-logo-v2.svg.png 1.5x, //upload.wikimedia.org/wikipedia/en/thumb/0/06/Wiktionary-logo-v2.svg/70px-Wiktionary-logo-v2.svg.png 2x"
                                                                data-file-width="391" data-file-height="391" /></a> </div>
                                                    <div> <b><a class="external text"
                                                                href="https://en.wiktionary.org/">Wiktionary</a></b> <br /> Dictionary
                                                        and thesaurus </div>
                                                </div>
                                            </div>
                                        </div>
                                        <h2 id="mp-lang" class="mp-h2"><span class="mw-headline" id="Wikipedia_languages">Wikipedia
                                                languages</span></h2>
                                        <div>
                                            <div id="lang" class="nowraplinks nourlexpansion plainlinks">
                                                <p>This Wikipedia is written in <a href="/wiki/English_language"
                                                        title="English language">English</a>. Started in 2001<span
                                                        style="display:none">&#160;(<span
                                                            class="bday dtstart published updated">2001</span>)</span>, it currently
                                                    contains <a href="/wiki/Special:Statistics" title="Special:Statistics">6,212,504</a>
                                                    articles.&#32;
                                                    Many other Wikipedias are available; some of the largest are listed below.
                                                </p>
                                                <ul>
                                                    <li id="lang-3">More than 1,000,000 articles: <div
                                                            class="hlist hlist-separated inline">
                                                            <ul>
                                                                <li><a class="external text" href="https://ar.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Arabic (ar:)"
                                                                            lang="ar">العربية</span></a></li>
                                                                <li><a class="external text" href="https://de.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="German (de:)"
                                                                            lang="de">Deutsch</span></a></li>
                                                                <li><a class="external text" href="https://es.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Spanish (es:)"
                                                                            lang="es">Español</span></a></li>
                                                                <li><a class="external text" href="https://fr.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="French (fr:)"
                                                                            lang="fr">Français</span></a></li>
                                                                <li><a class="external text" href="https://it.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Italian (it:)"
                                                                            lang="it">Italiano</span></a></li>
                                                                <li><a class="external text" href="https://nl.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Dutch (nl:)"
                                                                            lang="nl">Nederlands</span></a></li>
                                                                <li><a class="external text" href="https://ja.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Japanese (ja:)"
                                                                            lang="ja">日本語</span></a></li>
                                                                <li><a class="external text" href="https://pl.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Polish (pl:)"
                                                                            lang="pl">Polski</span></a></li>
                                                                <li><a class="external text" href="https://pt.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Portuguese (pt:)"
                                                                            lang="pt">Português</span></a></li>
                                                                <li><a class="external text" href="https://ru.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Russian (ru:)"
                                                                            lang="ru">Русский</span></a></li>
                                                                <li><a class="external text" href="https://sv.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Swedish (sv:)"
                                                                            lang="sv">Svenska</span></a></li>
                                                                <li><a class="external text" href="https://uk.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Ukrainian (uk:)"
                                                                            lang="uk">Українська</span></a></li>
                                                                <li><a class="external text" href="https://vi.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Vietnamese (vi:)" lang="vi">Tiếng
                                                                            Việt</span></a></li>
                                                                <li><a class="external text" href="https://zh.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Chinese (zh:)"
                                                                            lang="zh">中文</span></a></li>
                                                            </ul>
                                                        </div>
                                                    </li>
                                                    <li id="lang-2">More than 250,000 articles: <div
                                                            class="hlist hlist-separated inline">
                                                            <ul>
                                                                <li><a class="external text" href="https://id.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Indonesian (id:)" lang="id">Bahasa
                                                                            Indonesia</span></a></li>
                                                                <li><a class="external text" href="https://ms.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Malay (ms:)" lang="ms">Bahasa
                                                                            Melayu</span></a></li>
                                                                <li><a class="external text"
                                                                        href="https://zh-min-nan.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Min Nan Chinese (nan:)"
                                                                            lang="nan">Bân-lâm-gú</span></a></li>
                                                                <li><a class="external text" href="https://bg.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Bulgarian (bg:)"
                                                                            lang="bg">Български</span></a></li>
                                                                <li><a class="external text" href="https://ca.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Catalan (ca:)"
                                                                            lang="ca">Català</span></a></li>
                                                                <li><a class="external text" href="https://cs.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Czech (cs:)"
                                                                            lang="cs">Čeština</span></a></li>
                                                                <li><a class="external text" href="https://da.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Danish (da:)"
                                                                            lang="da">Dansk</span></a></li>
                                                                <li><a class="external text" href="https://eo.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Esperanto (eo:)"
                                                                            lang="eo">Esperanto</span></a></li>
                                                                <li><a class="external text" href="https://eu.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Basque (eu:)"
                                                                            lang="eu">Euskara</span></a></li>
                                                                <li><a class="external text" href="https://fa.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Persian (fa:)"
                                                                            lang="fa">فارسی</span></a>&#8206;</li>
                                                                <li><a class="external text" href="https://he.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Hebrew (he:)"
                                                                            lang="he">עברית</span></a></li>
                                                                <li><a class="external text" href="https://ko.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Korean (ko:)"
                                                                            lang="ko">한국어</span></a></li>
                                                                <li><a class="external text" href="https://hu.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Hungarian (hu:)"
                                                                            lang="hu">Magyar</span></a></li>
                                                                <li><a class="external text" href="https://no.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Norwegian (no:)" lang="no">Norsk
                                                                            Bokmål</span></a></li>
                                                                <li><a class="external text" href="https://ro.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Romanian (ro:)"
                                                                            lang="ro">Română</span></a></li>
                                                                <li><a class="external text" href="https://sr.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Serbian (sr:)"
                                                                            lang="sr">Srpski</span></a></li>
                                                                <li><a class="external text" href="https://sh.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Serbo-Croatian (sh:)"
                                                                            lang="sh">Srpskohrvatski</span></a></li>
                                                                <li><a class="external text" href="https://fi.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Finnish (fi:)"
                                                                            lang="fi">Suomi</span></a></li>
                                                                <li><a class="external text" href="https://tr.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Turkish (tr:)"
                                                                            lang="tr">Türkçe</span></a></li>
                                                            </ul>
                                                        </div>
                                                    </li>
                                                    <li id="lang-1">More than 50,000 articles: <div
                                                            class="hlist hlist-separated inline">
                                                            <ul>
                                                                <li><a class="external text"
                                                                        href="https://ast.wikipedia.org/wiki/"><span class="autonym"
                                                                            title="Asturian (ast:)" lang="ast">Asturianu</span></a></li>
                                                                <li><a class="external text" href="https://bs.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Bosnian (bs:)"
                                                                            lang="bs">Bosanski</span></a></li>
                                                                <li><a class="external text" href="https://et.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Estonian (et:)"
                                                                            lang="et">Eesti</span></a></li>
                                                                <li><a class="external text" href="https://el.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Greek (el:)"
                                                                            lang="el">Ελληνικά</span></a></li>
                                                                <li><a class="external text"
                                                                        href="https://simple.wikipedia.org/wiki/"><span class="autonym"
                                                                            title="Simple English (simple:)" lang="simple">Simple
                                                                            English</span></a></li>
                                                                <li><a class="external text" href="https://gl.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Galician (gl:)"
                                                                            lang="gl">Galego</span></a></li>
                                                                <li><a class="external text" href="https://hr.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Croatian (hr:)"
                                                                            lang="hr">Hrvatski</span></a></li>
                                                                <li><a class="external text" href="https://lv.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Latvian (lv:)"
                                                                            lang="lv">Latviešu</span></a></li>
                                                                <li><a class="external text" href="https://lt.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Lithuanian (lt:)"
                                                                            lang="lt">Lietuvių</span></a></li>
                                                                <li><a class="external text" href="https://ml.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Malayalam (ml:)"
                                                                            lang="ml">മലയാളം</span></a></li>
                                                                <li><a class="external text" href="https://mk.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Macedonian (mk:)"
                                                                            lang="mk">Македонски</span></a></li>
                                                                <li><a class="external text" href="https://nn.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Norwegian Nynorsk (nn:)"
                                                                            lang="nn">Norsk nynorsk</span></a></li>
                                                                <li><a class="external text" href="https://sk.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Slovak (sk:)"
                                                                            lang="sk">Slovenčina</span></a></li>
                                                                <li><a class="external text" href="https://sl.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Slovenian (sl:)"
                                                                            lang="sl">Slovenščina</span></a></li>
                                                                <li><a class="external text" href="https://th.wikipedia.org/wiki/"><span
                                                                            class="autonym" title="Thai (th:)" lang="th">ไทย</span></a>
                                                                </li>
                                                            </ul>
                                                        </div>
                                                    </li>
                                                </ul>
                                            </div>
                                            <div id="metalink" style="text-align:center;" class="plainlinks"><strong><a
                                                        href="https://meta.wikimedia.org/wiki/List_of_Wikipedias" class="extiw"
                                                        title="meta:List of Wikipedias">Complete list of Wikipedias</a></strong></div>
                                        </div>
                                    </div>
                                    <!--
                NewPP limit report
                Parsed by mw1403
                Cached time: 20201221154153
                Cache expiry: 3600
                Dynamic content: true
                Complications: []
                CPU time usage: 0.360 seconds
                Real time usage: 0.483 seconds
                Preprocessor visited node count: 4117/1000000
                Post‐expand include size: 120543/2097152 bytes
                Template argument size: 7597/2097152 bytes
                Highest expansion depth: 21/40
                Expensive parser function count: 17/500
                Unstrip recursion depth: 0/20
                Unstrip post‐expand size: 4875/5000000 bytes
                Lua time usage: 0.083/10.000 seconds
                Lua memory usage: 2108809/52428800 bytes
                Number of Wikibase entities loaded: 0/400
                -->
                                    <!--
                Transclusion expansion time report (%,ms,calls,template)
                100.00%  331.605      1 -total
                 40.05%  132.806      9 Template:Main_page_image
                 33.39%  110.709      1 Wikipedia:Main_Page/Tomorrow
                 23.38%   77.539      9 Template:Str_number/trim
                 21.16%   70.174     27 Template:If_empty
                 18.57%   61.594      2 Template:Main_page_image/TFA
                 16.50%   54.718      1 Wikipedia:Today's_featured_article/December_21,_2020
                 13.09%   43.406      2 Template:In_the_news
                 12.96%   42.970      1 Wikipedia:Selected_anniversaries/December_21
                 12.65%   41.953      1 Template:Did_you_know/Queue/2
                -->

                                    <!-- Saved in parser cache with key enwiki:pcache:idhash:15580374-0!canonical and timestamp 20201221154152 and revision id 987965326. Serialized with JSON.
                 -->
                                </div><noscript><img src="//en.wikipedia.org/wiki/Special:CentralAutoLogin/start?type=1x1" alt=""
                                        title="" width="1" height="1" style="border: none; position: absolute;" /></noscript>
                                <div class="printfooter">Retrieved from "<a dir="ltr"
                                        href="https://en.wikipedia.org/w/index.php?title=Main_Page&amp;oldid=987965326">https://en.wikipedia.org/w/index.php?title=Main_Page&amp;oldid=987965326</a>"
                                </div>
                            </div>
                            <div id="catlinks" class="catlinks catlinks-allhidden" data-mw="interface"></div>
                        </div>
                    </div>
                    <div id='mw-data-after-content'>
                        <div class="read-more-container"></div>
                    </div>

                    <div id="mw-navigation">
                        <h2>Navigation menu</h2>
                        <div id="mw-head">
                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-personal" class="mw-portlet mw-portlet-personal vector-menu" aria-labelledby="p-personal-label"
                                role="navigation">
                                <h3 id="p-personal-label">
                                    <span>Personal tools</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li id="pt-anonuserpage">Not logged in</li>
                                        <li id="pt-anontalk"><a href="/wiki/Special:MyTalk"
                                                title="Discussion about edits from this IP address [n]" accesskey="n">Talk</a></li>
                                        <li id="pt-anoncontribs"><a href="/wiki/Special:MyContributions"
                                                title="A list of edits made from this IP address [y]" accesskey="y">Contributions</a>
                                        </li>
                                        <li id="pt-createaccount"><a
                                                href="/w/index.php?title=Special:CreateAccount&amp;returnto=Main+Page"
                                                title="You are encouraged to create an account and log in; however, it is not mandatory">Create
                                                account</a></li>
                                        <li id="pt-login"><a href="/w/index.php?title=Special:UserLogin&amp;returnto=Main+Page"
                                                title="You&#039;re encouraged to log in; however, it&#039;s not mandatory. [o]"
                                                accesskey="o">Log in</a></li>
                                    </ul>

                                </div>
                            </nav>

                            <div id="left-navigation">
                                <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                                <nav id="p-namespaces" class="mw-portlet mw-portlet-namespaces vector-menu vector-menu-tabs"
                                    aria-labelledby="p-namespaces-label" role="navigation">
                                    <h3 id="p-namespaces-label">
                                        <span>Namespaces</span>
                                    </h3>
                                    <div class="vector-menu-content">
                                        <ul class="vector-menu-content-list">
                                            <li id="ca-nstab-main" class="selected"><a href="/wiki/Main_Page"
                                                    title="View the content page [c]" accesskey="c">Main Page</a></li>
                                            <li id="ca-talk"><a href="/wiki/Talk:Main_Page" rel="discussion"
                                                    title="Discuss improvements to the content page [t]" accesskey="t">Talk</a></li>
                                        </ul>

                                    </div>
                                </nav>

                                <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                                <nav id="p-variants"
                                    class="mw-portlet mw-portlet-variants emptyPortlet vector-menu vector-menu-dropdown"
                                    aria-labelledby="p-variants-label" role="navigation">
                                    <input type="checkbox" class="vector-menu-checkbox" aria-labelledby="p-variants-label" />
                                    <h3 id="p-variants-label">
                                        <span>Variants</span>
                                    </h3>
                                    <div class="vector-menu-content">
                                        <ul class="vector-menu-content-list"></ul>

                                    </div>
                                </nav>

                            </div>
                            <div id="right-navigation">
                                <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                                <nav id="p-views" class="mw-portlet mw-portlet-views vector-menu vector-menu-tabs"
                                    aria-labelledby="p-views-label" role="navigation">
                                    <h3 id="p-views-label">
                                        <span>Views</span>
                                    </h3>
                                    <div class="vector-menu-content">
                                        <ul class="vector-menu-content-list">
                                            <li id="ca-view" class="selected"><a href="/wiki/Main_Page">Read</a></li>
                                            <li id="ca-viewsource"><a href="/w/index.php?title=Main_Page&amp;action=edit"
                                                    title="This page is protected.&#10;You can view its source [e]" accesskey="e">View
                                                    source</a></li>
                                            <li id="ca-history"><a href="/w/index.php?title=Main_Page&amp;action=history"
                                                    title="Past revisions of this page [h]" accesskey="h">View history</a></li>
                                        </ul>

                                    </div>
                                </nav>

                                <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                                <nav id="p-cactions"
                                    class="mw-portlet mw-portlet-cactions emptyPortlet vector-menu vector-menu-dropdown"
                                    aria-labelledby="p-cactions-label" role="navigation">
                                    <input type="checkbox" class="vector-menu-checkbox" aria-labelledby="p-cactions-label" />
                                    <h3 id="p-cactions-label">
                                        <span>More</span>
                                    </h3>
                                    <div class="vector-menu-content">
                                        <ul class="vector-menu-content-list"></ul>

                                    </div>
                                </nav>

                                <div id="p-search" role="search">
                                    <h3>
                                        <label for="searchInput">Search</label>
                                    </h3>
                                    <form action="/w/index.php" id="searchform">
                                        <div id="simpleSearch" data-search-loc="header-navigation">
                                            <input type="search" name="search" placeholder="Search Wikipedia" autocapitalize="sentences"
                                                title="Search Wikipedia [f]" accesskey="f" id="searchInput" />
                                            <input type="hidden" name="title" value="Special:Search">
                                            <input type="submit" name="fulltext" value="Search" title="Search Wikipedia for this text"
                                                id="mw-searchButton" class="searchButton mw-fallbackSearchButton" />
                                            <input type="submit" name="go" value="Go"
                                                title="Go to a page with this exact name if it exists" id="searchButton"
                                                class="searchButton" />
                                        </div>
                                    </form>
                                </div>

                            </div>
                        </div>

                        <div id="mw-panel">
                            <div id="p-logo" role="banner">
                                <a title="Visit the main page" class="mw-wiki-logo" href="/wiki/Main_Page"></a>
                            </div>
                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-navigation"
                                class="mw-portlet mw-portlet-navigation vector-menu vector-menu-portal portal portal-first"
                                aria-labelledby="p-navigation-label" role="navigation">
                                <h3 id="p-navigation-label">
                                    <span>Navigation</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li id="n-mainpage-description"><a href="/wiki/Main_Page" title="Visit the main page [z]"
                                                accesskey="z">Main page</a></li>
                                        <li id="n-contents"><a href="/wiki/Wikipedia:Contents"
                                                title="Guides to browsing Wikipedia">Contents</a></li>
                                        <li id="n-currentevents"><a href="/wiki/Portal:Current_events"
                                                title="Articles related to current events">Current events</a></li>
                                        <li id="n-randompage"><a href="/wiki/Special:Random"
                                                title="Visit a randomly selected article [x]" accesskey="x">Random article</a></li>
                                        <li id="n-aboutsite"><a href="/wiki/Wikipedia:About"
                                                title="Learn about Wikipedia and how it works">About Wikipedia</a></li>
                                        <li id="n-contactpage"><a href="//en.wikipedia.org/wiki/Wikipedia:Contact_us"
                                                title="How to contact Wikipedia">Contact us</a></li>
                                        <li id="n-sitesupport"><a
                                                href="https://donate.wikimedia.org/wiki/Special:FundraiserRedirector?utm_source=donate&amp;utm_medium=sidebar&amp;utm_campaign=C13_en.wikipedia.org&amp;uselang=en"
                                                title="Support us by donating to the Wikimedia Foundation">Donate</a></li>
                                    </ul>

                                </div>
                            </nav>

                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-interaction" class="mw-portlet mw-portlet-interaction vector-menu vector-menu-portal portal"
                                aria-labelledby="p-interaction-label" role="navigation">
                                <h3 id="p-interaction-label">
                                    <span>Contribute</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li id="n-help"><a href="/wiki/Help:Contents"
                                                title="Guidance on how to use and edit Wikipedia">Help</a></li>
                                        <li id="n-introduction"><a href="/wiki/Help:Introduction"
                                                title="Learn how to edit Wikipedia">Learn to edit</a></li>
                                        <li id="n-portal"><a href="/wiki/Wikipedia:Community_portal"
                                                title="The hub for editors">Community portal</a></li>
                                        <li id="n-recentchanges"><a href="/wiki/Special:RecentChanges"
                                                title="A list of recent changes to Wikipedia [r]" accesskey="r">Recent changes</a></li>
                                        <li id="n-upload"><a href="/wiki/Wikipedia:File_Upload_Wizard"
                                                title="Add images or other media for use on Wikipedia">Upload file</a></li>
                                    </ul>

                                </div>
                            </nav>
                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-tb" class="mw-portlet mw-portlet-tb vector-menu vector-menu-portal portal"
                                aria-labelledby="p-tb-label" role="navigation">
                                <h3 id="p-tb-label">
                                    <span>Tools</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li id="t-whatlinkshere"><a href="/wiki/Special:WhatLinksHere/Main_Page"
                                                title="List of all English Wikipedia pages containing links to this page [j]"
                                                accesskey="j">What links here</a></li>
                                        <li id="t-recentchangeslinked"><a href="/wiki/Special:RecentChangesLinked/Main_Page"
                                                rel="nofollow" title="Recent changes in pages linked from this page [k]"
                                                accesskey="k">Related changes</a></li>
                                        <li id="t-upload"><a href="/wiki/Wikipedia:File_Upload_Wizard" title="Upload files [u]"
                                                accesskey="u">Upload file</a></li>
                                        <li id="t-specialpages"><a href="/wiki/Special:SpecialPages"
                                                title="A list of all special pages [q]" accesskey="q">Special pages</a></li>
                                        <li id="t-permalink"><a href="/w/index.php?title=Main_Page&amp;oldid=987965326"
                                                title="Permanent link to this revision of this page">Permanent link</a></li>
                                        <li id="t-info"><a href="/w/index.php?title=Main_Page&amp;action=info"
                                                title="More information about this page">Page information</a></li>
                                        <li id="t-cite"><a
                                                href="/w/index.php?title=Special:CiteThisPage&amp;page=Main_Page&amp;id=987965326&amp;wpFormIdentifier=titleform"
                                                title="Information on how to cite this page">Cite this page</a></li>
                                        <li id="t-wikibase"><a href="https://www.wikidata.org/wiki/Special:EntityPage/Q5296"
                                                title="Structured data on this page hosted by Wikidata [g]" accesskey="g">Wikidata
                                                item</a></li>
                                    </ul>

                                </div>
                            </nav>
                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-coll-print_export"
                                class="mw-portlet mw-portlet-coll-print_export vector-menu vector-menu-portal portal"
                                aria-labelledby="p-coll-print_export-label" role="navigation">
                                <h3 id="p-coll-print_export-label">
                                    <span>Print/export</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li id="coll-download-as-rl"><a
                                                href="/w/index.php?title=Special:DownloadAsPdf&amp;page=Main_Page&amp;action=show-download-screen"
                                                title="Download this page as a PDF file">Download as PDF</a></li>
                                        <li id="t-print"><a href="/w/index.php?title=Main_Page&amp;printable=yes"
                                                title="Printable version of this page [p]" accesskey="p">Printable version</a></li>
                                    </ul>

                                </div>
                            </nav>
                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-wikibase-otherprojects"
                                class="mw-portlet mw-portlet-wikibase-otherprojects vector-menu vector-menu-portal portal"
                                aria-labelledby="p-wikibase-otherprojects-label" role="navigation">
                                <h3 id="p-wikibase-otherprojects-label">
                                    <span>In other projects</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li class="wb-otherproject-link wb-otherproject-commons"><a
                                                href="https://commons.wikimedia.org/wiki/Main_Page" hreflang="en">Wikimedia Commons</a>
                                        </li>
                                        <li class="wb-otherproject-link wb-otherproject-mediawiki"><a
                                                href="https://www.mediawiki.org/wiki/MediaWiki" hreflang="en">MediaWiki</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-meta"><a
                                                href="https://meta.wikimedia.org/wiki/Main_Page" hreflang="en">Meta-Wiki</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-species"><a
                                                href="https://species.wikimedia.org/wiki/Main_Page" hreflang="en">Wikispecies</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikibooks"><a
                                                href="https://en.wikibooks.org/wiki/Main_Page" hreflang="en">Wikibooks</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikidata"><a
                                                href="https://www.wikidata.org/wiki/Wikidata:Main_Page" hreflang="en">Wikidata</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikimania"><a
                                                href="https://wikimania.wikimedia.org/wiki/Wikimania" hreflang="en">Wikimania</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikinews"><a
                                                href="https://en.wikinews.org/wiki/Main_Page" hreflang="en">Wikinews</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikiquote"><a
                                                href="https://en.wikiquote.org/wiki/Main_Page" hreflang="en">Wikiquote</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikisource"><a
                                                href="https://en.wikisource.org/wiki/Main_Page" hreflang="en">Wikisource</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikiversity"><a
                                                href="https://en.wikiversity.org/wiki/Wikiversity:Main_Page"
                                                hreflang="en">Wikiversity</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wikivoyage"><a
                                                href="https://en.wikivoyage.org/wiki/Main_Page" hreflang="en">Wikivoyage</a></li>
                                        <li class="wb-otherproject-link wb-otherproject-wiktionary"><a
                                                href="https://en.wiktionary.org/wiki/Wiktionary:Main_Page" hreflang="en">Wiktionary</a>
                                        </li>
                                    </ul>

                                </div>
                            </nav>

                            <!-- Please do not use role attribute as CSS selector, it is deprecated. -->
                            <nav id="p-lang" class="mw-portlet mw-portlet-lang vector-menu vector-menu-portal portal"
                                aria-labelledby="p-lang-label" role="navigation">
                                <h3 id="p-lang-label">
                                    <span>Languages</span>
                                </h3>
                                <div class="vector-menu-content">
                                    <ul class="vector-menu-content-list">
                                        <li class="interlanguage-link interwiki-ar"><a href="https://ar.wikipedia.org/wiki/"
                                                title="Arabic" lang="ar" hreflang="ar" class="interlanguage-link-target">العربية</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-bg"><a href="https://bg.wikipedia.org/wiki/"
                                                title="Bulgarian" lang="bg" hreflang="bg"
                                                class="interlanguage-link-target">Български</a></li>
                                        <li class="interlanguage-link interwiki-bs"><a href="https://bs.wikipedia.org/wiki/"
                                                title="Bosnian" lang="bs" hreflang="bs" class="interlanguage-link-target">Bosanski</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-ca"><a href="https://ca.wikipedia.org/wiki/"
                                                title="Catalan" lang="ca" hreflang="ca" class="interlanguage-link-target">Català</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-cs"><a href="https://cs.wikipedia.org/wiki/"
                                                title="Czech" lang="cs" hreflang="cs" class="interlanguage-link-target">Čeština</a></li>
                                        <li class="interlanguage-link interwiki-da"><a href="https://da.wikipedia.org/wiki/"
                                                title="Danish" lang="da" hreflang="da" class="interlanguage-link-target">Dansk</a></li>
                                        <li class="interlanguage-link interwiki-de"><a href="https://de.wikipedia.org/wiki/"
                                                title="German" lang="de" hreflang="de" class="interlanguage-link-target">Deutsch</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-et"><a href="https://et.wikipedia.org/wiki/"
                                                title="Estonian" lang="et" hreflang="et" class="interlanguage-link-target">Eesti</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-el"><a href="https://el.wikipedia.org/wiki/"
                                                title="Greek" lang="el" hreflang="el" class="interlanguage-link-target">Ελληνικά</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-es"><a href="https://es.wikipedia.org/wiki/"
                                                title="Spanish" lang="es" hreflang="es" class="interlanguage-link-target">Español</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-eo"><a href="https://eo.wikipedia.org/wiki/"
                                                title="Esperanto" lang="eo" hreflang="eo"
                                                class="interlanguage-link-target">Esperanto</a></li>
                                        <li class="interlanguage-link interwiki-eu"><a href="https://eu.wikipedia.org/wiki/"
                                                title="Basque" lang="eu" hreflang="eu" class="interlanguage-link-target">Euskara</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-fa"><a href="https://fa.wikipedia.org/wiki/"
                                                title="Persian" lang="fa" hreflang="fa" class="interlanguage-link-target">فارسی</a></li>
                                        <li class="interlanguage-link interwiki-fr"><a href="https://fr.wikipedia.org/wiki/"
                                                title="French" lang="fr" hreflang="fr" class="interlanguage-link-target">Français</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-gl"><a href="https://gl.wikipedia.org/wiki/"
                                                title="Galician" lang="gl" hreflang="gl" class="interlanguage-link-target">Galego</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-ko"><a href="https://ko.wikipedia.org/wiki/"
                                                title="Korean" lang="ko" hreflang="ko" class="interlanguage-link-target">한국어</a></li>
                                        <li class="interlanguage-link interwiki-hr"><a href="https://hr.wikipedia.org/wiki/"
                                                title="Croatian" lang="hr" hreflang="hr" class="interlanguage-link-target">Hrvatski</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-id"><a href="https://id.wikipedia.org/wiki/"
                                                title="Indonesian" lang="id" hreflang="id" class="interlanguage-link-target">Bahasa
                                                Indonesia</a></li>
                                        <li class="interlanguage-link interwiki-it"><a href="https://it.wikipedia.org/wiki/"
                                                title="Italian" lang="it" hreflang="it" class="interlanguage-link-target">Italiano</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-he"><a href="https://he.wikipedia.org/wiki/"
                                                title="Hebrew" lang="he" hreflang="he" class="interlanguage-link-target">עברית</a></li>
                                        <li class="interlanguage-link interwiki-ka"><a href="https://ka.wikipedia.org/wiki/"
                                                title="Georgian" lang="ka" hreflang="ka" class="interlanguage-link-target">ქართული</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-lv"><a href="https://lv.wikipedia.org/wiki/"
                                                title="Latvian" lang="lv" hreflang="lv" class="interlanguage-link-target">Latviešu</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-lt"><a href="https://lt.wikipedia.org/wiki/"
                                                title="Lithuanian" lang="lt" hreflang="lt"
                                                class="interlanguage-link-target">Lietuvių</a></li>
                                        <li class="interlanguage-link interwiki-hu"><a href="https://hu.wikipedia.org/wiki/"
                                                title="Hungarian" lang="hu" hreflang="hu" class="interlanguage-link-target">Magyar</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-mk"><a href="https://mk.wikipedia.org/wiki/"
                                                title="Macedonian" lang="mk" hreflang="mk"
                                                class="interlanguage-link-target">Македонски</a></li>
                                        <li class="interlanguage-link interwiki-ms"><a href="https://ms.wikipedia.org/wiki/"
                                                title="Malay" lang="ms" hreflang="ms" class="interlanguage-link-target">Bahasa
                                                Melayu</a></li>
                                        <li class="interlanguage-link interwiki-nl"><a href="https://nl.wikipedia.org/wiki/"
                                                title="Dutch" lang="nl" hreflang="nl" class="interlanguage-link-target">Nederlands</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-ja"><a href="https://ja.wikipedia.org/wiki/"
                                                title="Japanese" lang="ja" hreflang="ja" class="interlanguage-link-target">日本語</a></li>
                                        <li class="interlanguage-link interwiki-no"><a href="https://no.wikipedia.org/wiki/"
                                                title="Norwegian Bokmål" lang="nb" hreflang="nb" class="interlanguage-link-target">Norsk
                                                bokmål</a></li>
                                        <li class="interlanguage-link interwiki-nn"><a href="https://nn.wikipedia.org/wiki/"
                                                title="Norwegian Nynorsk" lang="nn" hreflang="nn"
                                                class="interlanguage-link-target">Norsk nynorsk</a></li>
                                        <li class="interlanguage-link interwiki-pl"><a href="https://pl.wikipedia.org/wiki/"
                                                title="Polish" lang="pl" hreflang="pl" class="interlanguage-link-target">Polski</a></li>
                                        <li class="interlanguage-link interwiki-pt"><a href="https://pt.wikipedia.org/wiki/"
                                                title="Portuguese" lang="pt" hreflang="pt"
                                                class="interlanguage-link-target">Português</a></li>
                                        <li class="interlanguage-link interwiki-ro"><a href="https://ro.wikipedia.org/wiki/"
                                                title="Romanian" lang="ro" hreflang="ro" class="interlanguage-link-target">Română</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-ru"><a href="https://ru.wikipedia.org/wiki/"
                                                title="Russian" lang="ru" hreflang="ru" class="interlanguage-link-target">Русский</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-simple"><a href="https://simple.wikipedia.org/wiki/"
                                                title="Simple English" lang="en-simple" hreflang="en-simple"
                                                class="interlanguage-link-target">Simple English</a></li>
                                        <li class="interlanguage-link interwiki-sk"><a href="https://sk.wikipedia.org/wiki/"
                                                title="Slovak" lang="sk" hreflang="sk" class="interlanguage-link-target">Slovenčina</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-sl"><a href="https://sl.wikipedia.org/wiki/"
                                                title="Slovenian" lang="sl" hreflang="sl"
                                                class="interlanguage-link-target">Slovenščina</a></li>
                                        <li class="interlanguage-link interwiki-sr"><a href="https://sr.wikipedia.org/wiki/"
                                                title="Serbian" lang="sr" hreflang="sr" class="interlanguage-link-target">Српски /
                                                srpski</a></li>
                                        <li class="interlanguage-link interwiki-sh"><a href="https://sh.wikipedia.org/wiki/"
                                                title="Serbo-Croatian" lang="sh" hreflang="sh"
                                                class="interlanguage-link-target">Srpskohrvatski / српскохрватски</a></li>
                                        <li class="interlanguage-link interwiki-fi"><a href="https://fi.wikipedia.org/wiki/"
                                                title="Finnish" lang="fi" hreflang="fi" class="interlanguage-link-target">Suomi</a></li>
                                        <li class="interlanguage-link interwiki-sv"><a href="https://sv.wikipedia.org/wiki/"
                                                title="Swedish" lang="sv" hreflang="sv" class="interlanguage-link-target">Svenska</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-th"><a href="https://th.wikipedia.org/wiki/"
                                                title="Thai" lang="th" hreflang="th" class="interlanguage-link-target">ไทย</a></li>
                                        <li class="interlanguage-link interwiki-tr"><a href="https://tr.wikipedia.org/wiki/"
                                                title="Turkish" lang="tr" hreflang="tr" class="interlanguage-link-target">Türkçe</a>
                                        </li>
                                        <li class="interlanguage-link interwiki-uk"><a href="https://uk.wikipedia.org/wiki/"
                                                title="Ukrainian" lang="uk" hreflang="uk"
                                                class="interlanguage-link-target">Українська</a></li>
                                        <li class="interlanguage-link interwiki-vi"><a href="https://vi.wikipedia.org/wiki/"
                                                title="Vietnamese" lang="vi" hreflang="vi" class="interlanguage-link-target">Tiếng
                                                Việt</a></li>
                                        <li class="interlanguage-link interwiki-zh"><a href="https://zh.wikipedia.org/wiki/"
                                                title="Chinese" lang="zh" hreflang="zh" class="interlanguage-link-target">中文</a></li>
                                    </ul>

                                </div>
                            </nav>

                        </div>

                    </div>
                    <footer id="footer" class="mw-footer" role="contentinfo">
                        <ul id="footer-info">
                            <li id="footer-info-lastmod"> This page was last edited on 10 November 2020, at 08:18<span
                                    class="anonymous-show">&#160;(UTC)</span>.</li>
                            <li id="footer-info-copyright">Text is available under the <a rel="license"
                                    href="//en.wikipedia.org/wiki/Wikipedia:Text_of_Creative_Commons_Attribution-ShareAlike_3.0_Unported_License">Creative
                                    Commons Attribution-ShareAlike License</a><a rel="license"
                                    href="//creativecommons.org/licenses/by-sa/3.0/" style="display:none;"></a>;
                                additional terms may apply. By using this site, you agree to the <a
                                    href="//foundation.wikimedia.org/wiki/Terms_of_Use">Terms of Use</a> and <a
                                    href="//foundation.wikimedia.org/wiki/Privacy_policy">Privacy Policy</a>. Wikipedia® is a registered
                                trademark of the <a href="//www.wikimediafoundation.org/">Wikimedia Foundation, Inc.</a>, a non-profit
                                organization.</li>
                        </ul>

                        <ul id="footer-places">
                            <li id="footer-places-privacy"><a href="https://foundation.wikimedia.org/wiki/Privacy_policy" class="extiw"
                                    title="wmf:Privacy policy">Privacy policy</a></li>
                            <li id="footer-places-about"><a href="/wiki/Wikipedia:About" title="Wikipedia:About">About Wikipedia</a>
                            </li>
                            <li id="footer-places-disclaimer"><a href="/wiki/Wikipedia:General_disclaimer"
                                    title="Wikipedia:General disclaimer">Disclaimers</a></li>
                            <li id="footer-places-contact"><a href="//en.wikipedia.org/wiki/Wikipedia:Contact_us">Contact Wikipedia</a>
                            </li>
                            <li id="footer-places-mobileview"><a
                                    href="//en.m.wikipedia.org/w/index.php?title=Main_Page&amp;mobileaction=toggle_view_mobile"
                                    class="noprint stopMobileRedirectToggle">Mobile view</a></li>
                            <li id="footer-places-developers"><a
                                    href="https://www.mediawiki.org/wiki/Special:MyLanguage/How_to_contribute">Developers</a></li>
                            <li id="footer-places-statslink"><a href="https://stats.wikimedia.org/#/en.wikipedia.org">Statistics</a>
                            </li>
                            <li id="footer-places-cookiestatement"><a
                                    href="https://foundation.wikimedia.org/wiki/Cookie_statement">Cookie statement</a></li>
                        </ul>

                        <ul id="footer-icons" class="noprint">
                            <li id="footer-copyrightico"><a href="https://wikimediafoundation.org/"><img
                                        src="/static/images/footer/wikimedia-button.png"
                                        srcset="/static/images/footer/wikimedia-button-1.5x.png 1.5x, /static/images/footer/wikimedia-button-2x.png 2x"
                                        width="88" height="31" alt="Wikimedia Foundation" loading="lazy" /></a></li>
                            <li id="footer-poweredbyico"><a href="https://www.mediawiki.org/"><img
                                        src="/static/images/footer/poweredby_mediawiki_88x31.png" alt="Powered by MediaWiki"
                                        srcset="/static/images/footer/poweredby_mediawiki_132x47.png 1.5x, /static/images/footer/poweredby_mediawiki_176x62.png 2x"
                                        width="88" height="31" loading="lazy" /></a></li>
                        </ul>

                        <div style="clear: both;"></div>
                    </footer>


                    <script>
                        (RLQ = window.RLQ || []).push(function () {
                            mw.config.set({
                                "wgPageParseReport": {
                                    "limitreport": {
                                        "cputime": "0.360",
                                        "walltime": "0.483",
                                        "ppvisitednodes": {
                                            "value": 4117,
                                            "limit": 1000000
                                        },
                                        "postexpandincludesize": {
                                            "value": 120543,
                                            "limit": 2097152
                                        },
                                        "templateargumentsize": {
                                            "value": 7597,
                                            "limit": 2097152
                                        },
                                        "expansiondepth": {
                                            "value": 21,
                                            "limit": 40
                                        },
                                        "expensivefunctioncount": {
                                            "value": 17,
                                            "limit": 500
                                        },
                                        "unstrip-depth": {
                                            "value": 0,
                                            "limit": 20
                                        },
                                        "unstrip-size": {
                                            "value": 4875,
                                            "limit": 5000000
                                        },
                                        "entityaccesscount": {
                                            "value": 0,
                                            "limit": 400
                                        },
                                        "timingprofile": ["100.00%  331.605      1 -total",
                                            " 40.05%  132.806      9 Template:Main_page_image",
                                            " 33.39%  110.709      1 Wikipedia:Main_Page/Tomorrow",
                                            " 23.38%   77.539      9 Template:Str_number/trim",
                                            " 21.16%   70.174     27 Template:If_empty",
                                            " 18.57%   61.594      2 Template:Main_page_image/TFA",
                                            " 16.50%   54.718      1 Wikipedia:Today's_featured_article/December_21,_2020",
                                            " 13.09%   43.406      2 Template:In_the_news",
                                            " 12.96%   42.970      1 Wikipedia:Selected_anniversaries/December_21",
                                            " 12.65%   41.953      1 Template:Did_you_know/Queue/2"
                                        ]
                                    },
                                    "scribunto": {
                                        "limitreport-timeusage": {
                                            "value": "0.083",
                                            "limit": "10.000"
                                        },
                                        "limitreport-memusage": {
                                            "value": 2108809,
                                            "limit": 52428800
                                        }
                                    },
                                    "cachereport": {
                                        "origin": "mw1403",
                                        "timestamp": "20201221154153",
                                        "ttl": 3600,
                                        "transientcontent": true
                                    }
                                }
                            });
                        });
                    </script>
                    <script type="application/ld+json">
                        {
                            "@context": "https:\/\/schema.org",
                            "@type": "Article",
                            "name": "Main Page",
                            "url": "https:\/\/en.wikipedia.org\/wiki\/Main_Page",
                            "sameAs": "http:\/\/www.wikidata.org\/entity\/Q5296",
                            "mainEntity": "http:\/\/www.wikidata.org\/entity\/Q5296",
                            "author": {
                                "@type": "Organization",
                                "name": "Contributors to Wikimedia projects"
                            },
                            "publisher": {
                                "@type": "Organization",
                                "name": "Wikimedia Foundation, Inc.",
                                "logo": {
                                    "@type": "ImageObject",
                                    "url": "https:\/\/www.wikimedia.org\/static\/images\/wmf-hor-googpub.png"
                                }
                            },
                            "datePublished": "2002-01-26T15:28:12Z",
                            "dateModified": "2020-11-10T08:18:07Z",
                            "image": "https:\/\/upload.wikimedia.org\/wikipedia\/en\/f\/f2\/Chien_Courant_Italien_A_Poil_Ras.jpg",
                            "headline": "main page of a Wikimedia project (common for Wikipedia, Wiktionary and other projects)"
                        }
                    </script>
                    <script>
                        (RLQ = window.RLQ || []).push(function () {
                            mw.config.set({
                                "wgBackendResponseTime": 160,
                                "wgHostname": "mw1395"
                            });
                        });
                    </script>
                </body>

                </html>
           "#,
        ));
        let result = parser.parse(data.as_str());
        assert!(matches!(result, ParsingResult::Ok(_)));
    }
}
