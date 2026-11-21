use foundation_ui_wire::vocab::TokenTable;

#[test]
fn token_ids_and_css() {
    let mut tokens = TokenTable::new();
    assert_eq!(tokens.to_css_aliases(), ":root{}");
    assert_eq!(tokens.color("--color-primary"), 1);
    assert_eq!(tokens.font("--font-body"), 1);
    assert_eq!(tokens.color("--color-accent"), 2);
    assert_eq!(tokens.color("--color-primary"), 3);
    assert_eq!(tokens.color_id("--color-primary"), Some(1));
    assert_eq!(tokens.color_id("--color-accent"), Some(2));
    assert_eq!(tokens.font_id("--font-body"), Some(1));
    assert_eq!(tokens.color_id("--font-body"), None);
    assert_eq!(tokens.font_id("--color-primary"), None);
    assert_eq!(
        tokens.to_css_aliases(),
        concat!(
            ":root{--ewe-color-1:var(--color-primary);--ewe-color-2:var(--color-accent);",
            "--ewe-color-3:var(--color-primary);--ewe-font-1:var(--font-body);}",
        ),
    );
}
