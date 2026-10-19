//! `#[valtron::task]` — turn a transition function into a state-machine task.
//!
//! The attribute wraps a transition function (sync or `async`) in a same-named
//! struct that implements `TaskIterator` via `StateTask` (sync) or
//! `AsyncStateTask` (async). The transition keeps its name in the *value*
//! namespace, so it stays directly callable; the generated struct is a **braced**
//! struct, which lives only in the *type* namespace (no value-namespace
//! constructor) and therefore never collides with the function.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

use crate::crate_paths::foundation_core_path;

/// `#[task]` entry point — see the module docs.
pub(crate) fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(attr.span(), "#[task] takes no arguments").to_compile_error();
    }
    let func = match syn::parse2::<syn::ItemFn>(item) {
        Ok(func) => func,
        Err(err) => return err.to_compile_error(),
    };
    expand(&func).unwrap_or_else(|e| e.to_compile_error())
}

/// Last path segment of `ty`, if it is a path type.
fn last_segment(ty: &syn::Type) -> Option<&syn::PathSegment> {
    match ty {
        syn::Type::Path(tp) => tp.path.segments.last(),
        _ => None,
    }
}

/// Whether `ty` is the bare type named `name` (e.g. `Ctx`, `Option`, `TaskStatus`).
fn type_is(ty: &syn::Type, name: &str) -> bool {
    last_segment(ty).is_some_and(|seg| seg.ident == name)
}

/// The `idx`-th type argument of `ty`'s last segment (e.g. the `T` in `Option<T>`).
fn generic_arg(ty: &syn::Type, idx: usize) -> Option<&syn::Type> {
    let seg = last_segment(ty)?;
    match &seg.arguments {
        syn::PathArguments::AngleBracketed(args) => match args.args.iter().nth(idx) {
            Some(syn::GenericArgument::Type(t)) => Some(t),
            _ => None,
        },
        _ => None,
    }
}

/// The type of a typed `FnArg`.
fn fn_arg_type(arg: &syn::FnArg) -> Option<&syn::Type> {
    match arg {
        syn::FnArg::Typed(pt) => Some(pt.ty.as_ref()),
        syn::FnArg::Receiver(_) => None,
    }
}

/// Whether `arg` is a plain identifier pattern named `name`.
fn arg_is_named(arg: &syn::FnArg, name: &str) -> bool {
    match arg {
        syn::FnArg::Typed(pt) => matches!(&*pt.pat, syn::Pat::Ident(pi) if pi.ident == name),
        syn::FnArg::Receiver(_) => false,
    }
}

