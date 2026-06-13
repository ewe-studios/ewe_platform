//! WHY: The headline theme API (spec-39 decision 021). One function-like macro
//! that reads like a struct, so users declare design tokens without learning
//! attribute/derive mechanics — a function-like macro CAN hold values inline
//! (a `#[derive]` cannot; see decision 020/021).
//!
//! WHAT: `theme! { colors { … } spacing { … } … }` → a `const`-capable
//! `foundation_theme::GeneratedTheme` carrying the token table AND the
//! compile-time-generated `&'static str` CSS.
//!
//! HOW: Parse the fixed block set (unknown blocks are a compile error) into
//! `foundation_theme::ThemeToken`s, call the SHARED `theme_css` at expansion
//! time, and emit `GeneratedTheme::from_static(<css literal>, &[<tokens>])`.
//! The token slice is rvalue-static-promoted (all `const fn` constructors).
//!
//! Value grammar per entry `name: <value>`:
//! - a string literal — verbatim CSS (`primary: "#3b82f6"`, multi-part values);
//! - a bare suffixed literal — stringified, no quotes (`md: 16px`, `fast:
//!   250ms`, `base: 1.5rem`);
//! - the brace form `name: { light: <v>, dark: <v> }` — explicit dark (a plain
//!   value auto-derives dark for colors).

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{braced, Ident, Lit, Token};

use foundation_theme::{theme_css, ThemeToken};

use crate::crate_paths::foundation_theme_path;

/// Map a block name to its token category. `None` ⇒ unknown block (error).
fn block_category(name: &str) -> Option<&'static str> {
    Some(match name {
        "colors" | "color" => "color",
        "spacing" => "spacing",
        "padding" | "paddings" => "padding",
        "margin" | "margins" => "margin",
        "radius" | "radii" => "radius",
        "shadow" | "shadows" => "shadow",
        "font_size" | "font_sizes" => "font-size",
        "animation" | "animations" => "animation",
        _ => return None,
    })
}

struct ThemeInput {
    tokens: Vec<ThemeToken>,
}

impl Parse for ThemeInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut tokens = Vec::new();
        while !input.is_empty() {
            let block_name: Ident = input.parse()?;
            let category = block_category(&block_name.to_string()).ok_or_else(|| {
                syn::Error::new(
                    block_name.span(),
                    format!(
                        "unknown theme block `{block_name}` — expected one of: colors, \
                         spacing, padding, margin, radius, shadow, font_size, animation"
                    ),
                )
            })?;

            let content;
            braced!(content in input);
            while !content.is_empty() {
                let entry: Ident = content.parse()?;
                content.parse::<Token![:]>()?;
                let name = entry.to_string().replace('_', "-");
                let (light, dark) = parse_value(&content)?;
                tokens.push(ThemeToken::owned(name, category, light, dark));
                if content.is_empty() {
                    break;
                }
                content.parse::<Token![,]>()?;
            }
            // Blocks sit back-to-back; tolerate an optional comma between them.
            let _ = input.parse::<Token![,]>();
        }
        Ok(Self { tokens })
    }
}

/// Parse a value: either `{ light: <v>, dark: <v> }` or a single scalar.
fn parse_value(input: ParseStream) -> syn::Result<(String, Option<String>)> {
    if input.peek(syn::token::Brace) {
        let content;
        braced!(content in input);
        let mut light = None;
        let mut dark = None;
        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            let value = parse_scalar(&content)?;
            match key.to_string().as_str() {
                "light" => light = Some(value),
                "dark" => dark = Some(value),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("expected `light` or `dark`, found `{other}`"),
                    ))
                }
            }
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        let light = light.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "value block needs a `light: …` value")
        })?;
        Ok((light, dark))
    } else {
        Ok((parse_scalar(input)?, None))
    }
}

/// A single value token → its CSS string. String literals are used verbatim;
/// any other literal (`16px`, `250ms`, `1.5rem`, `2`) is reproduced as source.
fn parse_scalar(input: ParseStream) -> syn::Result<String> {
    let lit: Lit = input.parse().map_err(|_| {
        input.error("expected a CSS value: a string literal, or a bare token like `16px`")
    })?;
    Ok(match &lit {
        Lit::Str(s) => s.value(),
        other => quote!(#other).to_string(),
    })
}

pub fn theme(input: TokenStream) -> TokenStream {
    let parsed = match syn::parse2::<ThemeInput>(input) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error(),
    };

    let ft = foundation_theme_path();
    // The SAME generator the runtime builder and the derive use.
    let css = theme_css(&parsed.tokens);

    let token_exprs = parsed.tokens.iter().map(|t| {
        let name = t.name.as_ref();
        let category = t.category.as_ref();
        let light = t.light.as_ref();
        let dark = match &t.dark {
            Some(d) => {
                let d = d.as_ref();
                quote! { ::core::option::Option::Some(#d) }
            }
            None => quote! { ::core::option::Option::None },
        };
        quote! { #ft::ThemeToken::from_static(#name, #category, #light, #dark) }
    });

    // The token slice goes through a named `const` (not an inline `&[…]`):
    // `ThemeToken` holds a `Cow`, whose `Owned` variant needs `Drop`, so the
    // inline array would NOT rvalue-promote to `'static`. A `const` item gives
    // it static storage and still works in both runtime and `const` position.
    quote! {
        {
            const __THEME_TOKENS: &[#ft::ThemeToken] = &[ #(#token_exprs),* ];
            #ft::GeneratedTheme::from_static(#css, __THEME_TOKENS)
        }
    }
}
