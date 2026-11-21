//! Turns a typed action function into a value that the host can register.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    ext::IdentExt, parse::Parser, punctuated::Punctuated, Error, FnArg, GenericArgument, Ident,
    Item, ItemFn, Pat, PatType, PathArguments, ReturnType, Signature, Token, Type, TypeReference,
};

use crate::crate_paths::foundation_ui_runtime_path;

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_checked(attr, item).unwrap_or_else(Error::into_compile_error)
}

fn expand_checked(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let runtime = foundation_ui_runtime_path();
    let (origin, scope) = limits(attr, &runtime)?;
    let Item::Fn(function) = syn::parse2(item.clone())? else {
        return Err(Error::new_spanned(
            item,
            "#[action] can only be used on a function",
        ));
    };
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = function;
    let [context, event] = parameters(&sig)?;
    let Type::Reference(TypeReference {
        lifetime: None,
        mutability: None,
        elem: context_type,
        ..
    }) = &*context.ty
    else {
        return Err(Error::new_spanned(
            &context.ty,
            "the first parameter of an action must be a shared reference, for example &Context",
        ));
    };
    let (state, input) = interaction_types(&event.ty).ok_or_else(|| {
        Error::new_spanned(
            &event.ty,
            "the second parameter must be `Interaction<State>` or `Interaction<State, Input>`",
        )
    })?;
    let input = input.map_or_else(|| quote!(::std::string::String), |typ| quote!(#typ));
    let ReturnType::Type(_, result) = &sig.output else {
        return Err(Error::new_spanned(
            &sig,
            "an action must return `Result<(), ErrorTrace<ActionError>>`",
        ));
    };
    let name = &sig.ident;
    let action_name = name.unraw().to_string();
    let context_name = &context.pat;
    let gates_and_lints = attrs.iter().filter(|attr| {
        attr.path().is_ident("cfg")
            || attr.path().is_ident("allow")
            || attr.path().is_ident("warn")
            || attr.path().is_ident("deny")
            || attr.path().is_ident("forbid")
    });

    Ok(quote! {
        #(#attrs)*
        #[allow(non_camel_case_types)]
        #[derive(Clone, Copy, Debug, Default)]
        #vis struct #name;

        #(#gates_and_lints)*
        impl #runtime::action::Action<#input> for #name {
            const NAME: &'static str = #action_name;
            type State = #state;

            fn origin() -> ::core::option::Option<#runtime::action::Origin> { #origin }
            fn scope() -> #runtime::action::Scope { #scope }

            fn call<'__action>(&'__action self, #context_name: &'__action #context_type, #event)
                -> #runtime::action::ActionFuture<'__action>
            {
                ::std::boxed::Box::pin(async move {
                    ::core::convert::identity::<#result>((async move #block).await)
                })
            }
        }
    })
}

fn limits(attr: TokenStream, runtime: &TokenStream) -> syn::Result<(TokenStream, TokenStream)> {
    let invalid = |tokens: &TokenStream| {
        Error::new_spanned(
            tokens,
            format!(
                "unknown action limit `{tokens}`; use `server` or `host`, then `web` or `mobile`"
            ),
        )
    };
    let tokens = attr.clone();
    let args = Punctuated::<Ident, Token![,]>::parse_terminated
        .parse2(attr)
        .map_err(|_| invalid(&tokens))?;
    let origin = match args.first() {
        None => quote!(::core::option::Option::None),
        Some(value) if value == "server" => {
            quote!(::core::option::Option::Some(#runtime::action::Origin::Server))
        }
        Some(value) if value == "host" => {
            quote!(::core::option::Option::Some(#runtime::action::Origin::Host))
        }
        Some(value) => return Err(invalid(&quote!(#value))),
    };
    let scope = match args.get(1) {
        None => quote!(#runtime::action::Scope::All),
        Some(value) if value == "web" => quote!(#runtime::action::Scope::Web),
        Some(value) if value == "mobile" => quote!(#runtime::action::Scope::Mobile),
        Some(value) => return Err(invalid(&quote!(#value))),
    };
    if let Some(extra) = args.get(2) {
        return Err(Error::new_spanned(
            extra,
            "#[action] takes at most an origin and a scope",
        ));
    }
    if args.trailing_punct() {
        return Err(invalid(&tokens));
    }
    Ok((origin, scope))
}

fn parameters(sig: &Signature) -> syn::Result<[&PatType; 2]> {
    if sig.asyncness.is_none() {
        return Err(Error::new_spanned(
            sig.fn_token,
            "#[action] needs an `async fn`",
        ));
    }
    let qualifier = sig
        .constness
        .map(|token| quote!(#token))
        .or_else(|| sig.unsafety.map(|token| quote!(#token)))
        .or_else(|| sig.abi.as_ref().map(|abi| quote!(#abi)));
    if let Some(token) = qualifier {
        return Err(Error::new_spanned(
            token,
            "an action cannot be `const`, `unsafe` or `extern`",
        ));
    }
    if !sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &sig.generics,
            "an action cannot have generic parameters",
        ));
    }
    if let Some(clause) = &sig.generics.where_clause {
        return Err(Error::new_spanned(
            clause,
            "an action cannot have generic parameters",
        ));
    }
    if let Some(receiver) = sig.receiver() {
        return Err(Error::new_spanned(
            receiver,
            "an action is a free function; it cannot take self",
        ));
    }
    let mut args = sig.inputs.iter();
    let (Some(FnArg::Typed(context)), Some(FnArg::Typed(event)), None, None) =
        (args.next(), args.next(), args.next(), &sig.variadic)
    else {
        let span = sig.paren_token.span.join();
        return Err(Error::new(
            span,
            "an action takes two parameters: a context reference and an Interaction<T>",
        ));
    };
    for arg in [context, event] {
        if !matches!(&*arg.pat, Pat::Ident(pat) if pat.by_ref.is_none() && pat.subpat.is_none()) {
            return Err(Error::new_spanned(
                &arg.pat,
                "an action parameter must be a plain name",
            ));
        }
    }
    Ok([context, event])
}

fn interaction_types(event: &Type) -> Option<(&Type, Option<&Type>)> {
    let Type::Path(event) = event else {
        return None;
    };
    let segment = event.path.segments.last()?;
    if segment.ident != "Interaction" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut args = arguments.args.iter();
    match (args.next(), args.next(), args.next()) {
        (Some(GenericArgument::Type(state)), None, None) => Some((state, None)),
        (Some(GenericArgument::Type(state)), Some(GenericArgument::Type(input)), None) => {
            Some((state, Some(input)))
        }
        _ => None,
    }
}
