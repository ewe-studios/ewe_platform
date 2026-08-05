---
workspace_name: "ewe_platform"
spec_directory: "specifications/59-foundation-shell-repl"
feature_directory: "specifications/59-foundation-shell-repl/features/F03-prompt-customization"
this_file: "specifications/59-foundation-shell-repl/features/F03-prompt-customization/feature.md"

status: complete
priority: medium
created: 2026-07-20
completed: 2026-07-31

depends_on: ["F01-core-repl"]

tasks:
  completed: 3
  uncompleted: 0
  total: 3
  completion_percentage: 100%
---

# F03 — Prompt customization: custom strings, continuation prompt, colour/styling config

## Overview

Callers can customize every visible aspect of the REPL: prompt text,
continuation prompt, banner, goodbye message, and the colours used for prompts,
input, responses, errors and chrome. Advanced callers can also supply a callback
that generates the primary prompt dynamically.

[spec](../spec.md).

> **Delivered as `foundation_repl`** (not `foundation_shell_repl`). The colour
> model is the theme system from F01 (`ReplTheme` + backend-neutral `ReplColor`),
> not the crossterm-`ReplColors` sketch that appears in older drafts — see the
> Delivered design below, which is authoritative.

---

## Part A — Colours via `ReplTheme` (delivered)

Colour lives in `ReplTheme` (see F01), whose fields are backend-neutral
`ReplColor` values (`Default` / `Ansi(u8)` / `Rgb(r,g,b)`) so no terminal
library leaks into the public API. `ReplTheme` is a strict superset of the old
`ReplColors` sketch — it carries prompt, input, output, error, banner, border
and activity colours, four named palettes (`nebula`/`dusk`/`abyss`/`slate`), and
`by_name`/`named`/`from_env` selection.

`ReplConfig` (delivered) holds the prompt strings, banner/goodbye, input-length
ceiling, and the `theme`:

```rust
pub struct ReplConfig {
    pub prompt: String,              // default "| "
    pub continuation_prompt: String, // default "|... "
    pub banner: Option<String>,
    pub goodbye: Option<String>,
    pub max_input_length: Option<usize>, // default Some(64*1024)
    pub theme: ReplTheme,
}
```

`ReplConfig` stays a plain data value (`Clone + Debug + PartialEq + Eq`); the
dynamic-prompt closure therefore lives on `Repl`/`ReplBuilder`, not in the config
(Part C).

## Part B — Builder API (delivered)

```rust
impl ReplBuilder {
    pub fn prompt(self, s: impl Into<String>) -> Self;
    pub fn continuation_prompt(self, s: impl Into<String>) -> Self;
    pub fn banner(self, s: impl Into<String>) -> Self;
    pub fn goodbye(self, s: impl Into<String>) -> Self;
    pub fn max_input_length(self, n: usize) -> Self;
    pub fn theme(self, theme: ReplTheme) -> Self;

    /// Shorthand: recolour just the prompt on the current theme.
    /// `ReplColor::ansi(6)` is cyan.
    pub fn prompt_color(self, color: ReplColor) -> Self;

    /// Advanced: callback that generates the primary prompt each read (Part C).
    pub fn dynamic_prompt(self, f: impl Fn(&Repl) -> String + 'static) -> Self;
}
```

## Part C — Dynamic prompt callback (delivered)

A callback that generates the primary prompt just before each read (e.g. a
counter, timestamp, or current state):

```rust
/// Callback invoked before each read to generate the primary prompt string.
pub type PromptFn = Box<dyn Fn(&Repl) -> String>;

impl Repl {
    /// The prompt to draw next: the dynamic callback's result if set, else the
    /// static `config.prompt`. Public so callers/renderers can read it too.
    pub fn current_prompt(&self) -> String;
}
```

Stored on `Repl` (and `ReplBuilder`), **not** in `ReplConfig`, because a closure
is not `Clone`/`Debug`/`Eq`. When set, it replaces the static `prompt` for the
primary prompt only; the `continuation_prompt` stays static. A stateful callback
(interior mutability) can return a different string each iteration. The message
loop calls `current_prompt()` before borrowing input/display so the callback
sees a fully-available `&Repl`.

## Acceptance criteria (delivered)

1. `Repl::builder().prompt("> ").build()` uses `> ` — `current_prompt()` returns
   `"> "`. ✅ (`tests/prompt.rs::prompt_override_changes_current_prompt`)
2. `Repl::builder().prompt_color(ReplColor::ansi(6)).build()` sets the theme's
   `prompt_foreground` to cyan. ✅
   (`tests/prompt.rs::prompt_color_shorthand_sets_theme_prompt_foreground`)
   *(Reconciled: the delivered colour type is the backend-neutral `ReplColor`,
   not `crossterm::style::Color`.)*
3. A dynamic prompt callback receives the `&Repl` and returns a different string
   each iteration. ✅
   (`tests/prompt.rs::dynamic_prompt_receives_repl_and_varies_each_call`)
