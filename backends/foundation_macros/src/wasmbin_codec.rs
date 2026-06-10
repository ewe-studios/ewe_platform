//! WHY: `foundation_codegen::wasm` describes the WebAssembly binary format 1:1 as
//! Rust types; hand-writing Encode/Decode/Visit for every section, instruction, and
//! index would be unmaintainable. These derives autogenerate the codec and visitor
//! from the type shape, so adding a wasm proposal = adding Rust types (feature 15,
//! decision 031).
//!
//! WHAT: The synstructure bodies behind `#[derive(Wasmbin)]` (Encode + Decode +
//! DecodeWithDiscriminant), `#[derive(WasmbinCountable)]`, and `#[derive(Visit)]`.
//! Entry points live in `lib.rs` (proc-macro fns must sit at the crate root).
//!
//! HOW: Ported & adapted from `wasmbin-derive` v0.2.4
//! (https://github.com/RReverser/wasmbin), © Google Inc. / Ingvar Stepanyan,
//! licensed Apache-2.0. Modifications for ewe_platform: trait paths resolve through
//! `proc-macro-crate` to `foundation_codegen::wasm::{io,builtins,visit}` (upstream
//! hardcoded `crate::…` since the derives only ran inside wasmbin itself), and the
//! `decl_derive!` wrappers are replaced by plain `#[proc_macro_derive]` entry points
//! to match this crate's conventions.
//
// Copyright 2020 Google Inc. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use quote::{quote, ToTokens};
use std::borrow::Cow;
use synstructure::{Structure, VariantInfo};

use crate::crate_paths::foundation_codegen_path;

macro_rules! syn_throw {
    ($err:expr) => {
        return syn::Error::to_compile_error(&$err)
    };
}

macro_rules! syn_try {
    ($expr:expr) => {
        match $expr {
            Ok(expr) => expr,
            Err(err) => syn_throw!(err),
        }
    };
}

/// Extract `#[wasmbin(discriminant = EXPR)]` from a struct (used by single-struct
/// types that still carry a wire discriminant, e.g. blob-typed sections).
fn struct_discriminant<'v>(v: &VariantInfo<'v>) -> syn::Result<Option<Cow<'v, syn::Expr>>> {
    v.ast()
        .attrs
        .iter()
        .filter_map(|attr| match attr {
            syn::Attribute {
                style: syn::AttrStyle::Outer,
                meta,
                ..
            } if meta.path().is_ident("wasmbin") => {
                syn::custom_keyword!(discriminant);

                Some(
                    attr.parse_args_with(|parser: syn::parse::ParseStream| {
                        parser.parse::<discriminant>()?;
                        parser.parse::<syn::Token![=]>()?;
                        parser.parse()
                    })
                    .map(Cow::Owned),
                )
            }
            _ => None,
        })
        .try_fold(None, |prev, discriminant| {
            let discriminant = discriminant?;
            if let Some(prev) = prev {
                let mut err = syn::Error::new_spanned(
                    discriminant,
                    "#[derive(Wasmbin)]: duplicate discriminant",
                );
                err.combine(syn::Error::new_spanned(
                    prev,
                    "#[derive(Wasmbin)]: previous discriminant here",
                ));
                return Err(err);
            }
            Ok(Some(discriminant))
        })
}

