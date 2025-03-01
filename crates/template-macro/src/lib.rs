#[macro_use]
extern crate syn;

use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream, Peek, Result},
    parse_macro_input, token,
};

fn parse_template_stream(input: ParseStream) -> Result<(syn::Ident, syn::LitStr)> {
    assert!(!input.is_empty(), "Expected to see [..] content");

    assert!(input.peek(token::Bracket), "Expected macro to start with {{ .. }}");

    let tml_content;
    _ = bracketed!(tml_content in input);

    let name: syn::Ident = syn::parse2(parse_until(&tml_content, token::Comma)?)?;

    // skip comma
    assert!(skip_if(&tml_content, is_comma)?.is_some(), "Expected a comma after the language definition");

    let content: syn::LitStr = syn::parse2(parse_until(&tml_content, token::Bracket)?)?;

    Ok((name, content))
}

#[derive(Clone)]
struct TinyTemplateItem {
    name: syn::Ident,
    content: syn::LitStr,
}

impl Parse for TinyTemplateItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (name, content) = parse_template_stream(input)?;
        Ok(TinyTemplateItem { name, content })
    }
}

impl ToTokens for TinyTemplateItem {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.name.to_string();
        let content = &self.content;

        tokens.extend(quote! {
            core_template.add_template(#name, #content).expect("should register template");
        });
    }
}

#[derive(Clone)]
struct JinjaTemplateItem {
    name: syn::Ident,
    content: syn::LitStr,
}

impl Parse for JinjaTemplateItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (name, content) = parse_template_stream(input)?;
        Ok(JinjaTemplateItem { name, content })
    }
}

impl ToTokens for JinjaTemplateItem {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.name.to_string();
        let content = self.content.value();

        tokens.extend(quote! {
            core_template.add_template(#name, #content).expect("should compile and add template");
        });
    }
}

#[derive(Clone)]
struct TemplateTag {
    lang: String,
    tiny_templates: Option<Vec<TinyTemplateItem>>,
    jinja_templates: Option<Vec<JinjaTemplateItem>>,
}

impl TemplateTag {
    fn encode_tiny_template(&mut self, tokens: &mut proc_macro2::TokenStream) {
        let templates: TokenStream = self
            .tiny_templates
            .take()
            .unwrap()
            .iter()
            .map(|template| {
                quote! {
                    {
                        #template
                    }
                }
            })
            .collect();

        tokens.extend(quote! {
            {
                use ewe_templates::tinytemplate;

                let mut core_template = tinytemplate::TinyTemplate::new();

                #templates

                core_template
            }
        });
    }

    fn encode_minijinja_template(&mut self, tokens: &mut proc_macro2::TokenStream) {
        let templates: TokenStream = self
            .jinja_templates
            .take()
            .unwrap()
            .iter()
            .map(|template| {
                quote! {
                    #template
                }
            })
            .collect();

        tokens.extend(quote! {
            {
                use ewe_templates::minijinja;
                let mut core_template = minijinja::Environment::new();

                #templates

                core_template
            }
        });
    }
}

impl ToTokens for TemplateTag {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut core = self.clone();
        match self.lang.as_str() {
            "jinja" => core.encode_minijinja_template(tokens),
            "tiny" => core.encode_tiny_template(tokens),
            _ => panic!("{} language is not supported", self.lang),
        }
    }
}

impl Parse for TemplateTag {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        assert!(!input.is_empty(), "Do not expect the ending of a the macro here");

        let lang_ident: syn::Ident = input.parse()?;
        let lang = lang_ident.to_string();

        if let Err(err) = match lang.as_str() {
            "jinja" => Ok(()),
            "tiny" => Ok(()),
            _ => Err(input.error(format!(
                "'{lang}' is not a supported (jinja, tiny) template language"
            ))),
        } {
            panic!("Bad template language: {err}");
        }

        // skip comma
        assert!(skip_if(input, is_comma)?.is_some(), "Expected a comma after the language definition");

        assert!(input.peek(token::Brace), "Expected macro to have template content in {{ .. }} after template type");

        let content;
        _ = braced!(content in input);

        if lang.as_str() == "jinja" {
            let templates = parse_until_empty(&content, JinjaTemplateItem::parse, comma_parser)?;
            return Ok(TemplateTag {
                lang,
                tiny_templates: None,
                jinja_templates: Some(templates),
            });
        }

        let templates = parse_until_empty(&content, TinyTemplateItem::parse, comma_parser)?;

        Ok(TemplateTag {
            lang,
            tiny_templates: Some(templates),
            jinja_templates: None,
        })
    }
}

fn is_comma(tk: &TokenTree) -> bool {
    match tk {
        TokenTree::Punct(pt) => pt.as_char() == ',',
        _ => false,
    }
}

/// `skip_iff` will skip the next token if
/// it matches the underlying provided
/// function returning the next token else
/// returns that same token and the cursor unchanged.
fn skip_if(
    input: ParseStream,
    punct_fn: impl Fn(&proc_macro2::TokenTree) -> bool,
) -> Result<Option<proc_macro2::TokenTree>> {
    input.step(|cursor| {
        let rest = *cursor;
        match rest.token_tree() {
            Some((tt, next)) => {
                if !punct_fn(&tt) {
                    return Ok((None, rest));
                }
                Ok((Some(tt), next))
            }
            None => Err(cursor.error("failed skipping")),
        }
    })
}

fn comma_parser(input: ParseStream) -> Result<()> {
    _ = input.parse::<syn::Token![,]>()?;
    Ok(())
}

fn parse_until_empty<T, ParseFunc, SeparatorParser>(
    input: syn::parse::ParseStream,
    parser: ParseFunc,
    separator: SeparatorParser,
) -> Result<Vec<T>>
where
    ParseFunc: Fn(ParseStream) -> Result<T>,
    SeparatorParser: Fn(ParseStream) -> Result<()>,
{
    let mut parsed = Vec::new();

    while !input.is_empty() {
        let content = parser(input)?;
        parsed.push(content);

        if input.is_empty() {
            break;
        }

        separator(input)?;
    }

    Ok(parsed)
}

#[allow(dead_code)]
fn skip_next(input: ParseStream) -> Result<()> {
    input.step(|cursor| {
        let rest = *cursor;
        match rest.token_tree() {
            Some((_, next)) => Ok(((), next)),
            None => Err(cursor.error("failed skipping")),
        }
    })
}

fn parse_until<E: Peek>(input: ParseStream, end: E) -> Result<TokenStream> {
    let mut tokens = TokenStream::new();
    while !input.is_empty() && !input.peek(end) {
        let next: TokenTree = input.parse()?;
        tokens.extend(Some(next));
    }
    Ok(tokens)
}

#[proc_macro]
pub fn template(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // parse
    let tml: TemplateTag = parse_macro_input!(tokens);

    // generate
    quote! {
        #tml
    }
    .into()
}

#[cfg(test)]
#[test]
fn trybuild() {
    let tc = trybuild::TestCases::new();
    tc.pass("test/tiny/main.rs");
    tc.compile_fail("test/tiny_fail/main.rs");

    tc.pass("test/jinja/main.rs");
    tc.compile_fail("test/jinja_fail/main.rs");
}
