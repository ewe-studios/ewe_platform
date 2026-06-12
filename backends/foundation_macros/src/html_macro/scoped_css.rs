//! WHY: Scoped `<style primal:style>` blocks must be transformed at COMPILE
//! time (decision 019 — zero runtime CSS processing in WASM): `:parent`
//! resolves to the owning element's identity and bare selectors get prefixed
//! so styles can't leak out of their component.
//!
//! WHAT: [`transform_scoped_css`] implementing the feature-09 §3 five-rule
//! classification: `:parent` replacement, already-scoped pass-through,
//! `&`-nesting pass-through, custom-property pass-through, and unscoped
//! prefixing — recursing into `@media`, leaving other at-rules untouched.
//!
//! HOW: A small brace-matching rule splitter rather than a full CSS parser —
//! the spec names `lightningcss`, but the classification table only needs
//! rule boundaries and leading-selector inspection; a dependency-free walker
//! keeps the macro crate lean (swap point documented in the feature status
//! if real-world CSS outgrows it).

/// Transform one scoped CSS block against the parent selector
/// (`"#id"` or `".first-class"`).
pub(crate) fn transform_scoped_css(css: &str, parent_sel: &str) -> String {
    let mut out = String::with_capacity(css.len() + 64);
    transform_rules(css, parent_sel, &mut out);
    out.trim().to_string()
}

fn transform_rules(css: &str, parent_sel: &str, out: &mut String) {
    let mut rest = css;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return;
        }
        // Custom property or bare declaration at this level: `--x: y;`.
        if rest.starts_with("--") {
            let end = rest.find(';').map_or(rest.len(), |i| i + 1);
            out.push_str(&rest[..end]);
            out.push('\n');
            rest = &rest[end..];
            continue;
        }
        let Some(open) = rest.find('{') else {
            // Trailing junk without a block — emit verbatim.
            out.push_str(rest);
            return;
        };
        let selector = rest[..open].trim();
        let body_end = match matching_brace(rest, open) {
            Some(i) => i,
            None => rest.len(),
        };
        let body = &rest[open + 1..body_end];

        if let Some(stripped) = selector.strip_prefix("@media") {
            out.push_str("@media");
            out.push_str(stripped);
            out.push_str(" {\n");
            transform_rules(body, parent_sel, out); // same walk inside @media
            out.push_str("}\n");
        } else if selector.starts_with('@') {
            // @keyframes, @font-face, @supports payloads: unchanged.
            out.push_str(selector);
            out.push_str(" {");
            out.push_str(body);
            out.push_str("}\n");
        } else {
            let scoped = selector
                .split(',')
                .map(|sel| classify_and_transform(sel.trim(), parent_sel))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&scoped);
            out.push_str(" {");
            out.push_str(body);
            out.push_str("}\n");
        }
        rest = &rest[(body_end + 1).min(rest.len())..];
    }
}

/// Index of the `}` matching the `{` at `open`.
fn matching_brace(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, ch) in s.char_indices().skip(open) {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The §3 five-rule table for one selector.
fn classify_and_transform(selector: &str, parent_sel: &str) -> String {
    // Rule 1 — `:parent` is replaced with the parent identity.
    if let Some(rest) = selector.strip_prefix(":parent") {
        return format!("{parent_sel}{rest}");
    }
    // Rule 3 — `&` nesting inherits scoping from its parent rule.
    if selector.starts_with('&') {
        return selector.to_string();
    }
    // Rule 2 — already scoped: leading simple selector IS the parent identity
    // (the exact id, or — when the parent identity is a class — that class).
    let leading = selector
        .split([' ', '>', '+', '~', ':'])
        .next()
        .unwrap_or(selector);
    if leading == parent_sel {
        return selector.to_string();
    }
    // Rule 4 — unscoped: prefix with the parent identity.
    format!("{parent_sel} {selector}")
}

// In-file tests: proc-macro crates cannot export test hooks (same rationale as
// parser.rs).
#[cfg(test)]
mod tests {
    use super::*;

    /// The spec §3.1 worked example, rule by rule.
    #[test]
    fn five_rule_table() {
        let css = r"
:parent { background: white; }
:parent:hover { opacity: 0.8; }
#panel .title { font-weight: bold; }
& .nested { color: red; }
.unscoped-child { padding: 8px; }
--color-accent: #ff0;
h2 { font-size: 18px; }
";
        let out = transform_scoped_css(css, "#panel");
        assert!(out.contains("#panel { background: white; }"), "{out}");
        assert!(out.contains("#panel:hover { opacity: 0.8; }"));
        assert!(out.contains("#panel .title { font-weight: bold; }"), "already scoped untouched");
        assert!(out.contains("& .nested { color: red; }"), "nesting untouched");
        assert!(out.contains("#panel .unscoped-child { padding: 8px; }"));
        assert!(out.contains("--color-accent: #ff0;"), "custom property untouched");
        assert!(out.contains("#panel h2 { font-size: 18px; }"));
    }

    /// Class-identity parents scope by the first class.
    #[test]
    fn class_parent_already_scoped() {
        let out = transform_scoped_css(".sidebar > p { margin: 0; }", ".sidebar");
        assert_eq!(out, ".sidebar > p { margin: 0; }");
    }

    /// Compound selectors are each classified.
    #[test]
    fn compound_selectors_each_prefixed() {
        let out = transform_scoped_css(".a, :parent, .b { x: y }", "#p");
        assert_eq!(out, "#p .a, #p, #p .b { x: y }");
    }

    /// @media recurses; @keyframes passes through.
    #[test]
    fn at_rules() {
        let css = "@media (min-width: 600px) { .title { color: red } }\n@keyframes spin { from { r: 0 } }";
        let out = transform_scoped_css(css, "#p");
        assert!(out.contains("@media (min-width: 600px) {"));
        assert!(out.contains("#p .title { color: red }"), "{out}");
        assert!(out.contains("@keyframes spin { from { r: 0 } }"), "{out}");
    }
}