fn gen_encode_discriminant(repr: &syn::Type, discriminant: &syn::Expr) -> proc_macro2::TokenStream {
    quote!(<#repr as Encode>::encode(&#discriminant, w)?)
}

fn is_newtype_like(v: &VariantInfo) -> bool {
    matches!(v.ast().fields, fields @ syn::Fields::Unnamed(_) if fields.len() == 1)
}

/// Wrap a field's codec/visit expression so errors report the field's path
/// (skipped for newtype-like variants where the path adds no information).
fn track_err_in_field(
    mut res: proc_macro2::TokenStream,
    v: &VariantInfo,
    field: &syn::Field,
    index: usize,
) -> proc_macro2::TokenStream {
    if !is_newtype_like(v) {
        let field_name = match &field.ident {
            Some(ident) => ident.to_string(),
            None => index.to_string(),
        };
        res = quote!(#res.map_err(|err| err.in_path(PathItem::Name(#field_name))));
    }
    res
}

fn track_err_in_variant(
    res: proc_macro2::TokenStream,
    v: &VariantInfo,
) -> proc_macro2::TokenStream {
    use std::fmt::Write;

    let mut variant_name = String::new();
    if let Some(prefix) = v.prefix {
        write!(variant_name, "{prefix}::").unwrap();
    }
    write!(variant_name, "{}", v.ast().ident).unwrap();

    quote!(#res.map_err(|err| err.in_path(PathItem::Variant(#variant_name))))
}

fn catch_expr(
    res: proc_macro2::TokenStream,
    err: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote!(
        (move || -> Result<_, #err> {
            Ok({ #res })
        })()
    )
}

fn gen_decode(v: &VariantInfo) -> proc_macro2::TokenStream {
    let mut res = v.construct(|field, index| {
        let res = track_err_in_field(quote!(Decode::decode(r)), v, field, index);
        quote!(#res?)
    });
    res = catch_expr(res, quote!(DecodeError));
    res = track_err_in_variant(res, v);
    res
}

fn parse_repr(s: &Structure) -> syn::Result<syn::Type> {
    s.ast()
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("repr"))
        .ok_or_else(|| {
            syn::Error::new_spanned(
                &s.ast().ident,
                "Wasmbin enums must have a #[repr(type)] attribute",
            )
        })?
        .parse_args()
}

/// `#[derive(Wasmbin)]` body: Encode for everything; enums with `#[repr(N)]` +
/// per-variant discriminants get DecodeWithDiscriminant (catch-all variants without
/// a discriminant delegate to their single field's own discriminant decoding).
pub(crate) fn wasmbin_derive(s: Structure) -> proc_macro2::TokenStream {
    let root = foundation_codegen_path();
    let (encode_discriminant, decode) = match s.ast().data {
        syn::Data::Enum(_) => {
            let repr = syn_try!(parse_repr(&s));

            let mut encode_discriminant = quote!();

            let mut decoders = quote!();
            let mut decode_other = quote!({ return Ok(None) });

            for v in s.variants() {
                match v.ast().discriminant {
                    Some((_, discriminant)) => {
                        let pat = v.pat();

                        let encode = gen_encode_discriminant(&repr, discriminant);
                        (quote!(#pat => #encode,)).to_tokens(&mut encode_discriminant);

                        let decode = gen_decode(v);
                        (quote!(
                            #discriminant => #decode?,
                        ))
                        .to_tokens(&mut decoders);
                    }
                    None => {
                        let fields = v.ast().fields;
                        if fields.len() != 1 {
                            syn_throw!(syn::Error::new_spanned(
                                fields,
                                "Catch-all variants without discriminant must have a single field."
                            ));
                        }
                        let field = fields.iter().next().unwrap();
                        let construct = match &field.ident {
                            Some(ident) => quote!({ #ident: res }),
                            None => quote!((res)),
                        };
                        let variant_name = v.ast().ident;
                        decode_other = quote! {
                            if let Some(res) = DecodeWithDiscriminant::maybe_decode_with_discriminant(discriminant, r)? {
                                Self::#variant_name #construct
                            } else #decode_other
                        };
                    }
                }
            }

            (
                quote! {
                    match *self {
                        #encode_discriminant
                        _ => {}
                    }
                },
                quote! {
                    gen impl DecodeWithDiscriminant for @Self {
                        type Discriminant = #repr;

                        fn maybe_decode_with_discriminant(discriminant: #repr, r: &mut impl std::io::Read) -> Result<Option<Self>, DecodeError> {
                            Ok(Some(match discriminant {
                                #decoders
                                _ => #decode_other
                            }))
                        }
                    }

                    gen impl Decode for @Self {
                        fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
                            DecodeWithDiscriminant::decode_without_discriminant(r)
                        }
                    }
                },
            )
        }
        _ => {
            let variants = s.variants();
            assert_eq!(variants.len(), 1);
            let v = &variants[0];
            let decode = gen_decode(v);
            match syn_try!(struct_discriminant(v)) {
                Some(discriminant) => (
                    gen_encode_discriminant(&syn::parse_quote!(u8), &discriminant),
                    quote! {
                        gen impl DecodeWithDiscriminant for @Self {
                            type Discriminant = u8;

                            fn maybe_decode_with_discriminant(discriminant: u8, r: &mut impl std::io::Read) -> Result<Option<Self>, DecodeError> {
                                match discriminant {
                                    #discriminant => #decode.map(Some),
                                    _ => Ok(None),
                                }
                            }
                        }

                        gen impl Decode for @Self {
                            fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
                                DecodeWithDiscriminant::decode_without_discriminant(r)
                            }
                        }
                    },
                ),
                None => (
                    quote! {},
                    quote! {
                        gen impl Decode for @Self {
                            fn decode(r: &mut impl std::io::Read) -> Result<Self, DecodeError> {
                                #decode
                            }
                        }
                    },
                ),
            }
        }
    };

    let encode_body = s.each(|bi| {
        quote! {
            Encode::encode(#bi, w)?
        }
    });

    s.gen_impl(quote! {
        use #root::wasm::io::{Encode, Decode, DecodeWithDiscriminant, DecodeError, PathItem};

        gen impl Encode for @Self {
            fn encode(&self, w: &mut impl std::io::Write) -> std::io::Result<()> {
                #encode_discriminant;
                match *self { #encode_body }
                Ok(())
            }
        }

        #decode
    })
}

/// `#[derive(WasmbinCountable)]` body: marker impl — the type serializes inside
/// length-prefixed collections.
pub(crate) fn wasmbin_countable_derive(s: Structure) -> proc_macro2::TokenStream {
    let root = foundation_codegen_path();
    s.gen_impl(quote! {
        gen impl #root::wasm::builtins::WasmbinCountable for @Self {}
    })
}

/// `#[derive(Visit)]` body: typed deep-traversal over every field (shared and
/// mutable variants), with the same error-path tracking as the codec.
pub(crate) fn wasmbin_visit_derive(mut s: Structure) -> proc_macro2::TokenStream {
    let root = foundation_codegen_path();
    s.bind_with(|_| synstructure::BindStyle::Move);

    fn generate_visit_body(
        s: &Structure,
        method: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let body = s.each_variant(|v| {
            let res = v.bindings().iter().enumerate().map(|(i, bi)| {
                let res = quote!(Visit::#method(#bi, f));
                track_err_in_field(res, v, bi.ast(), i)
            });
            let mut res = quote!(#(#res?;)*);
            res = catch_expr(res, quote!(VisitError<VisitE>));
            res = track_err_in_variant(res, v);
            quote!(#res?)
        });
        quote!(
            match self { #body }
            Ok(())
        )
    }

    let visit_children_body = generate_visit_body(&s, quote!(visit_child));

    let visit_children_mut_body = generate_visit_body(&s, quote!(visit_child_mut));

    s.gen_impl(quote! {
        use #root::wasm::visit::{Visit, VisitError};
        use #root::wasm::io::PathItem;

        gen impl Visit for @Self where Self: 'static {
            fn visit_children<'a, VisitT: 'static, VisitE, VisitF: FnMut(&'a VisitT) -> Result<(), VisitE>>(&'a self, f: &mut VisitF) -> Result<(), VisitError<VisitE>> {
                #visit_children_body
            }

            fn visit_children_mut<VisitT: 'static, VisitE, VisitF: FnMut(&mut VisitT) -> Result<(), VisitE>>(&mut self, f: &mut VisitF) -> Result<(), VisitError<VisitE>> {
                #visit_children_mut_body
            }
        }
    })
}
