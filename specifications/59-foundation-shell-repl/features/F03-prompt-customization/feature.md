---
workspace_name: "ewe_platform"
spec_directory: "specifications/59-foundation-shell-repl"
feature_directory: "specifications/59-foundation-shell-repl/features/F03-prompt-customization"
this_file: "specifications/59-foundation-shell-repl/features/F03-prompt-customization/feature.md"

status: planned
priority: medium
created: 2026-07-20

depends_on: ["F01-core-repl"]

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# F03 — Prompt customization: custom strings, continuation prompt, color/styling config

## Overview

Callers can customize every visible aspect of the REPL: prompt text,
continuation prompt, banner, goodbye message, and foreground colors for
prompts, responses, and errors.

[spec](../spec.md).

---

## Part A — Extended ReplConfig

```rust
// foundation_shell_repl/src/config.rs (extended)

use crossterm::style::Color;

/// Color configuration for the REPL display.
#[derive(Debug, Clone)]
pub struct ReplColors {
    /// Prompt foreground color. Default: DarkGreen.
    pub prompt_color: Color,
    /// Continuation prompt foreground color. Default: DarkYellow.
    pub continuation_prompt_color: Color,
    /// Response text foreground color. Default: default (no change).
    pub response_color: Option<Color>,
    /// Error text foreground color. Default: Red.
    pub error_color: Color,
}

impl Default for ReplColors {
    fn default() -> Self {
        Self {
            prompt_color: Color::DarkGreen,
            continuation_prompt_color: Color::DarkYellow,
            response_color: None,
            error_color: Color::Red,
        }
    }
}

/// Full configuration for a REPL session.
#[derive(Debug, Clone)]
pub struct ReplConfig {
    pub prompt: String,
    pub continuation_prompt: String,
    pub banner: Option<String>,
    pub goodbye: Option<String>,
    pub max_input_length: Option<usize>,
    pub colors: ReplColors,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: ">>> ".into(),
            continuation_prompt: "... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
            colors: ReplColors::default(),
        }
    }
}
```

## Part B — Builder API

```rust
impl ReplBuilder {
    pub fn colors(mut self, colors: ReplColors) -> Self {
        self.config.colors = colors;
        self
    }

    /// Shorthand: set the prompt color directly.
    pub fn prompt_color(mut self, color: Color) -> Self {
        self.config.colors.prompt_color = color;
        self
    }
}
```

## Part C — Dynamic prompt callback

For advanced use cases, a callback that generates the prompt dynamically
(e.g., showing a counter, timestamp, or current state):

```rust
/// Optional callback invoked before each input to generate the prompt string.
pub type PromptFn = Box<dyn Fn(&Repl) -> String>;

impl ReplBuilder {
    pub fn dynamic_prompt(mut self, f: impl Fn(&Repl) -> String + 'static) -> Self {
        self.config.dynamic_prompt = Some(f);
        self
    }
}
```

When set, the callback replaces the static `prompt` string for the primary
prompt only. The `continuation_prompt` remains static.

## Acceptance criteria

1. `Repl::builder().prompt("> ").build()` uses `> ` instead of `>>> `.
2. `Repl::builder().prompt_color(Color::Cyan).build()` renders the prompt in
   cyan.
3. A dynamic prompt callback receives the `Repl` reference and returns a
   different string each iteration.
