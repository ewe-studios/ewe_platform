use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Ident, ItemFn, LitStr};

pub fn expand(method: &str, attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let path: LitStr = syn::parse2(attr)?;
    if !path.value().starts_with('/') {
        return Err(syn::Error::new(
            path.span(),
            "expected an absolute route path",
        ));
    }
    let function: ItemFn = syn::parse2(item)?;
    let sig = &function.sig;
    if sig.inputs.len() != 3 || !sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            sig,
            "expected a function with context, request, and connection arguments",
        ));
    }
    let name = &sig.ident;
    let vis = &function.vis;
    let attrs = &function.attrs;
    let gates: Vec<_> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect();
    let method = Ident::new(method, Span::call_site());
    let package = match crate_name("foundation_http") {
        Ok(FoundCrate::Name(alias)) => alias,
        _ => "foundation_http".to_owned(),
    };
    let package = Ident::new(&package, Span::call_site());
    let mut inner = sig.clone();
    inner.ident = Ident::new("__http_route_body", sig.ident.span());
    let body = &function.block;
    let call = quote!(__http_route_body(ctx, request, connection));
    let result = if sig.asyncness.is_some() {
        quote!(::#package::ConnectionResult::Async(::std::boxed::Box::pin(#call)))
    } else {
        call
    };
    Ok(quote! {
        #(#attrs)*
        #[allow(non_camel_case_types)]
        #vis struct #name;

        #(#gates)*
        impl ::#package::Route for #name {
            const METHOD: ::#package::SimpleMethod = ::#package::SimpleMethod::#method;
            const PATH: &'static str = #path;
        }

        #(#gates)*
        impl ::#package::Serve for #name {
            fn serve(
                &self,
                ctx: ::#package::Ctx,
                request: ::#package::SimpleIncomingRequest,
                connection: ::#package::SharedByteBufferStream<::#package::RawStream>,
            ) -> ::#package::ConnectionResult {
                #inner #body
                #result
            }
        }
    })
}
