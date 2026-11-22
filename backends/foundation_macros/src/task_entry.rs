//! Entry attributes for repository task executables.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Parse, parse2, Error, FnArg, Ident, ItemFn, LitStr, ReturnType, Token, Type};

struct Discovery {
    directory: LitStr,
    namespace: Option<LitStr>,
}

impl Parse for Discovery {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let directory = input.parse()?;
        let namespace = if input.is_empty() {
            None
        } else {
            input.parse::<Token![,]>()?;
            let key: Ident = input.parse()?;
            if key != "namespace" {
                return Err(Error::new_spanned(key, "Expected namespace"));
            }
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        };
        Ok(Self {
            directory,
            namespace,
        })
    }
}

pub fn expand(args: TokenStream, item: TokenStream, discovery: bool) -> TokenStream {
    expand_result(args, item, discovery).unwrap_or_else(|e| e.into_compile_error())
}

fn path_named(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|part| part.ident == name))
}

fn expand_result(
    args: TokenStream,
    item: TokenStream,
    discovery: bool,
) -> syn::Result<TokenStream> {
    let mut body: ItemFn = parse2(item)?;
    let sig = &body.sig;
    if sig.ident != "main"
        || sig.asyncness.is_none()
        || sig.constness.is_some()
        || sig.unsafety.is_some()
        || sig.abi.is_some()
        || !sig.generics.params.is_empty()
        || sig.inputs.len() != 1
        || !matches!(sig.inputs.first(), Some(FnArg::Typed(arg)) if path_named(&arg.ty, "TaskRunner"))
        || !matches!(&sig.output, ReturnType::Type(_, ty) if path_named(ty, "TaskResult"))
    {
        return Err(Error::new_spanned(
            sig,
            "Expected async fn main(runner: TaskRunner) -> TaskResult",
        ));
    }
    let crate_name = match proc_macro_crate::crate_name("foundation_tasks") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let name = format_ident!("{name}");
            quote!(::#name)
        }
        _ => quote!(::foundation_tasks),
    };
    let discovered = if discovery {
        let Discovery {
            directory,
            namespace,
        } = parse2(args)?;
        let binary_name = std::env::var("CARGO_BIN_NAME").map_err(|_| {
            Error::new_spanned(
                &directory,
                "Directory discovery requires a Cargo binary target",
            )
        })?;
        let digits = b"0123456789abcdef";
        let suffix: String = binary_name
            .bytes()
            .flat_map(|byte| {
                [
                    char::from(digits[usize::from(byte >> 4)]),
                    char::from(digits[usize::from(byte & 15)]),
                ]
            })
            .collect();
        let key = format!("FOUNDATION_TASKS_INDEX_{suffix}");
        let index = std::env::var(&key).map_err(|_| {
            Error::new_spanned(
                &directory,
                "Add foundation_tasks::build::task_dir to the Cargo build script",
            )
        })?;
        let expected = std::env::var(format!("{key}_DIR")).map_err(|_| {
            Error::new_spanned(
                &directory,
                "The Cargo build script must declare the task directory",
            )
        })?;
        if directory.value() != expected {
            return Err(Error::new_spanned(
                directory,
                "The task directory differs from the Cargo build-script declaration",
            ));
        }
        let namespace = namespace.map(|namespace| quote!(.with_namespace(#namespace)));
        quote! {
            mod __foundation_discovered_tasks {
                use #crate_name::{TaskResult, TaskSet};
                include!(#index);
            }
            fn __foundation_configure(mut runner: #crate_name::TaskRunner)
                -> impl std::future::Future<Output = #crate_name::TaskResult> + Send
            {
                async move {
                    let mut set = #crate_name::TaskSet::new(env!("CARGO_PKG_NAME")) #namespace;
                    __foundation_discovered_tasks::register(&mut set)?;
                    runner.register(set)?;
                    __foundation_task_body(runner).await
                }
            }
        }
    } else {
        if !args.is_empty() {
            return Err(Error::new_spanned(
                args,
                "The task entry attribute takes no arguments",
            ));
        }
        quote! {
            async fn __foundation_configure(runner: #crate_name::TaskRunner) -> #crate_name::TaskResult {
                __foundation_task_body(runner).await
            }
        }
    };
    let attrs = std::mem::take(&mut body.attrs);
    body.sig.ident = format_ident!("__foundation_task_body");
    body.vis = syn::Visibility::Inherited;
    Ok(quote! {
        #discovered
        #body
        #(#attrs)*
        fn main() -> std::process::ExitCode {
            #crate_name::run_entry(env!("CARGO_BIN_NAME"), |runner| Box::pin(__foundation_configure(runner)))
        }
    })
}
