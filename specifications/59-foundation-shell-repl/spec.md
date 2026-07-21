# spec-59: foundation_shell_repl

A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
interactive terminal sessions. Takes user input (single or multiline), hands it
to the caller for processing, and displays the result — like Python, Ruby, or
Node's built-in REPLs, but as a reusable Rust crate anyone can build on.

Compiles for both native (`cfg(unix)` / `cfg(windows)`) and `wasm32` targets.
The shared core types and API are identical across platforms; the input and
display modules swap to native (stdin/stdout/raw-mode) or wasm (stub/JS-bridge)
implementations via `target_family` cfg gates.

## Goals

1. **Simple API** — a minimal loop: create the REPL, iterate on user messages,
   reply with output. No framework boilerplate, no complex widget trees.
2. **Multiline input** — Shift+Enter inserts a newline; Enter submits the full
   text. The caller receives one complete string per submission, regardless of
   line count.
3. **Clean TUI** — readable prompt, input area, response area. No syntax
   highlighting, autocomplete, or panels. Just a terminal that feels like a
   proper REPL session.
4. **Extensible** — callers can customize prompts, colors, history depth, and
   register simple commands (help, clear, version).
5. **Cross-platform** — compiles on `cfg(unix)`, `cfg(windows)`, and `wasm32`.
   Native uses stdin/stdout + crossterm raw mode; wasm uses stub/JS-bridge
   backends (partial support is acceptable — the API stays the same).

## Non-goals

- Code editor features (syntax highlighting, autocomplete, bracket matching).
- Rich TUI layouts (panels, sidebars, tabs).
- Shell job control or process management.
- Network/remote REPL sessions.
- Full wasm terminal parity — wasm backends may be stubs or minimal bridges.

## Decisions

### TUI backend: crossterm (native) + wasm stub/bridge

No heavy TUI framework (ratatui/tui-rs). On native targets the REPL owns the
terminal via `crossterm` (raw mode, cursor positioning, scroll region) and
draws directly. On `wasm32`, crossterm is not available — the display and
input modules use a `js-sys` / `web-sys` based stub or a plain text buffer.
Wasm support need not be fully featured; the public API remains identical.

### Module layout: shared/ + native/ + wasm/

The crate uses `target_family` cfg gates to pick the right backend:

```
src/
  lib.rs            — public API, cfg-gated module selection
  shared/
    config.rs       — ReplConfig, ReplColors
    history.rs      — ring buffer
    commands.rs     — command registry
    repl.rs         — Repl, ReplBuilder, ReplMessageIter (uses traits)
  native/
    mod.rs          — cfg(any(unix, windows))
    input.rs        — crossterm raw-mode key reader
    display.rs      — crossterm terminal drawing
  wasm/
    mod.rs          — cfg(target_family = "wasm")
    input.rs        — stub / web-sys key listener
    display.rs      — stub / console.log-based output
```

### Shared traits for input and display

`shared/repl.rs` doesn't know about crossterm or web-sys. It uses two traits:

```rust
/// Reads keystrokes and assembles multiline input.
pub trait ReplInput: Sized {
    fn new() -> Self;
    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
    ) -> io::Result<String>;
    fn eof(&self) -> bool;
}

/// Renders prompts, responses, and cursor movements.
pub trait ReplDisplay {
    fn new() -> Self where Self: Sized;
    fn draw_prompt(&mut self, prompt: &str, buffer: &str);
    fn redraw_with_newline(&mut self, prompt: &str, buffer: &str);
    fn append_char(&mut self, c: char, buffer: &str);
    fn backspace(&mut self, buffer: &str);
    fn clear_line(&mut self, prompt: &str);
    fn newline(&mut self);
    fn print_response(&mut self, response: &str);
    fn print_banner(&mut self, banner: &str);
    fn print_error(&mut self, msg: &str);
    fn move_cursor_left(&mut self);
    fn move_cursor_right(&mut self);
}
```

The native module provides `native::input::NativeInput` and
`native::display::NativeDisplay` implementing these traits. The wasm module
provides `wasm::input::WasmInput` and `wasm::display::WasmDisplay` — the wasm
versions may be no-op stubs or minimal console-based implementations.

### Input handling: line-by-line assembly (native)

Each keypress is read via `crossterm::event::poll/read`. Printable characters
are appended to a buffer. Shift+Enter (or Ctrl+J/Enter) appends a `\n` to the
buffer and continues. Plain Enter flushes the buffer as one complete message
to the iterator consumer.

### Iterator-based API

The REPL yields input via an `Iterator<Item = String>` (or `Iterator<Item =
ReplMessage>` for richer types). The caller owns the processing logic — the
REPL is purely the I/O layer.

### History

In-memory ring buffer of last N inputs (default 1000). Up/Down arrows traverse
history. No disk persistence by default — callers can enable it with an optional
path. The disk persistence feature is `native`-only (wasm has no filesystem).

## Features

| ID | Title | Status |
|----|-------|--------|
| [F01-core-repl](features/F01-core-repl/feature.md) | Shared traits + native input/display + wasm stubs + REPL loop | implemented |
| [F02-command-history](features/F02-command-history/feature.md) | In-memory history ring buffer + up/down navigation + optional disk persistence (native-only) | implemented |
| [F03-prompt-customization](features/F03-prompt-customization/feature.md) | Customizable prompt strings, continuation prompt, color/styling config | implemented |
| [F04-builtin-commands](features/F04-builtin-commands/feature.md) | Help, clear, exit, version — caller-extensible command registration | implemented |

## Crate impact

**`foundation_shell_repl`** — new crate:

```
src/
  lib.rs            — cfg-gated module selection, re-exports shared API
  shared/
    config.rs       — ReplConfig, ReplColors
    history.rs      — ring buffer
    commands.rs     — command registry
    repl.rs         — Repl, ReplBuilder, ReplMessageIter, traits
  native/
    mod.rs          — cfg(any(unix, windows))
    input.rs        — crossterm raw-mode key reader (ReplInput impl)
    display.rs      — crossterm terminal drawing (ReplDisplay impl)
  wasm/
    mod.rs          — cfg(target_family = "wasm")
    input.rs        — stub / web-sys key listener (ReplInput impl)
    display.rs      — stub / console.log output (ReplDisplay impl)
```

Feature flags:
- `history-file` (optional serde + file I/O for disk persistence — native only)
- `colors` (enabled by default — ANSI prompt/response styling, native only)
- `wasm` (gates wasm32 compilation; pulls in js-sys/web-sys when present)

Conditional dependencies:
- `crossterm` — only on `cfg(not(target_family = "wasm"))`
- `js-sys` / `web-sys` — only on `cfg(target_family = "wasm")` + `wasm` feature

## Reusable APIs

- **`Repl`** — the main handle. `Repl::new()` returns a ready-to-use instance
  with defaults. `Repl::builder()` allows customization.
- **`Repl::messages()`** — returns an iterator yielding `String`. Blocks until
  the user presses Enter.
- **`Repl::reply(msg)`** — displays a response in the terminal, properly
  scrolling and repositioning the cursor for the next input.
- **`Repl::register_command(name, handler)`** — register a `/command` that
  the REPL intercepts before yielding to the caller.

## Example

```rust
use foundation_shell_repl::Repl;

fn main() {
    let mut repl = Repl::new();

    repl.register_command("hello", |_repl| {
        "👋 Hello from the REPL!".to_string()
    });

    for input in repl.messages() {
        let response = format!("You said: {}", input);
        repl.reply(&response);
    }
}
```
