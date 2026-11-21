//! Write the browser representation of a Foundation view.
use crate::{Html, ViewError};
use foundation_ui_runtime::{
    action::ActionRef,
    view::{core_kind, property, template_types::AttrName, TokenTable},
};
use foundation_ui_wire::{
    vocab::{WEB_EVENTS, WEB_KINDS},
    LengthUnit, PropId, PropValueT,
};
use std::{borrow::Cow, fmt::Write as _, io::Write};

type Attributes = Vec<(AttrName, Cow<'static, str>)>;

pub(crate) fn write(
    html: &Html,
    tokens: &TokenTable,
    out: &mut dyn Write,
) -> Result<(), ViewError> {
    let mut html = html.clone();
    web_tree(&mut html, tokens)?;
    html.write_markup(|chunk| out.write_all(chunk.as_bytes()))
        .map_err(ViewError::Write)
}

fn valid_name(name: Option<&str>) -> Result<&str, ViewError> {
    name.filter(|s| {
        !s.is_empty()
            && s.chars().all(|c| {
                !c.is_control()
                    && !c.is_whitespace()
                    && !matches!(c, '"' | '\'' | '>' | '<' | '/' | '=')
            })
    })
    .ok_or_else(|| ViewError::Render("Invalid markup name".into()))
}

fn web_tree(node: &mut Html, tokens: &TokenTable) -> Result<(), ViewError> {
    if let Some(tag) = &node.tag {
        let name = valid_name(tag.name())?;
        if !name.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
            return Err(ViewError::Render("Invalid element name".into()));
        }
        let kind = core_kind(name);
        let mut attrs = Vec::new();
        let mut styles = String::new();
        if let Some(kind) = kind {
            let web = WEB_KINDS
                .iter()
                .find(|row| row.kind == kind)
                .ok_or_else(|| ViewError::Render(format!("Missing web element for {name}")))?;
            node.tag = Some(web.tag.into());
            attrs.push(("class".into(), web.class.into()));
            attrs.extend(
                web.attrs
                    .iter()
                    .map(|&(key, value)| (key.into(), value.into())),
            );
        }
        let local_id = node
            .attributes
            .iter()
            .find(|(key, _)| key.name() == Some("primal-id"))
            .and_then(|(_, value)| value.parse().ok());
        if local_id.is_some_and(|id| node.parts.iter().any(|part| part.marks_state_on(id))) {
            return Err(ViewError::Render(
                "This view requires signed component state".into(),
            ));
        }
        for (key, value) in std::mem::take(&mut node.attributes) {
            let name = valid_name(key.name())?;
            if name == "primal:anchor" {
                continue;
            }
            if name.eq_ignore_ascii_case("primal::state") {
                return Err(ViewError::Render(
                    "The runtime owns the state attribute".into(),
                ));
            }
            if let Some(event) = name.strip_prefix("ewe:on:") {
                let (_, web) = WEB_EVENTS
                    .iter()
                    .find(|(id, _)| {
                        id.variant_name()
                            .is_some_and(|n| n.eq_ignore_ascii_case(event))
                    })
                    .ok_or_else(|| ViewError::Render(format!("Unknown event {event}")))?;
                ActionRef::parse(&value).map_err(|e| ViewError::Render(e.to_string()))?;
                attrs.push((format!("primal:on{web}").into(), value));
            } else if kind.is_some() && !matches!(name, "primal-id" | "key") {
                let prop = property(local_id.unwrap_or_default(), name, &value, tokens)
                    .map_err(|err| ViewError::Render(err.to_string()))?;
                web_property(&mut attrs, &mut styles, prop.prop, &prop.value, value)?;
                if prop.prop == PropId::CHECKED
                    && node.tag.as_ref().and_then(|tag| tag.name()) == Some("button")
                {
                    if let PropValueT::Bool(flag) = prop.value {
                        attrs.push(("aria-pressed".into(), flag.v.to_string().into()));
                    }
                }
            } else if kind.is_none() || name != "key" {
                attrs.push((key, value));
            }
        }
        if !styles.is_empty() {
            attrs.push(("style".into(), styles.into()));
        }
        node.attributes = attrs;
    }
    for child in &mut node.children {
        web_tree(child, tokens)?;
    }
    Ok(())
}

fn web_property(
    attrs: &mut Attributes,
    styles: &mut String,
    prop: PropId,
    typed: &PropValueT,
    value: Cow<'static, str>,
) -> Result<(), ViewError> {
    let style = match (prop, typed) {
        (PropId::ENABLED | PropId::HIDDEN | PropId::CHECKED, PropValueT::Bool(flag)) => {
            let (name, visible) = match prop {
                PropId::ENABLED => ("disabled", !flag.v),
                PropId::HIDDEN => ("hidden", flag.v),
                _ => ("checked", flag.v),
            };
            if visible {
                attrs.push((name.into(), "".into()));
            }
            return Ok(());
        }
        (_, PropValueT::Text(_)) => {
            let name = match prop {
                PropId::A11Y_LABEL => "aria-label",
                PropId::SOURCE => "src",
                PropId::PLACEHOLDER => "placeholder",
                PropId::VALUE => "value",
                PropId::UTILITY_CLASSES => {
                    if let Some((_, classes)) = attrs
                        .iter_mut()
                        .find(|(key, _)| key.name() == Some("class"))
                    {
                        for class in value.split_ascii_whitespace().filter(|class| {
                            !class.starts_with("ios-") && !class.starts_with("android-")
                        }) {
                            classes.to_mut().push(' ');
                            classes
                                .to_mut()
                                .push_str(class.strip_prefix("web-").unwrap_or(class));
                        }
                    }
                    return Ok(());
                }
                _ => return Err(ViewError::Render("Unknown web text property".into())),
            };
            attrs.push((name.into(), value));
            return Ok(());
        }
        (_, PropValueT::Length(length)) => {
            let length = length
                .v
                .as_ref()
                .ok_or_else(|| ViewError::Render("Missing length".into()))?;
            let name = match prop {
                PropId::PADDING => "padding",
                PropId::SPACING => "gap",
                PropId::HEIGHT => "height",
                PropId::WIDTH => "width",
                _ => return Err(ViewError::Render("Unknown web length property".into())),
            };
            let length = match length.unit {
                LengthUnit::Auto => "auto".into(),
                LengthUnit::Percent => format!("{}%", length.value),
                _ => format!("{}px", length.value),
            };
            (name, length)
        }
        (PropId::AXIS, _) => (
            "flex-direction",
            if value == "horizontal" {
                "row"
            } else {
                "column"
            }
            .into(),
        ),
        (PropId::ALIGN, _) => (
            "align-items",
            match value.as_ref() {
                "start" => "flex-start",
                "end" => "flex-end",
                other => other,
            }
            .into(),
        ),
        (PropId::FONT, PropValueT::FontToken(token)) => {
            ("font", format!("var(--ewe-font-{})", token.id))
        }
        (PropId::COLOR | PropId::BACKGROUND, PropValueT::ColorToken(token)) => (
            if prop == PropId::COLOR {
                "color"
            } else {
                "background-color"
            },
            format!("var(--ewe-color-{})", token.id),
        ),
        _ => return Err(ViewError::Render("Unknown web property".into())),
    };
    write!(styles, "{}:{};", style.0, style.1).expect("writing to a string cannot fail");
    Ok(())
}
