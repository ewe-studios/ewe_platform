use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{ItemFn, LitStr, Token};

/// WHY: We need to parse `name = "..."` and `desc = "..."` from the attribute
/// arguments to validate they are present and correctly typed.
///
/// WHAT: Represents a single `key = "value"` pair in the attribute arguments.
///
/// HOW: Implements `syn::Parse` to extract ident + string literal pairs.
struct AttrArg {
    key: syn::Ident,
    value: LitStr,
}

impl Parse for AttrArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let key: syn::Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value: LitStr = input.parse().map_err(|_| {
            syn::Error::new(key.span(), format!("`{key}` must be a string literal"))
        })?;
        Ok(Self { key, value })
    }
}

/// WHY: The attribute takes multiple key-value pairs separated by commas,
/// so we need a container that parses the full comma-separated list.
///
/// WHAT: A collection of `key = "value"` pairs parsed from the attribute.
///
/// HOW: Uses `Punctuated` to parse comma-separated `AttrArg` items.
struct AttrArgs {
    args: Punctuated<AttrArg, Token![,]>,
}

impl Parse for AttrArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::parse_terminated(input)?;
        Ok(Self { args })
    }
}

/// WHY: Centralizes the proc macro logic outside of `lib.rs` for testability
/// and separation of concerns.
///
/// WHAT: Validates the `wasm_entrypoint` attribute and returns the original
/// function unchanged (marker-only macro).
///
/// HOW: Parses attribute args for `name` and `desc`, verifies the item is
/// a function, then emits the function unmodified.
///
/// # Errors
///
/// Returns compile errors if:
/// - Applied to a non-function item
/// - Missing `name` attribute
/// - Missing `desc` attribute
/// - `name` or `desc` is not a string literal
pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand_inner(attr, item) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    }
}

fn expand_inner(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let args: AttrArgs = syn::parse2(attr)?;

    // Verify the item is a function
    let func: ItemFn = syn::parse2(item).map_err(|_| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "wasm_entrypoint can only be applied to functions",
        )
    })?;

    // Extract name and desc
    let mut name: Option<LitStr> = None;
    let mut desc: Option<LitStr> = None;

    for arg in &args.args {
        let key_str = arg.key.to_string();
        match key_str.as_str() {
            "name" => name = Some(arg.value.clone()),
            "desc" => desc = Some(arg.value.clone()),
            other => {
                return Err(syn::Error::new(
                    arg.key.span(),
                    format!("unknown attribute `{other}`, expected `name` or `desc`"),
                ));
            }
        }
    }

    if name.is_none() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing required attribute `name`",
        ));
    }

    if desc.is_none() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing required attribute `desc`",
        ));
    }

    // Return the function unchanged — this is a marker-only macro
    Ok(quote! { #func })
}
