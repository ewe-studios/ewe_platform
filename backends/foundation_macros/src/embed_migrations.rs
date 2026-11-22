//! `embed_migrations!` and `embed_migration_layout!`.
//!
//! Both macros read a directory at build time, check the migration rules and
//! emit static values. The checks are the same as the run-time checks of
//! `foundation_db::MigrationRunner::plan`, so a build fails on a gap, a repeat
//! or a bad name.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::path::{Path, PathBuf};

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use sha2::{Digest, Sha256};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{bracketed, LitStr, Token};

/// Fewest digits in an embedded file number.
const MIN_NUMBER_DIGITS: usize = 4;
/// Default template group of a layout.
const DEFAULT_TEMPLATE: &str = "shared";

struct EmbedArgs {
    path: LitStr,
    group: Option<LitStr>,
}

impl Parse for EmbedArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse::<LitStr>()?;
        let mut group = None;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let key = input.parse::<syn::Ident>()?;
            if key != "group" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `group = \"name\"` after the path",
                ));
            }
            input.parse::<Token![=]>()?;
            group = Some(input.parse::<LitStr>()?);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { path, group })
    }
}

struct LayoutArgs {
    path: LitStr,
    templates: Vec<LitStr>,
}

impl Parse for LayoutArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse::<LitStr>()?;
        let mut templates = Vec::new();
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let key = input.parse::<syn::Ident>()?;
            if key != "templates" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `templates = [\"name\", ...]` after the path",
                ));
            }
            input.parse::<Token![=]>()?;
            let content;
            bracketed!(content in input);
            let list = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
            templates.extend(list);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        if templates.is_empty() {
            templates.push(LitStr::new(DEFAULT_TEMPLATE, path.span()));
        }
        Ok(Self { path, templates })
    }
}

struct FileEntry {
    number: u64,
    name: String,
    sha256: String,
    relative: String,
}

fn db_path() -> TokenStream2 {
    match crate_name("foundation_db") {
        // `Itself` is also what an integration test target of the same package
        // reports. In that target `crate` is the test crate, not the library.
        Ok(FoundCrate::Itself) => {
            let target = std::env::var("CARGO_CRATE_NAME").unwrap_or_default();
            if target == "foundation_db" {
                quote!(crate)
            } else {
                quote!(::foundation_db)
            }
        }
        Ok(FoundCrate::Name(name)) => {
            let ident = proc_macro2::Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::foundation_db),
    }
}

/// Resolve a path relative to the manifest directory of the compiling crate.
///
/// A compile-fail test runs in a scratch crate below the workspace `target`
/// directory, so the path is not under that crate. In that case the search
/// walks up to the workspace root.
fn resolve_dir(relative: &str) -> PathBuf {
    let path = Path::new(relative);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let first = base.join(relative);
    if first.exists() {
        return first;
    }
    let mut current = base.as_path();
    while let Some(parent) = current.parent() {
        let candidate = parent.join(relative);
        if candidate.exists() {
            return candidate;
        }
        current = parent;
    }
    first
}

fn valid_group_name(group: &str) -> bool {
    !group.is_empty()
        && group.len() <= 64
        && group
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn parse_name(file_name: &str, origin: &str) -> Result<(u64, String), String> {
    let Some(stem) = file_name.strip_suffix(".sql") else {
        return Err(format!(
            "{origin}: a migration file name must be <digits>_<name>.sql"
        ));
    };
    let Some((digits, name)) = stem.split_once('_') else {
        return Err(format!(
            "{origin}: a migration file name must be <digits>_<name>.sql"
        ));
    };
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(format!(
            "{origin}: a migration file name must be <digits>_<name>.sql, with a lowercase name"
        ));
    }
    if digits.len() < MIN_NUMBER_DIGITS {
        return Err(format!(
            "{origin}: the number has {} digits, but at least {MIN_NUMBER_DIGITS} are required",
            digits.len()
        ));
    }
    let number = digits
        .parse::<u64>()
        .map_err(|_| format!("{origin}: the number is too large"))?;
    if number == 0 {
        return Err(format!("{origin}: the number must be at least 1"));
    }
    Ok((number, name.to_string()))
}

