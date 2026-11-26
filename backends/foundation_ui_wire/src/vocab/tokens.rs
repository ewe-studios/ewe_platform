use alloc::{string::String, vec::Vec};
use core::fmt::Write;

/// Design token names of an application. The first identifier is 1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenTable {
    colors: Vec<String>,
    fonts: Vec<String>,
}

impl TokenTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a color custom property and return its identifier.
    ///
    /// # Panics
    ///
    /// Panics when the color table cannot represent another identifier.
    pub fn color(&mut self, css_var: impl Into<String>) -> u32 {
        add_token(&mut self.colors, css_var.into())
    }

    /// Append a font custom property and return its identifier.
    ///
    /// # Panics
    ///
    /// Panics when the font table cannot represent another identifier.
    pub fn font(&mut self, css_var: impl Into<String>) -> u32 {
        add_token(&mut self.fonts, css_var.into())
    }

    /// Return the first identifier for a color custom property.
    #[must_use]
    pub fn color_id(&self, css_var: &str) -> Option<u32> {
        find_token(&self.colors, css_var)
    }

    /// Return the first identifier for a font custom property.
    #[must_use]
    pub fn font_id(&self, css_var: &str) -> Option<u32> {
        find_token(&self.fonts, css_var)
    }

    /// Write aliases for the color and font custom properties.
    #[must_use]
    pub fn to_css_aliases(&self) -> String {
        let mut css = String::from(":root{");
        for (kind, names) in [("color", &self.colors), ("font", &self.fonts)] {
            for (index, name) in names.iter().enumerate() {
                let id = index + 1;
                write!(css, "--ewe-{kind}-{id}:var({name});")
                    .expect("Writing to a String cannot fail");
            }
        }
        css.push('}');
        css
    }
}

fn add_token(names: &mut Vec<String>, name: String) -> u32 {
    let id = u32::try_from(names.len() + 1).expect("The design token table is full");
    names.push(name);
    id
}

fn find_token(names: &[String], name: &str) -> Option<u32> {
    u32::try_from(names.iter().position(|entry| entry == name)? + 1).ok()
}
