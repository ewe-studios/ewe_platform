//! WHY: `#[derive(ThemeTokens)]` is the LEGACY compile-time theme form
//! (decision 020). It is kept for back-compat, but the CSS it emits now comes
//! from the SAME generator as `theme!{}` and the runtime builder
//! (`foundation_theme::theme_css`, decision 021) — one source of truth.
//!
//! WHAT: A derive over a struct of `#[token(...)]` fields. The spec's sketch
//! used nested non-Rust literals; the real syntax carries everything in
//! attributes (valid Rust):
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
//! Generates `Theme::new()`, `Theme::CSS: &'static str`, and `css_string()`.
//!
//! HOW: Parse the `#[token]` attrs into `foundation_theme::ThemeToken`s, call
//! `theme_css` at expansion time, embed the result as a `'static` literal.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

use foundation_theme::{theme_css, ThemeToken};

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

    let css = theme_css(&tokens);
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

fn parse_token_attr(field: &syn::Field) -> Result<Option<ThemeToken>, syn::Error> {
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
        let category =
            category.ok_or_else(|| syn::Error::new(span, "#[token] needs category = \"…\""))?;
        let light = light
            .ok_or_else(|| syn::Error::new(span, "#[token] needs light = \"…\" or value = \"…\""))?;
        let name = field
            .ident
            .as_ref()
            .expect("named")
            .to_string()
            .replace('_', "-");
        return Ok(Some(ThemeToken::owned(name, category, light, dark)));
    }
    Ok(None)
}