/// Read one group folder and check its files.
fn read_group(dir: &Path, relative_root: &str, problems: &mut Vec<String>) -> Vec<FileEntry> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            problems.push(format!("{relative_root}: {error}"));
            return Vec::new();
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                problems.push(format!("{relative_root}: {error}"));
                continue;
            }
        };
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let origin = format!("{relative_root}/{file_name}");
        if path.is_dir() {
            problems.push(format!(
                "{origin}: a group folder must not contain a folder"
            ));
            continue;
        }
        let (number, name) = match parse_name(&file_name, &origin) {
            Ok(parsed) => parsed,
            Err(problem) => {
                problems.push(problem);
                continue;
            }
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(format!("{origin}: {error}"));
                continue;
            }
        };
        let Ok(sql) = String::from_utf8(bytes) else {
            problems.push(format!("{origin}: the file is not UTF-8 text"));
            continue;
        };
        files.push(FileEntry {
            number,
            name,
            sha256: sha256_hex(sql.as_bytes()),
            relative: format!("{relative_root}/{file_name}"),
        });
    }
    files.sort_by_key(|file| file.number);

    let mut expected = 1u64;
    for file in &files {
        if file.number < expected {
            problems.push(format!(
                "{relative_root}: group uses number {} more than once",
                file.number
            ));
        }
        while expected < file.number {
            problems.push(format!(
                "{relative_root}: group is missing migration number {expected}"
            ));
            expected += 1;
        }
        expected = file.number + 1;
    }
    files
}

fn emit_migration_set(group: &str, files: &[FileEntry], span: Span) -> TokenStream2 {
    let db = db_path();
    let count = files.len();
    let group_lit = LitStr::new(group, span);
    let entries = files.iter().map(|file| {
        let number = file.number;
        let name = LitStr::new(&file.name, span);
        let sha = LitStr::new(&file.sha256, span);
        let relative = LitStr::new(&file.relative, span);
        let sql_path = if Path::new(&file.relative).is_absolute() {
            quote!(#relative)
        } else {
            quote!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #relative))
        };
        quote! {
            #db::EmbeddedMigration {
                number: #number,
                name: #name,
                sql: include_str!(#sql_path),
                sha256: #sha,
            }
        }
    });
    quote! {
        {
            static __FOUNDATION_EMBEDDED: [#db::EmbeddedMigration; #count] = [#(#entries),*];
            #db::MigrationSet {
                group: ::std::borrow::Cow::Borrowed(#group_lit),
                source: #db::MigrationSource::Embedded(&__FOUNDATION_EMBEDDED),
            }
        }
    }
}

fn default_group(path: &LitStr) -> String {
    PathBuf::from(path.value())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Embed a directory as one `MigrationSet` at build time.
pub(crate) fn embed_migrations_impl(input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(input as EmbedArgs);
    let group = if let Some(group) = &args.group {
        group.value()
    } else {
        default_group(&args.path)
    };
    let mut problems = Vec::new();
    if !valid_group_name(&group) {
        problems.push(format!(
            "group name {group:?} must use lowercase letters, digits and _ (1 to 64 characters)"
        ));
    }
    let relative_root = args.path.value().trim_end_matches('/').to_string();
    let dir = resolve_dir(&relative_root);
    let files = read_group(&dir, &relative_root, &mut problems);
    if !problems.is_empty() {
        return syn::Error::new(args.path.span(), problems.join("\n"))
            .to_compile_error()
            .into();
    }
    emit_migration_set(&group, &files, args.path.span()).into()
}

fn check_template_names(template_names: &[String], problems: &mut Vec<String>) {
    for name in template_names {
        if !valid_group_name(name) {
            problems.push(format!(
                "template group name {name:?} must use lowercase letters, digits and _ (1 to 64 characters)"
            ));
        }
    }
}

fn read_layout_groups(
    dir: &Path,
    relative_root: &str,
    problems: &mut Vec<String>,
) -> Result<Vec<(String, Vec<FileEntry>)>, String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut groups = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                problems.push(format!("{}: {error}", dir.display()));
                continue;
            }
        };
        if !entry.path().is_dir() {
            continue;
        }
        let group = entry.file_name().to_string_lossy().into_owned();
        if !valid_group_name(&group) {
            problems.push(format!(
                "group name {group:?} must use lowercase letters, digits and _ (1 to 64 characters)"
            ));
            continue;
        }
        let relative = format!("{relative_root}/{group}");
        let files = read_group(&entry.path(), &relative, problems);
        groups.push((group, files));
    }
    Ok(groups)
}

