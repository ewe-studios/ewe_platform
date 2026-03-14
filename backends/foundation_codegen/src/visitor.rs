use std::collections::HashMap;
use std::path::PathBuf;

use syn::visit::Visit;

use crate::types::{AttributeValue, FoundItem, ItemKind, Location};

/// WHY: Rust proc macros cannot see each other's invocations — each runs
/// in isolation with no shared state. This visitor provides the "global view"
/// by walking a parsed AST and collecting all items annotated with a target
/// attribute.
///
/// WHAT: AST visitor that finds items with a specific macro attribute and
/// records their name, kind, location, and parsed attribute arguments.
///
/// HOW: Implements `syn::visit::Visit` for struct/enum/trait/fn/type/mod items.
/// Tracks inline module nesting via a stack so that items inside
/// `mod inner { ... }` get their module path recorded.
pub struct MacroFinder {
    target_attr: String,
    current_file: PathBuf,
    module_stack: Vec<String>,
    /// Collected results from the visitor walk.
    pub found: Vec<FoundItem>,
}

impl MacroFinder {
    /// WHY: Each file scan needs a fresh visitor bound to the target attribute
    /// and file path.
    ///
    /// WHAT: Creates a new `MacroFinder` for the given attribute name and file.
    ///
    /// HOW: Initializes empty state; the visitor collects results during `visit_file`.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(target_attr: &str, file_path: &PathBuf) -> Self {
        Self {
            target_attr: target_attr.to_string(),
            current_file: file_path.clone(),
            module_stack: Vec::new(),
            found: Vec::new(),
        }
    }

    fn check_attributes(&mut self, attrs: &[syn::Attribute], ident: &syn::Ident, kind: ItemKind) {
        for attr in attrs {
            if attr.path().is_ident(&self.target_attr) {
                let attributes = parse_attribute_args(attr);
                let span = ident.span();

                self.found.push(FoundItem {
                    item_name: ident.to_string(),
                    item_kind: kind.clone(),
                    attributes,
                    location: Location {
                        file_path: self.current_file.clone(),
                        line: span.start().line,
                        column: span.start().column + 1,
                    },
                    macro_name: self.target_attr.clone(),
                    inline_module_path: self.module_stack.clone(),
                });
            }
        }
    }
}

impl<'ast> Visit<'ast> for MacroFinder {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.check_attributes(&node.attrs, &node.ident, ItemKind::Struct);
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.check_attributes(&node.attrs, &node.ident, ItemKind::Enum);
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.check_attributes(&node.attrs, &node.ident, ItemKind::Trait);
        syn::visit::visit_item_trait(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.check_attributes(&node.attrs, &node.sig.ident, ItemKind::Function);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.check_attributes(&node.attrs, &node.ident, ItemKind::TypeAlias);
        syn::visit::visit_item_type(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        // Only push onto the module stack for inline modules (those with a body { ... })
        if node.content.is_some() {
            self.module_stack.push(node.ident.to_string());
            syn::visit::visit_item_mod(self, node);
            self.module_stack.pop();
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }
}

/// WHY: Downstream code generators need the key-value pairs from macro
/// attributes to configure their output.
///
/// WHAT: Parses the arguments inside `#[attr_name(...)]` into a map.
///
/// HOW: Uses `attr.parse_nested_meta` to walk each argument and classify
/// it as string/bool/int/ident/list/flag based on the token that follows.
fn parse_attribute_args(attr: &syn::Attribute) -> HashMap<String, AttributeValue> {
    let mut map = HashMap::new();

    let _ = attr.parse_nested_meta(|meta| {
        let key = meta
            .path
            .get_ident()
            .map(ToString::to_string)
            .unwrap_or_default();

        if meta.input.peek(syn::Token![=]) {
            // Key = Value form
            let _eq: syn::Token![=] = meta.input.parse()?;
            if meta.input.peek(syn::LitStr) {
                let lit: syn::LitStr = meta.input.parse()?;
                map.insert(key, AttributeValue::String(lit.value()));
            } else if meta.input.peek(syn::LitBool) {
                let lit: syn::LitBool = meta.input.parse()?;
                map.insert(key, AttributeValue::Bool(lit.value()));
            } else if meta.input.peek(syn::LitInt) {
                let lit: syn::LitInt = meta.input.parse()?;
                map.insert(key, AttributeValue::Int(lit.base10_parse::<i64>()?));
            } else if meta.input.peek(syn::Ident) {
                let ident: syn::Ident = meta.input.parse()?;
                map.insert(key, AttributeValue::Ident(ident.to_string()));
            }
        } else if meta.input.peek(syn::token::Paren) {
            // Key(a, b, c) form — nested list
            let mut list = Vec::new();
            meta.parse_nested_meta(|nested| {
                let value = nested
                    .path
                    .get_ident()
                    .map(|i| AttributeValue::Ident(i.to_string()))
                    .unwrap_or(AttributeValue::Ident(String::new()));
                list.push(value);
                Ok(())
            })?;
            map.insert(key, AttributeValue::List(list));
        } else {
            // Bare flag
            map.insert(key, AttributeValue::Flag);
        }

        Ok(())
    });

    map
}
