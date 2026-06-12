//! WHY: Design tokens should be typed Rust, not stringly CSS — and the CSS
//! they imply (custom properties, dark-mode overrides, utility classes) must
//! be generated at COMPILE time (decision 020: zero runtime CSS processing).
//!
//! WHAT: `#[derive(ThemeTokens)]` over a struct of `#[token(...)]` fields.
//! The spec's sketch uses nested non-Rust literals; the REAL syntax carries
//! everything in attributes (valid Rust, same three levels of dark control):
//!
//! ```ignore
//! #[derive(ThemeTokens)]
//! struct AppTheme {
//!     #[token(category = "color", light = "#3b82f6", dark = "#60a5fa")] // explicit dark
//!     primary: (),
//!     #[token(category = "color", light = "#10b981")]                   // dark auto-derived
//!     secondary: (),
//!     #[token(category = "spacing", value = "16px")]                    // mode-independent
//!     md: (),
//! }
//! ```
//!
//! Generates `Theme::new()`, `Theme::CSS: &'static str`, and
//! `css_string(&self)`: a `:root` block of custom properties, a
//! `@media (prefers-color-scheme: dark)` override block (explicit values
//! verbatim; missing dark colors auto-derived at ~80% luminance), per-token
//! utility classes (`.bg-*`/`.text-*`/`.border-*`, `.p-*`/`.m-*`,
//! `.rounded-*`, `.shadow-*`), and the built-in utility set (feature 09 §7.2).
//!
//! HOW: Pure string assembly inside the macro — the output is ONE `'static`
//! literal in the binary.

use core::fmt::Write as _;

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

struct Token {
    name: String,
    category: String,
    light: String,
    dark: Option<String>,
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
.border-2 { border-width: 2px; }\n\
.hidden { display: none; }\n\
.visible { visibility: visible; }\n\
.opacity-0 { opacity: 0; }\n\
.opacity-100 { opacity: 1; }\n";

pub fn theme_tokens_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = match syn::parse2(input) {
        Ok(ast) => ast,
        Err(err) => return err.to_compile_error(),
    };
    let name = &ast.ident;
    let syn::Data::Struct(data) = &ast.data else {
        return syn::Error::new(ast.span(), "ThemeTokens derives on structs only")
            .to_compile_error();
    };

    let mut tokens = Vec::new();
    let mut field_inits = Vec::new();
    for field in &data.fields {
        let Some(ident) = &field.ident else {
            return syn::Error::new(field.span(), "ThemeTokens requires named fields")
                .to_compile_error();
        };
        field_inits.push(quote! { #ident: () });
        match parse_token_attr(field) {
            Ok(Some(token)) => tokens.push(token),
            Ok(None) => {
                return syn::Error::new(
                    field.span(),
                    "every ThemeTokens field needs #[token(category = \"…\", light/value = \"…\")]",
                )
                .to_compile_error()
            }
            Err(err) => return err.to_compile_error(),
        }
    }

    let css = build_css(&tokens);
    quote! {
        impl #name {
            /// The full theme stylesheet, generated at compile time.
            pub const CSS: &'static str = #css;

            #[must_use]
            pub fn new() -> Self {
                Self { #(#field_inits),* }
            }

            /// The pre-generated theme CSS (custom properties + dark
            /// overrides + utility classes).
            #[must_use]
            pub fn css_string(&self) -> &'static str {
                Self::CSS
            }
        }

        impl ::core::default::Default for #name {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}

fn parse_token_attr(field: &syn::Field) -> Result<Option<Token>, syn::Error> {
    for attr in &field.attrs {
        if !attr.path().is_ident("token") {
            continue;
        }
        let mut category = None;
        let mut light = None;
        let mut dark = None;
        attr.parse_nested_meta(|meta| {
            let value: syn::LitStr = meta.value()?.parse()?;
            if meta.path.is_ident("category") {
                category = Some(value.value());
            } else if meta.path.is_ident("light") || meta.path.is_ident("value") {
                light = Some(value.value());
            } else if meta.path.is_ident("dark") {
                dark = Some(value.value());
            } else {
                return Err(meta.error("expected category/light/value/dark"));
            }
            Ok(())
        })?;
        let span = field.span();
        let category = category
            .ok_or_else(|| syn::Error::new(span, "#[token] needs category = \"…\""))?;
        let light = light
            .ok_or_else(|| syn::Error::new(span, "#[token] needs light = \"…\" or value = \"…\""))?;
        return Ok(Some(Token {
            name: field.ident.as_ref().expect("named").to_string().replace('_', "-"),
            category,
            light,
            dark,
        }));
    }
    Ok(None)
}

fn build_css(tokens: &[Token]) -> String {
    let var_name = |t: &Token| format!("--{}-{}", t.category, t.name);

    let mut css = String::from(":root {\n");
    for token in tokens {
        let _ = writeln!(css, "  {}: {};", var_name(token), token.light);
    }
    css.push_str("}\n");

    // Dark block: colors only — explicit where given, auto-derived otherwise.
    let dark_entries: Vec<String> = tokens
        .iter()
        .filter(|t| t.category == "color")
        .filter_map(|t| {
            let value = t
                .dark
                .clone()
                .or_else(|| auto_dark(&t.light))?;
            Some(format!("    {}: {};\n", var_name(t), value))
        })
        .collect();
    if !dark_entries.is_empty() {
        css.push_str("@media (prefers-color-scheme: dark) {\n  :root {\n");
        for entry in &dark_entries {
            css.push_str(entry);
        }
        css.push_str("  }\n}\n");
    }

    // Utility classes per category (feature 09 §7.1).
    for token in tokens {
        let var = var_name(token);
        let name = &token.name;
        match token.category.as_str() {
            "color" => {
                let _ = writeln!(css, ".bg-{name} {{ background-color: var({var}); }}");
                let _ = writeln!(css, ".text-{name} {{ color: var({var}); }}");
                let _ = writeln!(css, ".border-{name} {{ border-color: var({var}); }}");
            }
            "spacing" => {
                let _ = writeln!(css, ".p-{name} {{ padding: var({var}); }}");
                let _ = writeln!(css, ".m-{name} {{ margin: var({var}); }}");
            }
            "radius" => {
                let _ = writeln!(css, ".rounded-{name} {{ border-radius: var({var}); }}");
            }
            "shadow" => {
                let _ = writeln!(css, ".shadow-{name} {{ box-shadow: var({var}); }}");
            }
            _ => {}
        }
    }
    css.push_str(BUILTIN_UTILITIES);
    css
}

/// Auto-derive a dark value from a light `#rrggbb` (or `#rgb`) color:
/// ~80% luminance (the spec's worked example within rounding). Non-hex
/// values return None — no auto-generation for non-colors.
fn auto_dark(light: &str) -> Option<String> {
    let hex = light.strip_prefix('#')?;
    let expand = |s: &str| -> Option<Vec<u8>> {
        match s.len() {
            3 => s
                .chars()
                .map(|c| u8::from_str_radix(&format!("{c}{c}"), 16).ok())
                .collect(),
            6 => (0..3)
                .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
                .collect(),
            _ => None,
        }
    };
    let rgb = expand(hex)?;
    let darkened: Vec<u8> = rgb
        .iter()
        .map(|&v| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let out = (f32::from(v) * 0.8).round() as u8;
            out
        })
        .collect();
    Some(format!("#{:02x}{:02x}{:02x}", darkened[0], darkened[1], darkened[2]))
}
