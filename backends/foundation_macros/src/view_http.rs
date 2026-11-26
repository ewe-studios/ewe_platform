//! Generate a native GET handler for a Foundation view.
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, ItemFn, LitStr, Result};

pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let path: LitStr = syn::parse2(attr)?;
    if !path.value().starts_with('/') {
        return Err(Error::new(path.span(), "expected an absolute route path"));
    }
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = syn::parse2(item)?;
    if sig.inputs.len() > 1 || !sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &sig,
            "expected a view function with zero or one request argument",
        ));
    }
    let crate_ident = match crate_name("foundation_ui_http") {
        Ok(FoundCrate::Name(name)) => Ident::new(&name, Span::call_site()),
        _ => Ident::new("foundation_ui_http", Span::call_site()),
    };
    let api = quote!(::#crate_ident);
    let http = quote!(#api::http);
    let handler = sig.ident.clone();
    sig.ident = Ident::new("handle_view", Span::mixed_site());
    let function = &sig.ident;
    let input = (!sig.inputs.is_empty()).then(|| quote!(request));
    let wait = sig.asyncness.is_some().then(|| quote!(.await));
    let parameter = if input.is_some() {
        quote!(request)
    } else {
        quote!(_request)
    };
    let response = quote!(Result<#api::Response, #api::ViewError>);
    let call = quote!(#api::IntoViewResponse::into_view_response(#function(#input)#wait));
    let (qualifier, result_type, body) = if sig.asyncness.is_some() {
        (quote!(async), response.clone(), call)
    } else {
        (
            quote!(),
            quote!(::core::future::Ready<#response>),
            quote!(::core::future::ready(#call)),
        )
    };
    let gates: Vec<_> = attrs.iter().filter(|a| a.path().is_ident("cfg")).collect();
    Ok(quote! {
        #(#attrs)*
        #[allow(non_camel_case_types)]
        #vis struct #handler;

        #(#gates)*
        impl #handler {
            /// Calls the view function with this request.
            #vis #qualifier fn respond(&self, #parameter: #api::ViewRequest) -> #result_type {
                #[allow(clippy::needless_pass_by_value, reason = "A view handler owns its request and can consume the incoming body.")]
                #sig #block
                #body
            }
        }

        #(#gates)*
        impl #http::Route for #handler {
            const METHOD: #http::SimpleMethod = #http::SimpleMethod::GET;
            const PATH: &'static str = #path;
        }

        #(#gates)*
        impl #http::Serve for #handler {
            fn serve(
                &self,
                ctx: #http::Ctx,
                req: #http::SimpleIncomingRequest,
                conn: #http::SharedByteBufferStream<#http::RawStream>,
            ) -> #http::ConnectionResult {
                #api::serve_view(ctx, req, conn, |request| async move { #handler.respond(request).await })
            }
        }
    })
}
