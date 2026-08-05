//! F03 prompt-customization tests: static prompt override, the `prompt_color`
//! shorthand, and the dynamic-prompt callback.
//!
//! These build a real `Repl` but never call `messages()`, so no terminal is
//! touched — the renderer is lazy (no surface until the first draw) and `Drop`
//! is a no-op when nothing was ever rendered.

use std::cell::Cell;

use foundation_repl::{Repl, ReplColor};

#[test]
fn prompt_override_changes_current_prompt() {
    // AC #1: `.prompt("> ")` uses "> " instead of the default "| ".
    let repl = Repl::builder().prompt("> ").build();
    assert_eq!(repl.current_prompt(), "> ");
    assert_eq!(repl.config().prompt, "> ");
}

#[test]
fn default_prompt_is_the_configured_static_string() {
    let repl = Repl::builder().build();
    assert_eq!(repl.current_prompt(), "| ");
}

#[test]
fn prompt_color_shorthand_sets_theme_prompt_foreground() {
    // AC #2 (delivered): the shorthand recolours the theme's prompt foreground
    // using the crate's backend-neutral colour type (ansi 6 == cyan).
    let repl = Repl::builder().prompt_color(ReplColor::ansi(6)).build();
    assert_eq!(repl.config().theme.prompt_foreground, ReplColor::ansi(6));
}

#[test]
fn dynamic_prompt_receives_repl_and_varies_each_call() {
    // AC #3: the callback gets the `&Repl` and can return a different string
    // each iteration (here, a counter carried in the closure).
    let counter = Cell::new(0u32);
    let repl = Repl::builder()
        .prompt("base")
        .dynamic_prompt(move |repl| {
            counter.set(counter.get() + 1);
            // Reading a field off the passed `&Repl` proves the reference is
            // handed to the callback.
            format!("[{}] {}", counter.get(), repl.config().prompt)
        })
        .build();

    assert_eq!(repl.current_prompt(), "[1] base");
    assert_eq!(repl.current_prompt(), "[2] base");
    assert_eq!(repl.current_prompt(), "[3] base");
}

#[test]
fn no_dynamic_prompt_falls_back_to_static() {
    let repl = Repl::builder().prompt("x> ").build();
    assert_eq!(repl.current_prompt(), "x> ");
    assert_eq!(repl.current_prompt(), "x> ");
}