/// Token-level type equality — good enough for the "state types match" check.
fn same_type(a: &syn::Type, b: &syn::Type) -> bool {
    quote!(#a).to_string() == quote!(#b).to_string()
}

fn expand(func: &syn::ItemFn) -> syn::Result<TokenStream> {
    let name = &func.sig.ident;
    let vis = &func.vis;
    let is_async = func.sig.asyncness.is_some();

    if !func.sig.generics.params.is_empty() || func.sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &func.sig.generics,
            "#[task] does not support generic transitions (yet)",
        ));
    }

    // --- inputs: `ctx: Ctx, state: T` ---
    let inputs: Vec<&syn::FnArg> = func.sig.inputs.iter().collect();
    if inputs.len() != 2 {
        return Err(syn::Error::new_spanned(
            &func.sig.inputs,
            "#[task] transition must take exactly two arguments: `ctx: Ctx, state: T`",
        ));
    }
    let (ctx_arg, state_arg) = (inputs[0], inputs[1]);
    if !arg_is_named(ctx_arg, "ctx") {
        return Err(syn::Error::new_spanned(
            ctx_arg,
            "#[task] first argument must be named `ctx`",
        ));
    }
    let ctx_ty = fn_arg_type(ctx_arg)
        .ok_or_else(|| syn::Error::new_spanned(ctx_arg, "#[task] first argument must be `ctx: Ctx`"))?;
    if !type_is(ctx_ty, "Ctx") {
        return Err(syn::Error::new_spanned(
            ctx_ty,
            "#[task] first argument must have type `Ctx`",
        ));
    }
    let ty_t = fn_arg_type(state_arg)
        .ok_or_else(|| syn::Error::new_spanned(state_arg, "#[task] second argument must be `state: T`"))?
        .clone();

    // --- output: `Option<(T, TaskStatus<R, P>)>` ---
    let output_ty = match &func.sig.output {
        syn::ReturnType::Type(_, ty) => ty.as_ref(),
        syn::ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &func.sig,
                "#[task] transition must return `Option<(T, TaskStatus<R, P>)>`",
            ))
        }
    };
    if !type_is(output_ty, "Option") {
        return Err(syn::Error::new_spanned(
            output_ty,
            "#[task] transition must return `Option<...>`",
        ));
    }
    let inner = generic_arg(output_ty, 0).ok_or_else(|| {
        syn::Error::new_spanned(output_ty, "#[task] return must be `Option<(T, TaskStatus<R, P>)>`")
    })?;
    let elems = match inner {
        syn::Type::Tuple(t) => &t.elems,
        _ => {
            return Err(syn::Error::new_spanned(
                inner,
                "#[task] return must be a 2-tuple `(T, TaskStatus<R, P>)`",
            ))
        }
    };
    if elems.len() != 2 {
        return Err(syn::Error::new_spanned(
            inner,
            "#[task] return must be a 2-tuple `(T, TaskStatus<R, P>)`",
        ));
    }
    let ty_t_out = &elems[0];
    let task_status_ty = &elems[1];
    if !type_is(task_status_ty, "TaskStatus") {
        return Err(syn::Error::new_spanned(
            task_status_ty,
            "#[task] second tuple element must be `TaskStatus<R, P>`",
        ));
    }
    let ty_r = generic_arg(task_status_ty, 0)
        .ok_or_else(|| syn::Error::new_spanned(task_status_ty, "#[task] `TaskStatus` needs a Ready type `R`"))?
        .clone();
    let pending_ty = generic_arg(task_status_ty, 1)
        .ok_or_else(|| syn::Error::new_spanned(task_status_ty, "#[task] `TaskStatus` needs a Pending type `P`"))?;

    if !same_type(&ty_t, ty_t_out) {
        return Err(syn::Error::new_spanned(
            ty_t_out,
            "#[task] the state type in the return tuple must match the `state` argument type",
        ));
    }

    // The pending type is the transition's own `P` — sync and async share it.
    let ty_p: syn::Type = pending_ty.clone();

    let fc = foundation_core_path();

    // The core iterator the generated struct wraps.
    let core = if is_async {
        quote! { #fc::valtron::AsyncStateTask<#ty_t, #ty_r, #ty_p> }
    } else {
        quote! {
            #fc::valtron::StateTask<
                #ty_t, #ty_r, #ty_p,
                fn(#fc::valtron::Ctx, #ty_t) -> ::core::option::Option<(#ty_t, #fc::valtron::TaskStatus<#ty_r, #ty_p>)>,
            >
        }
    };

    let pending_assoc = quote! { #ty_p };

    // `new(state)` seeds the transition. `#name` in value position is the fn; the
    // braced struct introduces no value-namespace constructor, so there is no
    // collision. The async form boxes both the transition and the future (an
    // `async fn`'s future type is unnameable).
    let new_body = if is_async {
        quote! {
            #name {
                inner: #fc::valtron::AsyncStateTask::new(
                    state,
                    ::std::boxed::Box::new(move |ctx, s| ::std::boxed::Box::pin(#name(ctx, s))),
                ),
            }
        }
    } else {
        quote! {
            #name {
                inner: #fc::valtron::StateTask::new(state, #name),
            }
        }
    };

    Ok(quote! {
        // The transition itself, unchanged — still directly callable as
        // `#name(ctx, state)` (and `.await`ed in the async form).
        #func

        #[allow(non_camel_case_types)]
        #vis struct #name {
            inner: #core,
        }

        impl #name {
            pub fn new(state: #ty_t) -> Self {
                #new_body
            }
        }

        impl #fc::valtron::TaskIterator for #name {
            type Ready = #ty_r;
            type Pending = #pending_assoc;

            fn next_status(
                &mut self,
                ctx: #fc::valtron::Ctx,
            ) -> ::core::option::Option<#fc::valtron::TaskStatus<Self::Ready, Self::Pending>> {
                self.inner.next_status(ctx)
            }
        }
    })
}