fn check_template_folders(
    dir: &Path,
    template_names: &[String],
    groups: &[(String, Vec<FileEntry>)],
    problems: &mut Vec<String>,
) {
    for name in template_names {
        if !groups.iter().any(|(group, _)| group == name) {
            problems.push(format!(
                "{}: template group folder not found",
                dir.join(name).display()
            ));
        }
    }
}

fn emit_layout(
    groups: &[(String, Vec<FileEntry>)],
    template_names: &[String],
    span: Span,
) -> TokenStream2 {
    let db = db_path();
    let mut static_names = Vec::new();
    let mut set_items = Vec::new();
    for (index, (group, files)) in groups.iter().enumerate() {
        let ident =
            proc_macro2::Ident::new(&format!("__FOUNDATION_GROUP_{index}"), Span::call_site());
        let value = emit_migration_set(group, files, span);
        set_items.push(quote! {
            static #ident: #db::MigrationSet = #value;
        });
        static_names.push((group.clone(), ident));
    }

    let template_idents: Vec<_> = template_names
        .iter()
        .filter_map(|name| {
            static_names
                .iter()
                .find(|(group, _)| group == name)
                .map(|(_, ident)| ident)
        })
        .collect();
    let specific_idents: Vec<_> = static_names
        .iter()
        .filter(|(group, _)| !template_names.iter().any(|name| name == group))
        .map(|(_, ident)| ident)
        .collect();
    let template_lits: Vec<_> = template_names
        .iter()
        .map(|name| LitStr::new(name, span))
        .collect();

    quote! {
        {
            #(#set_items)*
            static __FOUNDATION_TEMPLATES: &[&#db::MigrationSet] = &[#(&#template_idents),*];
            static __FOUNDATION_SPECIFIC: &[&#db::MigrationSet] = &[#(&#specific_idents),*];
            static __FOUNDATION_TEMPLATE_NAMES: &[&str] = &[#(#template_lits),*];
            #db::EmbeddedLayout {
                templates: __FOUNDATION_TEMPLATES,
                specific: __FOUNDATION_SPECIFIC,
                template_groups: __FOUNDATION_TEMPLATE_NAMES,
            }
        }
    }
}

/// Embed a directory layout at build time.
pub(crate) fn embed_migration_layout_impl(input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(input as LayoutArgs);
    let relative_root = args.path.value().trim_end_matches('/').to_string();
    let dir = resolve_dir(&relative_root);
    let template_names: Vec<String> = args.templates.iter().map(LitStr::value).collect();
    let mut problems = Vec::new();
    check_template_names(&template_names, &mut problems);

    let groups = match read_layout_groups(&dir, &relative_root, &mut problems) {
        Ok(groups) => groups,
        Err(problem) => {
            return syn::Error::new(args.path.span(), problem)
                .to_compile_error()
                .into();
        }
    };
    check_template_folders(&dir, &template_names, &groups, &mut problems);
    if !problems.is_empty() {
        return syn::Error::new(args.path.span(), problems.join("\n"))
            .to_compile_error()
            .into();
    }
    emit_layout(&groups, &template_names, args.path.span()).into()
}
