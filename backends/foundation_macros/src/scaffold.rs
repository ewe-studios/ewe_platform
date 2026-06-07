use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, FnArg, GenericArgument, ImplItem, ItemImpl, ItemStruct, PathArguments,
    Receiver, Token, Type,
};

// ── #[scaffoldable] — marks an impl block for automatic forwarding ──

#[allow(clippy::needless_pass_by_value)]
pub fn scaffoldable(_attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let impl_block: ItemImpl = match syn::parse2(item.clone()) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error(),
    };

    // Extract the inner type name
    let Some(type_name) = extract_type_name(&impl_block.self_ty) else {
        return syn::Error::new(
            impl_block.self_ty.span(),
            "#[scaffoldable] requires a simple type name (not a trait impl)",
        )
        .to_compile_error();
    };

    // Collect all pub fn method signatures
    let mut method_templates = Vec::new();
    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            // Only pub methods
            if !is_pub(&method.vis) {
                continue;
            }

            let sig = &method.sig;
            let ident = &sig.ident;
            let output = &sig.output;
            let generics = &sig.generics;
            let asyncness = &sig.asyncness;
            let unsafety = &sig.unsafety;
            let abi = &sig.abi;

            // Get the self receiver
            let receiver = sig.inputs.iter().find_map(|arg| match arg {
                FnArg::Receiver(r) => Some(r.clone()),
                FnArg::Typed(_) => None,
            });

            // Collect parameters (excluding self)
            let params: Punctuated<FnArg, Token![,]> = sig
                .inputs
                .iter()
                .filter(|arg| !matches!(arg, FnArg::Receiver(_)))
                .cloned()
                .collect();

            // Collect argument names (excluding self) — use &Box<Pat>
            let arg_names: Vec<&syn::Pat> = sig
                .inputs
                .iter()
                .filter_map(|arg| match arg {
                    FnArg::Typed(pat) => Some(pat.pat.as_ref()),
                    FnArg::Receiver(_) => None,
                })
                .collect();

            // Check for self receiver type
            let has_self = sig.inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(_)));

            if !has_self {
                // Skip methods without self receiver — can't forward meaningfully
                continue;
            }

            // Build the forwarding call
            let receiver_call = quote! { $access.#ident(#(#arg_names),*) };

            let template = {
                let recv = receiver.unwrap();
                // Replace `self` in the receiver with `$self`
                let recv_tokens = replace_self_in_receiver(&recv);
                quote! {
                    #unsafety #asyncness #abi fn #ident #generics (#recv_tokens, #params) #output {
                        #receiver_call
                    }
                }
            };
            method_templates.push(template);
        }
    }

    // Generate the macro_rules! template
    let macro_name = syn::Ident::new(
        &format!("__scaffold_methods_{type_name}"),
        type_name.span(),
    );

    let generated = if method_templates.is_empty() {
        quote! {}
    } else {
        quote! {
            #[doc(hidden)]
            #[macro_export]
            macro_rules! #macro_name {
                ($self:ident, $access:expr) => {
                    #(#method_templates)*
                };
            }
        }
    };

    // Return the original impl block + the generated macro
    quote! {
        #item
        #generated
    }
}

// ── #[derive(Scaffold)] — generates forwarding methods on outer struct ──

pub fn scaffold_derive(item: TokenStream2) -> TokenStream2 {
    let input: ItemStruct = match syn::parse2(item) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error(),
    };

    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut methods = Vec::new();

    for field in &input.fields {
        // Check for #[scaffold(field)] attribute
        if !has_attribute(&field.attrs, "scaffold") {
            continue;
        }

        let _field_name = field.ident.as_ref().unwrap();

        // Determine the access expression
        let access_expr = determine_access_expr(field);

        // Try to find the inner type name for the macro invocation
        if let Some(inner_type_name) = extract_inner_type_name(&field.ty) {
            // Generate macro invocation
            let macro_name = syn::Ident::new(
                &format!("__scaffold_methods_{inner_type_name}"),
                field.span(),
            );

            // The generated macro may be in the same crate or imported.
            // We emit it as a direct call to the macro_rules! generated by #[scaffoldable].
            methods.push(quote! {
                #macro_name!(self, #access_expr);
            });
        } else {
            // If we can't determine the inner type name, we still need to try the macro
            methods.push(quote! {
                compile_error!("inner type must have #[scaffoldable] applied to an impl block");
            });
        }
    }

    if methods.is_empty() {
        return syn::Error::new(
            input.span(),
            "#[derive(Scaffold)] requires at least one #[scaffold(field)] field",
        )
        .to_compile_error();
    }

    quote! {
        impl #impl_generics #struct_name #ty_generics #where_clause {
            #(#methods)*
        }
    }
}

// ── #[scaffold_impl] — manual impl block delegation ──

