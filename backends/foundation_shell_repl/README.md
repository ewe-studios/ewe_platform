# foundation_shell_repl

A lightweight, general-purpose REPL (Read-Eval-Print Loop) framework for
interactive terminal sessions. Takes user input (single or multiline), hands it
to the caller for processing, and displays the result — like Python, Ruby, or
Node's built-in REPLs, but as a reusable Rust crate anyone can build on.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
foundation_shell_repl = { path = "../backends/foundation_shell_repl" }
```

Then create a REPL in minutes:

```rust
use foundation_shell_repl::Repl;

fn main() {
    let mut repl = Repl::new();

    for input in repl.messages() {
        let response = format!("You said: {input}");
        repl.reply(&response);
    }
}
```

That's it. The REPL handles:
- **Input** — single or multiline text
- **Commands** — lines starting with `/` are intercepted
- **History** — up/down arrow to recall previous inputs
- **Display** — formatted output below the prompt

## Features

### Multiline input

Press **Shift+Enter** (or Ctrl+Enter) to insert a newline and keep editing.
Plain **Enter** submits the full buffer:

```
>>> def greet(name):
...     return f"Hello, {name}!"
...
  You said: def greet(name):
      return f"Hello, {name}!"
```

### Built-in commands

| Command | Action |
|---------|--------|
| `/help` | List available commands |
| `/clear` | Clear the terminal screen |
| `/exit` | Exit the REPL |
| `/version` | Show crate version |

### Custom commands

```rust
let mut repl = Repl::new();

repl.register_command("greet", |cmd| {
    let name = cmd.strip_prefix("/greet ").unwrap_or("world");
    format!("Hello, {name}!")
});
```

Commands starting with `/` are intercepted before reaching your processing
loop. Register as many as you need.

### Customization

Use the builder to customize prompts, banners, and colors:

```rust
use foundation_shell_repl::Repl;

let mut repl = Repl::builder()
    .prompt("myapp> ")
    .continuation_prompt("       ")
    .banner("Welcome to MyApp v1.0 — type /help for commands")
    .goodbye("Goodbye!")
    .build();

for input in repl.messages() {
    repl.reply(&process(input));
}
```

### History

The REPL keeps the last 1000 inputs in memory. Press **Up** to recall older
entries, **Down** to return to the current input. Duplicate entries are moved
to the end (most recent first). Empty submissions are ignored.

## Cross-platform

| Target | Feature | Input | Display |
|--------|---------|-------|---------|
| unix/windows | `native` (default) | crossterm raw-mode | crossterm terminal |
| wasm32 | `wasm` | stub (returns error) | console.log stub |

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `native` | ✅ | crossterm-based terminal I/O |
| `colors` | — | ANSI color support for prompts and errors |
| `history-file` | — | Save/load history to disk |
| `wasm` | — | wasm32 stub backends |

## Architecture

```
src/
├── lib.rs              # Public API, cfg-gated module selection
├── shared/             # Cross-platform core
│   ├── repl.rs         # Repl, ReplBuilder, ReplMessageIter
│   ├── traits.rs       # ReplInput, ReplDisplay traits
│   ├── config.rs       # ReplConfig, ReplColors
│   ├── commands.rs     # CommandRegistry, builtins
│   └── history.rs      # Ring buffer
├── native/             # cfg(feature = "native")
│   ├── input.rs        # crossterm raw-mode keyboard
│   └── display.rs      # crossterm terminal drawing
└── wasm/               # cfg(feature = "wasm")
    ├── input.rs        # stub (web-sys bridge placeholder)
    └── display.rs      # console.log stub
```

## Further reading

- [Getting Started](docs/getting-started.md) — detailed walkthrough with examples
- [API Reference](docs/api.md) — full type and method documentation
- [Commands Guide](docs/commands.md) — custom commands, patterns, and testing

## License

Apache-2.0
