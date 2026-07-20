# Getting Started with foundation_shell_repl

A step-by-step guide to building an interactive REPL session.

## 1. Minimal REPL

The fastest path to a working REPL:

```rust
use foundation_shell_repl::Repl;

fn main() {
    let mut repl = Repl::new();

    for input in repl.messages() {
        repl.reply(&format!("You said: {input}"));
    }
}
```

Run it:

```
$ cargo run
>>> hello
  You said: hello
>>> world
  You said: world
>>>
```

The REPL starts with `>>> ` as the prompt. Type anything and press **Enter**
to submit. The response appears indented below, followed by a fresh prompt.

## 2. Multiline Input

Some inputs span multiple lines — function definitions, SQL queries, JSON.
The REPL handles this with two keys:

| Key | Action |
|-----|--------|
| **Enter** | Submit the current buffer |
| **Shift+Enter** | Insert a newline and keep editing |

```
>>> SELECT name, email
... FROM users
... WHERE active = 1;
  You said: SELECT name, email
  FROM users
  WHERE active = 1;
```

After Shift+Enter, the prompt changes from `>>> ` to `... ` to indicate
continuation mode. This is configurable via `ReplBuilder::continuation_prompt()`.

## 3. Command Registration

Commands let you intercept special inputs before they reach your processing
loop. Any line starting with `/` is a command:

```rust
use foundation_shell_repl::Repl;

fn main() {
    let mut repl = Repl::new();

    repl.register_command("time", |_| {
        use std::time::SystemTime;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        format!("Timestamp: {}", now.as_secs())
    });

    repl.register_command("echo", |cmd| {
        cmd.strip_prefix("/echo ").unwrap_or("").to_string()
    });

    for input in repl.messages() {
        // Only reaches here for non-command input
        repl.reply(&format!("unknown: {input}"));
    }
}
```

Built-in commands you always get:

| Command | What it does |
|---------|-------------|
| `/help` | Lists all registered commands |
| `/clear` | Clears the terminal screen |
| `/exit` | Exits the REPL session |
| `/version` | Prints the crate version |

Commands are case-insensitive: `/HELP` and `/help` both work.

### Command with arguments

The handler receives the full command string including the prefix:

```rust
repl.register_command("calc", |cmd| {
    // cmd = "/calc 2 + 2"
    let expr = cmd.strip_prefix("/calc ").unwrap_or("");
    match simple_eval(expr) {
        Ok(result) => format!("= {result}"),
        Err(e) => format!("error: {e}"),
    }
});
```

## 4. Customizing the Session

The builder lets you change every visible aspect:

```rust
use foundation_shell_repl::{Repl, ReplConfig};

let repl = Repl::builder()
    .prompt("mydb> ")                // Primary prompt
    .continuation_prompt("       ")  // Multiline continuation
    .banner("MyDB Shell v2.1\nType /help for commands\n")
    .goodbye("Saving state... Goodbye!")
    .max_input_length(128 * 1024)    // 128KB max input
    .build();
```

### With colors (feature = "colors")

```rust
use crossterm::style::Color;
use foundation_shell_repl::{Repl, ReplColors};

let repl = Repl::builder()
    .prompt("λ ")
    .colors(ReplColors {
        prompt_color: Color::Cyan,
        continuation_prompt_color: Color::DarkGray,
        response_color: Some(Color::White),
        error_color: Color::Red,
    })
    .build();
```

## 5. Session Lifecycle

The REPL runs until one of these conditions:

| Event | Result |
|-------|--------|
| User types `/exit` | Clean exit |
| User presses Ctrl+D (empty line) | Clean exit |
| User presses Ctrl+C | Interrupt, ends iteration |
| Terminal closes | Drop triggers goodbye message |

You can also trigger exit programmatically from within a command handler:

```rust
repl.register_command("quit", |_cmd| {
    repl.exit();  // Won't compile — see below
    "/exit".into()
});
```

**Note:** `Repl::exit()` requires `&self` via interior mutability, so it works
from inside the `messages()` loop without borrow conflicts.

## 6. What's Next

- [API Reference](api.md) — full type documentation
- [Commands Guide](commands.md) — patterns for custom command systems
- [Architecture](../README.md#architecture) — module layout and backend split