#[allow(clippy::needless_pass_by_value)]
pub fn scaffold_impl(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let impl_block: ItemImpl = match syn::parse2(item) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error(),
    };

    // Parse the via attribute from #[scaffold_impl(via = "...")]
    let block_via = parse_scaffold_impl_attr(&attr);

    // Parse block-level #[scaffold_call(call = { ... })] from impl block attrs
    let block_call = impl_block
        .attrs
        .iter()
        .find(|a| a.path().is_ident("scaffold_call"))
        .and_then(parse_call_block);

    let items = impl_block.items.clone();

    // Walk impl items, replace scaffold!() bodies with delegation
    let mut new_items = Vec::new();
    for item in items {
        if let ImplItem::Fn(mut method) = item {
            // Check for scaffold!() body
            if is_scaffold_call(&method.block) {
                // Per-method #[scaffold_call(call = { ... })] — highest priority
                let method_call_block = find_scaffold_call_block(&method.attrs);

                let delegated_body = if let Some(call_block) = method_call_block {
                    let call_expr = quote! { { #call_block } };
                    build_delegation(&method.sig, &call_expr)
                } else {
                    // Determine via expression: method scaffold_method > block scaffold_call > block via
                    let via_expr = method
                        .attrs
                        .iter()
                        .find(|a| a.path().is_ident("scaffold_method"))
                        .and_then(parse_scaffold_method_attr)
                        .or_else(|| block_call.clone().map(|b| quote! { { #b } }))
                        .or_else(|| block_via.clone());

                    let Some(via_expr) = via_expr else {
                        return syn::Error::new(
                            method.span(),
                            "scaffold!() requires a delegation target: add #[scaffold_impl(via = \"...\")], #[scaffold_method(via = \"...\")], or #[scaffold_call(call = { ... })]",
                        )
                        .to_compile_error();
                    };

                    build_delegation(&method.sig, &via_expr)
                };

                // Remove scaffold-related attributes from the output
                method.attrs.retain(|a| {
                    !a.path().is_ident("scaffold_method") && !a.path().is_ident("scaffold_call")
                });

                method.block = delegated_body;
                new_items.push(ImplItem::Fn(method));
            } else {
                // Real body — keep as override, just strip scaffold attrs
                method.attrs.retain(|a| {
                    !a.path().is_ident("scaffold_method") && !a.path().is_ident("scaffold_call")
                });
                new_items.push(ImplItem::Fn(method));
            }
        } else {
            // Associated types, consts, etc. — keep verbatim
            new_items.push(item);
        }
    }

    // Remove #[scaffold_impl] and #[scaffold_call] from the impl block attrs
    let mut new_attrs: Vec<Attribute> = impl_block.attrs.clone();
    new_attrs.retain(|a| {
        !a.path().is_ident("scaffold_impl")
            && !a.path().is_ident("scaffold_call")
            && !a.path().is_ident("scaffold_method")
    });

    // Reconstruct the impl block manually (impl_block already contains 'impl')
    let attrs_tokens: TokenStream2 = new_attrs.iter().map(quote::ToTokens::to_token_stream).collect();
    let unsafety = &impl_block.unsafety;
    let generics = &impl_block.generics;
    let self_ty = &impl_block.self_ty;
    let where_clause = &impl_block.generics.where_clause;

    let header = if let Some((defaultness, trait_path, for_token)) = &impl_block.trait_ {
        let d = defaultness.as_ref().map(quote::ToTokens::to_token_stream);
        quote! { #d #trait_path #for_token #self_ty }
    } else {
        quote! { #self_ty }
    };

    quote! {
        #attrs_tokens
        #unsafety impl #generics #header #where_clause {
            #(#new_items)*
        }
    }
}

// ── Helpers ──

/// Replace `self` with `$self` in a receiver (for `macro_rules`! template)
fn replace_self_in_receiver(recv: &Receiver) -> proc_macro2::TokenStream {
    let mutability = &recv.mutability;
    let reference = &recv.reference;
    if reference.is_some() || mutability.is_some() {
        // &self, &mut self
        let refs = reference.as_ref().map(|(amp, lt)| quote! { #amp #lt });
        if mutability.is_some() {
            quote! { #refs mut $self }
        } else {
            quote! { #refs $self }
        }
    } else {
        // self (by value)
        quote! { $self }
    }
}

fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn extract_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

fn extract_inner_type_name(ty: &Type) -> Option<String> {
    // Unwrap Arc<T>, Box<T>, Mutex<T>, RwLock<T>, Rc<T>, RefCell<T> to get T
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                    // Check for nested wrappers (Arc<Mutex<T>>)
                    if let Type::Path(inner_path) = inner_ty {
                        if let Some(inner_segment) = inner_path.path.segments.last() {
                            if let PathArguments::AngleBracketed(inner_args) =
                                &inner_segment.arguments
                            {
                                if let Some(GenericArgument::Type(innermost_ty)) =
                                    inner_args.args.first()
                                {
                                    return extract_type_name(innermost_ty);
                                }
                            }
                        }
                    }
                    return extract_type_name(inner_ty);
                }
            }
        }
        return extract_type_name(ty);
    }
    None
}

fn has_attribute(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}

fn determine_access_expr(field: &syn::Field) -> proc_macro2::TokenStream {
    let field_name = field.ident.as_ref().unwrap();

    // Check for #[scaffold_call(call = { ... })]
    for attr in &field.attrs {
        if attr.path().is_ident("scaffold_call") {
            if let Some(call_block) = parse_call_block(attr) {
                return quote! { #call_block };
            }
        }
    }

    // Built-in preset detection based on field type
    if let Type::Path(type_path) = &field.ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            match type_name.as_str() {
                "Mutex" | "RwLock" | "MutexGuard" | "RwLockReadGuard" | "RwLockWriteGuard"
                | "Arc" | "Rc" | "RefCell" | "Box" => {
                    return detect_preset(&type_name, field_name);
                }
                _ => {}
            }

            // Check for nested wrappers like Arc<Mutex<T>>
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(Type::Path(inner_path))) = args.args.first() {
                    if let Some(inner_segment) = inner_path.path.segments.last() {
                        let inner_name = inner_segment.ident.to_string();
                        if inner_name == "Mutex" || inner_name == "RwLock" {
                            return detect_preset(&inner_name, field_name);
                        }
                    }
                }
            }
        }
    }

    // Plain field access
    quote! { self.#field_name }
}

fn detect_preset(wrapper: &str, field: &syn::Ident) -> proc_macro2::TokenStream {
    match wrapper {
        "Mutex" => {
            quote! { self.#field.lock().unwrap() }
        }
        "RwLock" => {
            quote! { self.#field.read().unwrap() }
        }
        "RefCell" => {
            quote! { self.#field.borrow() }
        }
        _ => {
            quote! { self.#field }
        }
    }
}

fn parse_call_block(attr: &Attribute) -> Option<proc_macro2::TokenStream> {
    // Parse [field = "...",] call = { ... }
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        // Skip optional `field = "...",` prefix
        if input.peek(syn::Ident) {
            let ident: syn::Ident = input.fork().parse()?;
            if ident == "field" {
                let _: syn::Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                let _: syn::LitStr = input.parse()?;
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                }
            }
        }
        let ident: syn::Ident = input.parse()?;
        if ident != "call" {
            return Err(syn::Error::new(ident.span(), "expected `call`"));
        }
        input.parse::<Token![=]>()?;
        let content;
        syn::braced!(content in input);
        content.parse::<TokenStream2>()
    })
    .ok()
}

fn parse_scaffold_impl_attr(attr: &TokenStream2) -> Option<proc_macro2::TokenStream> {
    // Parse via = "expr"
    if attr.is_empty() {
        return None;
    }

    // Tokenize and look for via = "..."
    let token_vec: Vec<_> = attr.clone().into_iter().collect();
    for (i, tok) in token_vec.iter().enumerate() {
        if tok.to_string() == "via" {
            if let Some(proc_macro2::TokenTree::Punct(p)) = token_vec.get(i + 1) {
                if p.as_char() == '=' {
                    // Get the value — could be string literal or expression
                    if let Some(val) = token_vec.get(i + 2) {
                        if let proc_macro2::TokenTree::Literal(lit) = val {
                            // String literal — parse as expression
                            let s = lit.to_string();
                            let s = s.trim_matches('"');
                            return syn::parse_str::<proc_macro2::TokenStream>(s).ok();
                        }
                        return Some(proc_macro2::TokenStream::from(val.clone()));
                    }
                }
            }
        }
    }
    None
}

fn parse_scaffold_method_attr(attr: &Attribute) -> Option<proc_macro2::TokenStream> {
    // Parse via = "expr"
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        let ident: syn::Ident = input.parse()?;
        if ident != "via" {
            return Err(syn::Error::new(ident.span(), "expected `via`"));
        }
        input.parse::<Token![=]>()?;
        let lit: syn::LitStr = input.parse()?;
        let s = lit.value();
        syn::parse_str::<proc_macro2::TokenStream>(&s)
    })
    .ok()
}

#[allow(dead_code)]
fn parse_scaffold_call_attr_as_via(attr: &Attribute) -> Option<proc_macro2::TokenStream> {
    parse_call_block(attr).map(|block| quote! { #block })
}

fn find_scaffold_call_block(attrs: &[Attribute]) -> Option<proc_macro2::TokenStream> {
    attrs
        .iter()
        .find(|a| a.path().is_ident("scaffold_call"))
        .and_then(parse_call_block)
}

fn is_scaffold_call(block: &syn::Block) -> bool {
    if block.stmts.len() == 1 {
        if let syn::Stmt::Expr(syn::Expr::Macro(mac_expr), None) = &block.stmts[0] {
            if mac_expr.mac.path.is_ident("scaffold") {
                let tokens: String = mac_expr.mac.tokens.to_string();
                if tokens.trim().is_empty() || tokens.trim() == "()" {
                    return true;
                }
            }
        }
    }
    false
}

fn build_delegation(
    sig: &syn::Signature,
    via: &proc_macro2::TokenStream,
) -> syn::Block {
    let ident = &sig.ident;
    let arg_names: Vec<_> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat) => Some(pat.pat.to_token_stream()),
            FnArg::Receiver(_) => None,
        })
        .collect();

    let call = quote! { #via.#ident(#(#arg_names),*) };
    syn::parse_quote!({ #call })
}
