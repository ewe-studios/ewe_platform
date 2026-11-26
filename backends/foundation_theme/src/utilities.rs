//! A native theme manifest maps each supported utility to its drawing property.
#[cfg(feature = "native-manifest")]
use crate::{auto_dark, GeneratedTheme};
#[cfg(feature = "native-manifest")]
use alloc::{collections::BTreeMap, format, string::String};

pub(super) fn shared_utilities(category: &str) -> &'static [(&'static str, &'static str)] {
    match category {
        "color" => &[("text", "color"), ("bg", "background")],
        "padding" | "spacing" => &[("p", "padding")],
        "font-size" => &[("text", "font-size")],
        _ => &[],
    }
}

#[cfg(feature = "native-manifest")]
impl GeneratedTheme {
    /// Export the same light and dark utility values that the browser stylesheet uses.
    /// Each entry has a property name, a light value, and a dark value.
    ///
    /// # Panics
    /// Serialization to a string cannot fail for this map of string keys and values.
    #[must_use]
    pub fn native_manifest(&self) -> String {
        let mut entries = BTreeMap::new();
        for token in self.tokens() {
            let mappings = shared_utilities(&token.category);
            let dark = if token.category == "color" {
                token
                    .dark
                    .as_deref()
                    .map(String::from)
                    .or_else(|| auto_dark(&token.light))
            } else {
                None
            };
            for (prefix, property) in mappings {
                entries
                    .entry(format!("{prefix}-{}", token.name))
                    .or_insert_with(BTreeMap::new)
                    .insert(
                        *property,
                        [
                            String::from(token.light.as_ref()),
                            dark.clone()
                                .unwrap_or_else(|| String::from(token.light.as_ref())),
                        ],
                    );
            }
        }
        serde_json::to_string(&entries).expect("A map of strings always has a JSON representation")
    }
}
