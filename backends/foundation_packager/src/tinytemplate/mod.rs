//! ## TinyTemplate
//!
//! TinyTemplate is a minimal templating library originally designed for use in [Criterion.rs].
//! It deliberately does not provide all of the features of a full-power template engine, but in
//! return it provides a simple API, clear templating syntax, decent performance and very few
//! dependencies.
//!
//! Vendored from https://github.com/ewe-studios/TinyTemplate.git as an in-tree module.

mod compiler;
pub mod error;
mod instruction;
pub mod syntax;
mod template;

use error::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write;
use template::Template;

/// Type alias for closures which can be used as value formatters.
pub type ValueFormatter = dyn Fn(&Value, &mut String) -> Result<()>;

/// Appends `value` to `output`, performing HTML-escaping in the process.
pub fn escape(value: &str, output: &mut String) {
    let value_str = value;
    let mut last_emitted = 0;
    for (i, ch) in value.bytes().enumerate() {
        match ch as char {
            '<' | '>' | '&' | '\'' | '"' => {
                output.push_str(&value_str[last_emitted..i]);
                let s = match ch as char {
                    '>' => "&gt;",
                    '<' => "&lt;",
                    '&' => "&amp;",
                    '\'' => "&#39;",
                    '"' => "&quot;",
                    _ => unreachable!(),
                };
                output.push_str(s);
                last_emitted = i + 1;
            }
            _ => {}
        }
    }

    if last_emitted < value_str.len() {
        output.push_str(&value_str[last_emitted..]);
    }
}

/// The format function is used as the default value formatter for all values unless the user
/// specifies another. It is provided publicly so that it can be called as part of custom formatters.
pub fn format(value: &Value, output: &mut String) -> Result<()> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(b) => {
            write!(output, "{}", b)?;
            Ok(())
        }
        Value::Number(n) => {
            write!(output, "{}", n)?;
            Ok(())
        }
        Value::String(s) => {
            escape(s, output);
            Ok(())
        }
        _ => Err(unprintable_error()),
    }
}

/// Identical to [`format`](fn.format.html) except that this does not perform HTML escaping.
pub fn format_unescaped(value: &Value, output: &mut String) -> Result<()> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(b) => {
            write!(output, "{}", b)?;
            Ok(())
        }
        Value::Number(n) => {
            write!(output, "{}", n)?;
            Ok(())
        }
        Value::String(s) => {
            output.push_str(s);
            Ok(())
        }
        _ => Err(unprintable_error()),
    }
}

/// The TinyTemplate struct is the entry point for the TinyTemplate library.
pub struct TinyTemplate<'template> {
    templates: HashMap<&'template str, Template<'template>>,
    formatters: HashMap<&'template str, Box<ValueFormatter>>,
    default_formatter: &'template ValueFormatter,
}
impl<'template> TinyTemplate<'template> {
    pub fn new() -> TinyTemplate<'template> {
        let mut tt = TinyTemplate {
            templates: HashMap::default(),
            formatters: HashMap::default(),
            default_formatter: &format,
        };
        tt.add_formatter("unescaped", format_unescaped);
        tt
    }

    pub fn add_template(&mut self, name: &'template str, text: &'template str) -> Result<()> {
        let template = Template::compile(text)?;
        self.templates.insert(name, template);
        Ok(())
    }

    pub fn set_default_formatter<F>(&mut self, formatter: &'template F)
    where
        F: 'static + Fn(&Value, &mut String) -> Result<()>,
    {
        self.default_formatter = formatter;
    }

    pub fn add_formatter<F>(&mut self, name: &'template str, formatter: F)
    where
        F: 'static + Fn(&Value, &mut String) -> Result<()>,
    {
        self.formatters.insert(name, Box::new(formatter));
    }

    pub fn render<C>(&self, template: &str, context: &C) -> Result<String>
    where
        C: Serialize,
    {
        let value = serde_json::to_value(context)?;
        match self.templates.get(template) {
            Some(tmpl) => tmpl.render(
                &value,
                &self.templates,
                &self.formatters,
                self.default_formatter,
            ),
            None => Err(Error::GenericError {
                msg: format!("Unknown template '{}'", template),
            }),
        }
    }
}
impl<'template> Default for TinyTemplate<'template> {
    fn default() -> TinyTemplate<'template> {
        TinyTemplate::new()
    }
}
