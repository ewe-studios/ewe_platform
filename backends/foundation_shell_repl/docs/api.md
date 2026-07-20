# API Reference

Core types and methods for building REPL sessions.

## Repl

The main handle for a REPL session.

```rust
pub struct Repl { /* opaque */ }
```

### Construction

```rust
// Default: | prompt, no banner, 64KB max input
let repl = Repl::new();

// Custom config
let repl = Repl::with_config(ReplConfig {
    prompt: "app> ".into(),
    continuation_prompt: "    ".into(),
    banner: Some("Welcome!".into()),
    goodbye: Some("Bye!".into()),
    max_input_length: Some(128 * 1024),
    ..Default::default()
});

// Builder (preferred)
let repl = Repl::builder()
    .prompt("λ ")
    .banner("REPL v1.0")
    .build();
```

### Methods

#### `messages() -> ReplMessageIter<'_>`

Returns an iterator that yields complete user messages. Blocks on each
iteration until the user submits input (Enter) or cancels (Ctrl+C/Ctrl+D).

```rust
for input in repl.messages() {
    // input is a String, possibly multiline
    repl.reply(&process(input));
}
```

#### `reply(response: impl AsRef<str>)`

Displays a response below the current prompt, then redraws the prompt
so the cursor is ready for the next input.

```rust
repl.reply("Hello, world!");
// Output:
//
//   Hello, world!
//
// >>>
```

#### `register_command(name, handler)`

Registers a command that intercepts `/name` lines before they reach
the `messages()` iterator.

```rust
repl.register_command("status", |_| "All systems operational".into());
```

The handler receives the full command line (e.g. `/status --verbose`)
and returns the response string.

#### `exit()`

Signals the REPL to end after the current iteration.

```rust
repl.register_command("quit", |_cmd| {
    repl.exit();
    String::new()
});
```

---

## ReplBuilder

Fluent builder for customizing a REPL before creation.

```rust
pub struct ReplBuilder { /* opaque */ }
```

| Method | Default | Description |
|--------|---------|-------------|
| `prompt(s)` | `"| "` | Primary prompt string |
| `continuation_prompt(s)` | `"... "` | Multiline continuation prompt |
| `banner(s)` | `None` | Welcome message on startup |
| `goodbye(s)` | `None` | Farewell message on drop |
| `max_input_length(n)` | `65536` | Max input length in chars |
| `colors(c)` | default colors | Color configuration (requires `colors` feature) |
| `build()` | — | Create the `Repl` |

---

## ReplConfig

Configuration struct (used by `Repl::with_config` and `ReplBuilder`).

```rust
pub struct ReplConfig {
    pub prompt: String,
    pub continuation_prompt: String,
    pub banner: Option<String>,
    pub goodbye: Option<String>,
    pub max_input_length: Option<usize>,
    pub colors: ReplColors,  // only with `colors` feature
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: "| ".into(),
            continuation_prompt: "... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
            colors: ReplColors::default(),
        }
    }
}
```

---

## ReplColors

Color configuration for prompts and output (requires `colors` feature).

```rust
pub struct ReplColors {
    pub prompt_color: Color,               // default: DarkGreen
    pub continuation_prompt_color: Color,  // default: DarkYellow
    pub response_color: Option<Color>,     // default: None
    pub error_color: Color,                // default: Red
}
```

Uses `crossterm::style::Color` — any crossterm color works.

---

## ReplMessageIter

The iterator returned by `Repl::messages()`. Yields `String` values.

```rust
impl Iterator for ReplMessageIter<'_> {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item>;
}
```

### When iteration ends

| Condition | Returns |
|-----------|---------|
| User presses Enter with text | `Some(message)` |
| User presses Ctrl+D on empty line | `None` |
| User presses Ctrl+C | `None` |
| User types `/exit` | `None` |
| `Repl::exit()` was called | `None` |

### Command handling

Lines starting with `/` are intercepted:
- `/help`, `/clear`, `/version` → response printed, `None` returned
- `/exit` → REPL exits, `None` returned
- Unknown `/cmd` → error printed, `Some(cmd)` returned (so you can log it)

---

## CommandRegistry

The internal command registry (exposed for testing and advanced use).

```rust
pub struct CommandRegistry { /* opaque */ }

impl CommandRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, name: impl Into<String>, handler: CommandHandler);
    pub fn try_handle(&self, line: &str) -> Option<CommandResult>;
}
```

---

## CommandResult

The result of command dispatch.

```rust
pub enum CommandResult {
    /// Print as a normal response.
    Output(String),
    /// Send raw escape sequences to the terminal (e.g. `/clear`).
    Terminal(String),
    /// Exit the REPL.
    Exit,
}
```

---

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `native` | ✅ | Enables crossterm-based terminal I/O for unix/windows |
| `colors` | — | ANSI color support for prompts and errors (implies `native`) |
| `history-file` | — | Save/load history to disk via `ReplHistory::load_from_file` / `save_to_file` |
| `wasm` | — | wasm32 stub backends (input returns error, display uses console.log) |
