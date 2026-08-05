# spec-59: foundation_repl

> **Delivered as the crate `foundation_repl`** (v0.2.x), not `foundation_shell_repl`.
> This spec's original "Decisions" sketched a crossterm-direct, `ReplColors`
> design; the crate actually shipped on a **ratatui inline viewport** (crossterm
> host on native, ratzilla host in the browser) with the **`ReplTheme`** colour
> system. The **Delivered design** notes below are authoritative where they
> differ from the older prose. All four features are complete; see each
> `feature.md`.

A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
interactive terminal sessions. Takes user input (single or multiline), hands it
to the caller for processing, and displays the result — like Python, Ruby, or
Node's built-in REPLs, but as a reusable Rust crate anyone can build on.

Compiles for both native (`cfg(unix)` / `cfg(windows)`) and `wasm32` targets.
The shared core types and API are identical across platforms; the input and
display backends swap via cfg gates and the `ReplHost` trait.

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

### Rendering backend: ratatui inline viewport + `ReplHost` (delivered)

The input area is drawn as a bordered, padded box measured into a cell buffer
and flushed through **ratatui's `Backend`** — not painted directly with
crossterm escape codes as the original draft proposed. A rendering surface is
added by implementing **`ReplHost`** (create a backend, report size, say whether
it has its own scrollback); `ReplRenderer` does the rest. Two hosts ship:
`CrosstermHost` (native terminal) and `RatzillaHost` (browser, `ratzilla`
feature). `ViewportMode::{Inline,Fullscreen}` picks the output strategy. Turning
the `colors` feature off keeps the box and layout, dropping only styling.

### Module layout: shared/ + native/ + wasm/ (delivered)

```
src/
  lib.rs            — public API, cfg-gated module + re-exports
  shared/
    config.rs       — ReplConfig (holds a ReplTheme)
    theme.rs        — ReplTheme, ReplColor, BorderKind, ReplPadding, ActivityStyle
    history.rs      — ring buffer + optional history-file load/save
    commands.rs     — command registry
    repl.rs         — Repl, ReplBuilder, ReplMessageIter, PromptFn
    traits.rs       — ReplInput, ReplDisplay, SharedDisplay
    layout.rs       — box/wrapping/window geometry
    render.rs       — ReplRenderer over a ReplHost
    host.rs         — ReplHost, ViewportMode
    activity.rs     — Activity indicator (spinner / progress / stream sink)
  native/           — cfg(native): CrosstermHost, NativeInput, NativeDisplay
  wasm/             — cfg(wasm|ratzilla): WasmInput/WasmDisplay, RatzillaHost
```

### Shared traits for input and display (delivered)

`shared/repl.rs` is backend-agnostic. `ReplInput` reads keystrokes and assembles
multiline input; `ReplDisplay` renders the input box, activity, streamed output,
responses, errors and banner (a richer shape than the original per-keystroke
sketch — the renderer redraws the whole input area from the buffer each frame):

```rust
pub trait ReplInput {
    fn new() -> Self where Self: Sized;
    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
        history: Option<&mut ReplHistory>,
    ) -> std::io::Result<String>;
    fn eof(&self) -> bool;
}

pub trait ReplDisplay {
    fn render_input(&mut self, view: &InputView);
    fn finish_input(&mut self, view: &InputView);
    fn render_activity(&mut self, view: &ActivityView);
    fn stream_push(&mut self, text: &str);
    fn print_response(&mut self, response: &str);
    fn print_error(&mut self, message: &str);
    fn print_banner(&mut self, banner: &str);
    fn clear_screen(&mut self);
    fn shutdown(&mut self);
}
```

### Input handling: whole-area redraw (native)

Keystrokes are read via crossterm events. The buffer and caret are owned
together, so arrow keys, Home/End, Ctrl+W/Ctrl+U and history recall all agree
with what is on screen. Shift+Enter (or any modifier + Enter) inserts a newline;
plain Enter submits the whole buffer as one message. Ctrl+C abandons the line;
Ctrl+D on an empty line ends the session.

### Iterator-based API

The REPL yields input via `Iterator<Item = String>` (`Repl::messages()`). The
caller owns the processing logic — the REPL is the I/O layer. `Repl::reply()`
prints responses; `Repl::animation()` shows work in progress and doubles as a
streaming sink.

### History

In-memory ring buffer of last N inputs (default 1000). Up/Down arrows traverse
history. Optional disk persistence via the `history-file` feature
(`ReplHistory::{load_from_file,save_to_file}`, JSON), native-only.

## Features

| ID | Title | Status |
|----|-------|--------|
| [F01-core-repl](features/F01-core-repl/feature.md) | Shared traits + native input/display + wasm stubs + REPL loop | complete |
| [F02-command-history](features/F02-command-history/feature.md) | In-memory history ring buffer + up/down navigation + optional disk persistence (native-only) | complete |
| [F03-prompt-customization](features/F03-prompt-customization/feature.md) | Customizable prompt strings, continuation prompt, theme/colour config, dynamic prompt | complete |
| [F04-builtin-commands](features/F04-builtin-commands/feature.md) | Help, clear, exit, version — caller-extensible command registration | complete |

## Crate impact

**`foundation_repl`** — new crate (see the delivered module layout in Decisions).

Feature flags (delivered):
- `native` (default) — crossterm + ratatui terminal I/O
- `colors` (default) — colour the box, prompt and output shading
- `history-file` — serde + file I/O for disk persistence (native only)
- `wasm` — wasm32 stub backends (js-sys/web-sys)
- `ratzilla` — browser rendering via ratzilla (use instead of `native`)

Conditional dependencies:
- `crossterm` / `ratatui` — native rendering
- `ratzilla` — browser rendering
- `js-sys` / `web-sys` — wasm feature
- `serde` / `serde_json` — history-file feature

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
use foundation_repl::Repl;

fn main() {
    let repl = Repl::new();

    // Command handlers take the command line and return the response text.
    repl.register_command("hello", |_cmd| "👋 Hello from the REPL!".to_string());

    for input in repl.messages() {
        repl.reply(&format!("You said: {input}"));
    }
}
```
