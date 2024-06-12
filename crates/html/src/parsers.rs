// Module implements the parser has defined in: https://github.com/taoqf/node-html-parser

use core::str::Chars;
use std::{cell, rc};

use anyhow::anyhow;
use lazy_regex::{lazy_regex, regex, regex_captures, Lazy, Regex, RegexBuilder};
use lazy_static::lazy_static;
use phf::phf_map;
use std::{any, collections::HashMap, str::FromStr};
use thiserror::Error;

use crate::markup::ElementResult;

lazy_static! {
    static ref HTML_TAG_REGEX: HashMap<HTMLTags, regex::Regex> = vec![
        (
            HTMLTags::A,
            RegexBuilder::new("^a$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Abbr,
            RegexBuilder::new("^abbr$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Address,
            RegexBuilder::new("^address$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Area,
            RegexBuilder::new("^area$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Article,
            RegexBuilder::new("^article$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Aside,
            RegexBuilder::new("^aside$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Audio,
            RegexBuilder::new("^audio$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::B,
            RegexBuilder::new("^b$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Base,
            RegexBuilder::new("^base$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Bdi,
            RegexBuilder::new("^bdi$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Bdo,
            RegexBuilder::new("^bdo$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Blockquote,
            RegexBuilder::new("^blockquote$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Br,
            RegexBuilder::new("^br$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Button,
            RegexBuilder::new("^button$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Canvas,
            RegexBuilder::new("^canvas$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Caption,
            RegexBuilder::new("^caption$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Cite,
            RegexBuilder::new("^cite$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Code,
            RegexBuilder::new("^code$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Col,
            RegexBuilder::new("^col$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Colgroup,
            RegexBuilder::new("^colgroup$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Data,
            RegexBuilder::new("^data$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Datalist,
            RegexBuilder::new("^datalist$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Dd,
            RegexBuilder::new("^dd$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Del,
            RegexBuilder::new("^del$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Details,
            RegexBuilder::new("^details$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Dfn,
            RegexBuilder::new("^dfn$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Dialog,
            RegexBuilder::new("^dialog$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Div,
            RegexBuilder::new("^div$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Dl,
            RegexBuilder::new("^dl$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Dt,
            RegexBuilder::new("^dt$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Em,
            RegexBuilder::new("^em$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Embed,
            RegexBuilder::new("^embed$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Fieldset,
            RegexBuilder::new("^fieldset$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Figcaption,
            RegexBuilder::new("^figcaption$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Figure,
            RegexBuilder::new("^figure$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Footer,
            RegexBuilder::new("^footer$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Form,
            RegexBuilder::new("^form$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H1,
            RegexBuilder::new("^h1$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H2,
            RegexBuilder::new("^h2$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H3,
            RegexBuilder::new("^h3$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H4,
            RegexBuilder::new("^h4$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H5,
            RegexBuilder::new("^h5$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::H6,
            RegexBuilder::new("^h6$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Header,
            RegexBuilder::new("^header$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Hgroup,
            RegexBuilder::new("^hgroup$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Hr,
            RegexBuilder::new("^hr$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::I,
            RegexBuilder::new("^i$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Iframe,
            RegexBuilder::new("^iframe$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Img,
            RegexBuilder::new("^img$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Input,
            RegexBuilder::new("^input$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Ins,
            RegexBuilder::new("^ins$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Kbd,
            RegexBuilder::new("^kbd$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Label,
            RegexBuilder::new("^label$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Legend,
            RegexBuilder::new("^legend$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Li,
            RegexBuilder::new("^li$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Link,
            RegexBuilder::new("^link$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Main,
            RegexBuilder::new("^main$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Map,
            RegexBuilder::new("^map$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Mark,
            RegexBuilder::new("^mark$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Menu,
            RegexBuilder::new("^menu$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Menuitem,
            RegexBuilder::new("^menuitem$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Meta,
            RegexBuilder::new("^meta$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Meter,
            RegexBuilder::new("^meter$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Nav,
            RegexBuilder::new("^nav$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Noframes,
            RegexBuilder::new("^noframes$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Noscript,
            RegexBuilder::new("^noscript$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Object,
            RegexBuilder::new("^object$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Ol,
            RegexBuilder::new("^ol$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Optgroup,
            RegexBuilder::new("^optgroup$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Option,
            RegexBuilder::new("^option$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Output,
            RegexBuilder::new("^output$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::P,
            RegexBuilder::new("^p$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Param,
            RegexBuilder::new("^param$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Picture,
            RegexBuilder::new("^picture$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Monospace,
            RegexBuilder::new("^monospace$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Pre,
            RegexBuilder::new("^pre$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Progress,
            RegexBuilder::new("^progress$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Q,
            RegexBuilder::new("^q$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Rp,
            RegexBuilder::new("^rp$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Rt,
            RegexBuilder::new("^rt$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Rtc,
            RegexBuilder::new("^rtc$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Ruby,
            RegexBuilder::new("^ruby$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::S,
            RegexBuilder::new("^s$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Samp,
            RegexBuilder::new("^samp$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Script,
            RegexBuilder::new("^script$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Section,
            RegexBuilder::new("^section$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Select,
            RegexBuilder::new("^select$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Slot,
            RegexBuilder::new("^slot$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Small,
            RegexBuilder::new("^small$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Source,
            RegexBuilder::new("^source$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Span,
            RegexBuilder::new("^span$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Strong,
            RegexBuilder::new("^strong$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Style,
            RegexBuilder::new("^style$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Sub,
            RegexBuilder::new("^sub$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Summary,
            RegexBuilder::new("^summary$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Sup,
            RegexBuilder::new("^sup$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Table,
            RegexBuilder::new("^table$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Tbody,
            RegexBuilder::new("^tbody$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Td,
            RegexBuilder::new("^td$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Template,
            RegexBuilder::new("^template$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Textarea,
            RegexBuilder::new("^textarea$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Tfoot,
            RegexBuilder::new("^tfoot$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Th,
            RegexBuilder::new("^th$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Thead,
            RegexBuilder::new("^thead$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Time,
            RegexBuilder::new("^time$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Title,
            RegexBuilder::new("^title$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Tr,
            RegexBuilder::new("^tr$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Track,
            RegexBuilder::new("^track$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::U,
            RegexBuilder::new("^u$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Ul,
            RegexBuilder::new("^ul$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Var,
            RegexBuilder::new("^var$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Video,
            RegexBuilder::new("^video$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            HTMLTags::Wbr,
            RegexBuilder::new("^wbr$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
    ]
    .into_iter()
    .collect();
    static ref SVG_TAG_REGEX: HashMap<SVGTags, regex::Regex> = vec![
        (
            SVGTags::A,
            RegexBuilder::new("^a$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Altglyph,
            RegexBuilder::new("^altglyph$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Altglyphdef,
            RegexBuilder::new("^altglyphdef$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Altglyphitem,
            RegexBuilder::new("^altglyphitem$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Animate,
            RegexBuilder::new("^animate$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Animatecolor,
            RegexBuilder::new("^animatecolor$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Animatemotion,
            RegexBuilder::new("^animatemotion$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Animatetransform,
            RegexBuilder::new("^animatetransform$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Circle,
            RegexBuilder::new("^circle$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Clippath,
            RegexBuilder::new("^clippath$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Cursor,
            RegexBuilder::new("^cursor$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Defs,
            RegexBuilder::new("^defs$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Desc,
            RegexBuilder::new("^desc$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Discard,
            RegexBuilder::new("^discard$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Ellipse,
            RegexBuilder::new("^ellipse$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feblend,
            RegexBuilder::new("^feblend$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fecolormatrix,
            RegexBuilder::new("^fecolormatrix$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fecomponenttransfer,
            RegexBuilder::new("^fecomponenttransfer$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fecomposite,
            RegexBuilder::new("^fecomposite$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feconvolvematrix,
            RegexBuilder::new("^feconvolvematrix$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fediffuselighting,
            RegexBuilder::new("^fediffuselighting$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fedisplacementmap,
            RegexBuilder::new("^fedisplacementmap$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fedistantlight,
            RegexBuilder::new("^fedistantlight$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fedropshadow,
            RegexBuilder::new("^fedropshadow$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feflood,
            RegexBuilder::new("^feflood$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fefunca,
            RegexBuilder::new("^fefunca$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fefuncb,
            RegexBuilder::new("^fefuncb$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fefuncg,
            RegexBuilder::new("^fefuncg$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fefuncr,
            RegexBuilder::new("^fefuncr$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fegaussianblur,
            RegexBuilder::new("^fegaussianblur$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feimage,
            RegexBuilder::new("^feimage$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Femerge,
            RegexBuilder::new("^femerge$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Femergenode,
            RegexBuilder::new("^femergenode$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Femorphology,
            RegexBuilder::new("^femorphology$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feoffset,
            RegexBuilder::new("^feoffset$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fepointlight,
            RegexBuilder::new("^fepointlight$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fespecularlighting,
            RegexBuilder::new("^fespecularlighting$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fespotlight,
            RegexBuilder::new("^fespotlight$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Fetile,
            RegexBuilder::new("^fetile$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Feturbulence,
            RegexBuilder::new("^feturbulence$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Filter,
            RegexBuilder::new("^filter$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Font,
            RegexBuilder::new("^font$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Foreignobject,
            RegexBuilder::new("^foreignobject$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::G,
            RegexBuilder::new("^g$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Glyph,
            RegexBuilder::new("^glyph$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Glyphref,
            RegexBuilder::new("^glyphref$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Hatch,
            RegexBuilder::new("^hatch$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Hatchpath,
            RegexBuilder::new("^hatchpath$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Hkern,
            RegexBuilder::new("^hkern$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Image,
            RegexBuilder::new("^image$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Line,
            RegexBuilder::new("^line$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Lineargradient,
            RegexBuilder::new("^lineargradient$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Marker,
            RegexBuilder::new("^marker$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Mask,
            RegexBuilder::new("^mask$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Mesh,
            RegexBuilder::new("^mesh$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Meshgradient,
            RegexBuilder::new("^meshgradient$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Meshpatch,
            RegexBuilder::new("^meshpatch$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Meshrow,
            RegexBuilder::new("^meshrow$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Metadata,
            RegexBuilder::new("^metadata$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Mpath,
            RegexBuilder::new("^mpath$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Path,
            RegexBuilder::new("^path$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Tiled,
            RegexBuilder::new("^tiled$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Pattern,
            RegexBuilder::new("^pattern$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Polygon,
            RegexBuilder::new("^polygon$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Polyline,
            RegexBuilder::new("^polyline$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Radialgradient,
            RegexBuilder::new("^radialgradient$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Rect,
            RegexBuilder::new("^rect$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Script,
            RegexBuilder::new("^script$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Set,
            RegexBuilder::new("^set$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Solidcolor,
            RegexBuilder::new("^solidcolor$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Stop,
            RegexBuilder::new("^stop$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Style,
            RegexBuilder::new("^style$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Svg,
            RegexBuilder::new("^svg$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Switch,
            RegexBuilder::new("^switch$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Symbol,
            RegexBuilder::new("^symbol$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Text,
            RegexBuilder::new("^text$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Textpath,
            RegexBuilder::new("^textpath$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Title,
            RegexBuilder::new("^title$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Tref,
            RegexBuilder::new("^tref$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Tspan,
            RegexBuilder::new("^tspan$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Unknown,
            RegexBuilder::new("^unknown$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Use,
            RegexBuilder::new("^use$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::View,
            RegexBuilder::new("^view$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
        (
            SVGTags::Vkern,
            RegexBuilder::new("^vkern$")
                .case_insensitive(true)
                .build()
                .unwrap()
        ),
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

    #[error("we expect after tag names will be a space")]
    ExpectingSpaceAfterTagName,
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
    pub fn get_regex(&self) -> Option<&lazy_regex::Regex> {
        SVG_TAG_REGEX.get(self)
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
    Header,
    Hgroup,
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
            "header" => Ok(HTMLTags::Header),
            "hgroup" => Ok(HTMLTags::Hgroup),
            "hr" => Ok(HTMLTags::Hr),
            "i" => Ok(HTMLTags::I),
            "iframe" => Ok(HTMLTags::Iframe),
            "img" => Ok(HTMLTags::Img),
            "input" => Ok(HTMLTags::Input),
            "ins" => Ok(HTMLTags::Ins),
            "kbd" => Ok(HTMLTags::Kbd),
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
            _ => Err(ParsingTagError::UnknownTag),
        }
    }
}

impl HTMLTags {
    pub fn get_regex(&self) -> Option<&lazy_regex::Regex> {
        HTML_TAG_REGEX.get(self)
    }

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
            HTMLTags::Header => "header",
            HTMLTags::Hgroup => "hgroup",
            HTMLTags::Hr => "hr",
            HTMLTags::I => "i",
            HTMLTags::Iframe => "iframe",
            HTMLTags::Img => "img",
            HTMLTags::Input => "input",
            HTMLTags::Ins => "ins",
            HTMLTags::Kbd => "kbd",
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
        Ok(MarkupTags::Text(s.to_owned()))
    }
}

impl MarkupTags {
    pub fn is_element_closed_by_opening_tag(me: MarkupTags, other: MarkupTags) -> bool {
        match (me, other) {
            (MarkupTags::HTML(me), MarkupTags::HTML(other)) => {
                HTMLTags::is_html_element_closed_by_opening_tag(me, other)
            }
            _ => false,
        }
    }

    pub fn is_element_closed_by_closing_tag(me: MarkupTags, other: MarkupTags) -> bool {
        match (me, other) {
            (MarkupTags::HTML(me), MarkupTags::HTML(other)) => {
                HTMLTags::is_html_element_closed_by_closing_tag(me, other)
            }
            _ => false,
        }
    }

    pub fn get_regex(&self) -> Option<&lazy_regex::Regex> {
        match self {
            MarkupTags::HTML(html) => html.get_regex(),
            MarkupTags::SVG(svg) => svg.get_regex(),
            _ => None,
        }
    }

    pub fn to_string<'a>(self) -> Result<String, anyhow::Error> {
        match self {
            MarkupTags::SVG(sg) => Ok(sg.into()),
            MarkupTags::HTML(ht) => Ok(ht.into()),
            MarkupTags::Comment(text) | MarkupTags::Text(text) | MarkupTags::Component(text) => {
                Ok(text.clone())
            }
        }
    }

    pub fn to_str<'a>(self) -> Result<&'a str, anyhow::Error> {
        match self {
            MarkupTags::SVG(sg) => Ok(sg.into()),
            MarkupTags::HTML(ht) => Ok(ht.into()),
            _ => Err(anyhow!("Cant get &str representation of {:?}", self)),
        }
    }

    pub fn is_block_tag(tag: MarkupTags) -> bool {
        if MarkupTags::is_table_tag(tag.clone())
            || MarkupTags::is_d_tag(tag.clone())
            || MarkupTags::is_header_tag(tag.clone())
            || MarkupTags::is_f_tag(tag.clone())
        {
            return true;
        }
        match tag {
            MarkupTags::HTML(t) => match t {
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
            },
            _ => false,
        }
    }

    pub fn is_table_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => match t {
                HTMLTags::Tfoot
                | HTMLTags::Tbody
                | HTMLTags::Thead
                | HTMLTags::Th
                | HTMLTags::Tr
                | HTMLTags::Td
                | HTMLTags::Table => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_block_text_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => match t {
                HTMLTags::Script | HTMLTags::Noscript | HTMLTags::Style | HTMLTags::Pre => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_f_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => match t {
                HTMLTags::Form
                | HTMLTags::Footer
                | HTMLTags::Figure
                | HTMLTags::Figcaption
                | HTMLTags::Fieldset => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_d_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => match t {
                HTMLTags::Details
                | HTMLTags::Dialog
                | HTMLTags::Dd
                | HTMLTags::Div
                | HTMLTags::Dt => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_header_tag(tag: MarkupTags) -> bool {
        match tag {
            MarkupTags::HTML(t) => match t {
                HTMLTags::H1
                | HTMLTags::H2
                | HTMLTags::H3
                | HTMLTags::H4
                | HTMLTags::H5
                | HTMLTags::H6
                | HTMLTags::Header
                | HTMLTags::Hgroup => true,
                _ => false,
            },
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

    /// raw_peek lets you pull the amoune of str tokens
    /// by the provided `by` size from the actual cursor
    /// not the peek cursor which allows you to
    /// ignore the peek cursor position to check
    /// potential data that the peek cursor may
    /// be hind the position of the peek cursor.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    pub fn raw_peek(&mut self, by: usize) -> Option<&'a str> {
        if self.pos + by > self.content.len() {
            return None;
        }
        Some(&self.content[self.pos..(self.pos + by)])
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
            self.peek_pos += by;
            return Some(res);
        }
        None
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a step and returns the next
    /// token string.
    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace"))]
    pub fn peek_next(&mut self) -> Option<&'a str> {
        if let Some(res) = self.peek_slice(1) {
            self.peek_pos += 1;
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

        self.peek_pos -= by;
        Some(&self.content[self.peek_pos..(self.peek_pos + by)])
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
        Some(&self.content[self.peek_pos..(self.peek_pos + by)])
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
    pub fn take_with_amount(&mut self, by: usize) -> Option<&'a str> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }

        let res = Some(&self.content[self.pos..(self.peek_pos + by)]);
        self.pos = self.peek_pos;
        res
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e str[position_cursor...peek_cursor].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    pub fn take(&mut self) -> Option<&'a str> {
        self.take_with_amount(0)
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

pub struct HTMLParser {
    allowed_tag_symbols: &'static [char],
    pub handle_as_block_text: CheckTag,
}

static VALID_TAG_NAME_SYMBOLS: &[char] = &['@', '-', '_'];

impl Default for HTMLParser {
    fn default() -> Self {
        Self {
            allowed_tag_symbols: VALID_TAG_NAME_SYMBOLS,
            handle_as_block_text: MarkupTags::is_block_text_tag,
        }
    }
}

static FRAME_FLAG_TAG: &'static str = "documentfragmentcontainer";

fn wrap_data_in_framer_tag<'a>(data: &'a str) -> String {
    format!("<{}>{}</{}>", FRAME_FLAG_TAG, data, FRAME_FLAG_TAG)
}

// https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
static MARKUP_PATTERN_REGEXP: Lazy<Regex> = lazy_regex!(
    r#"<!--[\s\S]*?-->|<(\/?)([a-zA-Z][-.:0-9_a-zA-Z]*)((?:\s+[^>]*?(?:(?:'[^']*')|(?:"[^"]*"))?)*)\s*(\/?)>"#
); // use with ".."/g"
static ATTRIBUTE_PATTERN: Lazy<Regex> =
    lazy_regex!(r#"/(?:^|\s)(id|class)\s*=\s*((?:'[^']*')|(?:"[^"]*")|\S+)"#i); // use with /../gi

static SPACE_STR: &'static str = " ";
static TAG_OPEN_BRACKET: &'static str = "<";
static TAG_CLOSED_BRACKET: &'static str = ">";
static TAG_CLOSED_SLASH: &'static str = "/";
static TAG_NAME_SPACE_END: &'static str = " ";

fn create_range(start: usize, end: usize) -> (usize, usize) {
    let offset = FRAME_FLAG_TAG.len() + 2;
    return (start - offset, end - offset);
}

impl HTMLParser {
    pub fn new(allowed_tag_symbols: &'static [char], handle_as_block_text: CheckTag) -> Self {
        Self {
            allowed_tag_symbols,
            handle_as_block_text,
        }
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    pub fn parse<'a>(&self, input: &'a str) -> ParsingResult<Stack> {
        let data = wrap_data_in_framer_tag(input);
        let mut accumulator = Accumulator::new(data.as_str());

        let mut stacks: Vec<Stack> = vec![];

        while let Some(next) = accumulator.peek(1) {
            if "<" == next
                && (accumulator.peek(2).unwrap()[1..])
                    .chars()
                    .all(char::is_alphanumeric)
            {
                // peek one step and skip
                accumulator.peek_next();
                accumulator.skip();

                //
                match self.parse_elem(&mut accumulator) {
                    Ok(elem) => stacks.push(elem),
                    Err(err) => return Err(err),
                }
            }

            break;
        }

        Err(ParsingTagError::FailedParsing)
    }

    fn is_tag_name_data<'a>(&self, text: &'a str) -> bool {
        let is_aphanum = text.chars().any(char::is_alphanumeric);
        let is_allowed_symbol = text.chars().any(|t| self.allowed_tag_symbols.contains(&t));
        is_aphanum || (is_allowed_symbol && is_aphanum)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn parse_elem<'a>(&self, acc: &'a mut Accumulator) -> ParsingResult<Stack> {
        let mut elem = Stack::empty();

        while let Some(next) = acc.peek_next() {
            println!("Tag: {:?}", next);

            if !self.is_tag_name_data(next) {
                if next != TAG_NAME_SPACE_END && next != TAG_CLOSED_BRACKET {
                    return Err(ParsingTagError::ExpectingSpaceAfterTagName);
                }

                acc.unpeek_next();

                match MarkupTags::from_str(acc.take().unwrap()) {
                    Ok(tag) => elem.tag.replace(tag),
                    Err(err) => return Err(err),
                };

                acc.peek_next();

                println!("Tag Name: {:?}", elem.tag);

                if next == TAG_CLOSED_BRACKET {
                    return Ok(elem);
                }

                match self.parse_elem_attribute(&mut elem, acc) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }
            }
            println!("Acceptable symbol: {:?}", next);
        }

        Err(ParsingTagError::FailedParsing)
    }

    #[cfg_attr(any(debug_trace), debug_trace::instrument(level = "trace", skip(self)))]
    fn parse_elem_attribute<'a>(
        &self,
        stack: &mut Stack,
        acc: &'a mut Accumulator,
    ) -> ParsingResult<()> {
        todo!()
    }
}

#[cfg(test)]
mod html_parser_test {

    use super::*;

    #[test]
    fn test_basic_html_parsing_single_node() {
        let parser = HTMLParser::default();
        let content = "<div>hello</div>";

        let result = parser.parse(content);
        assert!(matches!(result, ParsingResult::Ok(_)));

        let parsed = result.unwrap();
        assert_eq!(
            parsed,
            Stack {
                tag: Some(MarkupTags::HTML(HTMLTags::Div)),
                closed: true,
                start_range: None,
                end_range: None,
                attrs: vec![],
                children: vec![Stack {
                    tag: Some(MarkupTags::Text("hello".to_string())),
                    closed: false,
                    attrs: vec![],
                    children: vec![],
                    start_range: None,
                    end_range: None,
                }]
            }
        )
    }
}
