//! Insert hooks into the original source text. Syntax spans preserve user formatting.
use super::{invalid, Capability};
use std::{collections::BTreeMap, io};
use syn::{Expr, Item, Stmt};

const BEGIN: &str = "// foundation_tasks build hooks begin";
const END: &str = "// foundation_tasks build hooks end";

fn calls(expr: &Expr, function: &syn::Path) -> bool {
    match expr {
        Expr::Call(call) => matches!(&*call.func, Expr::Path(path) if call.args.is_empty()
            && path.qself.is_none() && path.path.segments.iter().map(|s| &s.ident).eq(function.segments.iter().map(|s| &s.ident))),
        Expr::Try(expr) => calls(&expr.expr, function),
        Expr::MethodCall(expr) => calls(&expr.receiver, function),
        Expr::Paren(expr) => calls(&expr.expr, function),
        _ => false,
    }
}

pub(super) fn edit(source: &str, alias: &str, provider: &Capability) -> io::Result<String> {
    let ast = syn::parse_file(source).map_err(invalid)?;
    let mut mains = ast.items.iter().filter_map(|item| match item {
        Item::Fn(f) if f.sig.ident == "main" => Some(f),
        _ => None,
    });
    let main = mains
        .next()
        .ok_or_else(|| invalid("Missing main function"))?;
    if mains.next().is_some()
        || !main.sig.inputs.is_empty()
        || main.sig.asyncness.is_some()
        || main
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
    {
        return Err(invalid("Ambiguous build script entry point"));
    }
    let call = format!("{}::{}", alias.replace('-', "_"), provider.function);
    let path: syn::Path = syn::parse_str(&call).map_err(invalid)?;
    let start = main.block.brace_token.span.open().byte_range().end;
    let mut insert = start;
    // Inner function attributes must stay at the start of the block.
    for attr in &main.attrs {
        if matches!(attr.style, syn::AttrStyle::Inner(_)) {
            insert = attr.bracket_token.span.close().byte_range().end;
        }
    }
    let body = &source[insert..main.block.brace_token.span.close().byte_range().start];
    let nl = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut hooks = BTreeMap::new();
    let mut end = insert;
    if let Some(first) = body.trim_start().strip_prefix(BEGIN) {
        let (managed, tail) = first
            .split_once(END)
            .ok_or_else(|| invalid("Unclosed build hook section"))?;
        for line in managed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let (call, name) = line
                .rsplit_once(" // ")
                .ok_or_else(|| invalid("Changed build hook section"))?;
            if hooks.insert(name, call.to_owned()).is_some() {
                return Err(invalid("Duplicate managed hook"));
            }
        }
        end += body.len() - tail.len();
    } else if body.lines().any(|l| [BEGIN, END].contains(&l.trim())) {
        return Err(invalid("Misplaced build hook section"));
    }
    let returns_result = matches!(&main.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Result")));
    let statement = if provider.fallible && returns_result {
        format!("{call}()?;")
    } else if provider.fallible {
        format!("{call}().expect(\"{} build hook failed\");", provider.name)
    } else {
        format!("{call}();")
    };
    if let Some(existing) = hooks.get(provider.name) {
        return if *existing == statement {
            Ok(source.to_owned())
        } else {
            Err(invalid("Conflicting managed hook"))
        };
    }
    if main
        .block
        .stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Expr(expr, _) if calls(expr, &path)))
    {
        return Ok(source.to_owned());
    }
    hooks.insert(provider.name, statement);
    let mut block = format!("{nl}    {BEGIN}{nl}");
    for (name, call) in hooks {
        block.extend(["    ", &call, " // ", name, nl]);
    }
    block.push_str("    ");
    block.push_str(END);
    if !source[end..].starts_with(['\r', '\n']) {
        block.push_str(nl);
    }
    Ok(format!("{}{block}{}", &source[..insert], &source[end..]))
}
