# spec-59: foundation_shell_repl

A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
interactive terminal sessions. Takes user input (single or multiline), hands it
to the caller for processing, and displays the result — like Python, Ruby, or
Node's built-in REPLs, but as a reusable Rust crate anyone can build on.

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

## Non-goals

- Code editor features (syntax highlighting, autocomplete, bracket matching).
- Rich TUI layouts (panels, sidebars, tabs).
- Shell job control or process management.
- Network/remote REPL sessions.

## Decisions

### TUI backend: crossterm + minimal drawing

No heavy TUI framework (ratatui/tui-rs). The REPL owns the terminal via
`crossterm` (raw mode, cursor positioning, scroll region) and draws directly.
This keeps dependencies minimal and avoids fighting a framework's layout
system for what is essentially: "print prompt, read line, scroll, print
response, repeat."

### Input handling: line-by-line assembly

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
path.

## Features

| ID | Title | Status |
|----|-------|--------|
| [F01-core-repl](features/F01-core-repl/feature.md) | Core REPL engine: prompt, multiline input, reply rendering | planned |
| [F02-command-history](features/F02-command-history/feature.md) | In-memory history ring buffer + up/down navigation + optional disk persistence | planned |
| [F03-prompt-customization](features/F03-prompt-customization/feature.md) | Customizable prompt strings, continuation prompt, color/styling config | planned |
| [F04-builtin-commands](features/F04-builtin-commands/feature.md) | Help, clear, exit, version — caller-extensible command registration | planned |

## Crate impact

**`foundation_shell_repl`** — new crate:

- `lib.rs` — public API: `Repl`, `ReplBuilder`, `ReplMessage`, `ReplResponse`
- `repl.rs` — main REPL struct, `messages()` iterator, `reply()` method
- `input.rs` — raw-mode key reader, multiline buffer assembly, Enter vs
  Shift+Enter disambiguation
- `display.rs` — terminal drawing: prompt rendering, response output,
  scroll region management, cursor positioning
- `history.rs` — ring buffer, up/down traversal, optional file persistence
- `commands.rs` — built-in command registry (help, clear, exit, version),
  caller-extensible via `Repl::register_command()`
- `config.rs` — `ReplConfig` (prompt strings, colors, history depth, max
  input length)

New feature flags:
- `history-file` (optional serde + file I/O for disk persistence)
- `colors` (enabled by default — ANSI prompt/response styling)

## Reusable APIs

- **`Repl`** — the main handle. `Repl::new()` returns a ready-to-use instance
  with defaults. `Repl::builder()` allows customization.
- **`Repl::messages()`** — returns an iterator yielding `ReplMessage` (or
  `String` in simple mode). Blocks until the user presses Enter.
- **`Repl::reply(msg)`** — displays a response in the terminal, properly
  scrolling and repositioning the cursor for the next input.
- **`Repl::register_command(name, handler)`** — register a `!command` that
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
