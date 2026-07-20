# Commands Guide

How to build, register, and dispatch custom commands in your REPL.

## Built-in Commands

Every REPL starts with four built-in commands:

| Command | Type | Description |
|---------|------|-------------|
| `/help` | Output | Lists all registered commands |
| `/clear` | Terminal | Clears the screen (sends `ESC[2J ESC[H]`) |
| `/exit` | Exit | Ends the REPL session |
| `/version` | Output | Prints `foundation_shell_repl x.y.z` |

## Registering Custom Commands

Use `Repl::register_command()` before or after creating the session:

```rust
let mut repl = Repl::new();

repl.register_command("status", |_| "All systems operational".into());
repl.register_command("users", |_cmd| {
    let count = get_user_count();
    format!("{count} active users")
});
```

The handler receives the **full command line** including the `/` prefix:

```rust
repl.register_command("greet", |cmd| {
    // cmd = "/greet Alice"
    cmd.strip_prefix("/greet ")
        .map(|name| format!("Hello, {name}!"))
        .unwrap_or("Hello, world!".into())
});
```

## Command Types

Commands return a `CommandResult` that tells the REPL how to handle them:

### Output (default)

The response is printed as normal text below the prompt:

```rust
repl.register_command("time", |_| {
    format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs())
});
```

Output:
```
>>> /time
  1753123456

>>>
```

### Terminal

For raw escape sequences (like screen clearing):

```rust
repl.register_command("cls", |_| "\x1b[2J\x1b[H".into());
```

The string is written directly to stdout without any formatting.

### Exit

Ends the REPL session:

```rust
repl.register_command("quit", |cmd| {
    // This works because exit() uses &self (interior mutability)
    repl.exit();
    "Exiting...".into()
});
```

## Dispatch Rules

1. Lines starting with `/` are treated as commands
2. The command name is the first whitespace-separated word after `/`
3. Commands are case-insensitive (`/HELP` = `/help`)
4. If the command doesn't exist, an error message is printed and the
   original input is yielded to the caller (so you can log it)
5. Known commands are **not** yielded to the `messages()` iterator

## Pattern: Subcommands

For commands with subcommands, parse the arguments in the handler:

```rust
repl.register_command("config", |cmd| {
    let args = cmd.strip_prefix("/config ").unwrap_or("");
    let parts: Vec<&str> = args.split_whitespace().collect();

    match parts.as_slice() {
        ["get", key] => config.get(key).unwrap_or("not found").into(),
        ["set", key, val] => { config.set(key, val); "OK".into() }
        ["list"] => config.dump(),
        [] => "Usage: /config get|set|list".into(),
        _ => "Unknown subcommand".into(),
    }
});
```

## Pattern: Exit from within a command

Since `exit()` and `register_command()` use interior mutability (`&self`),
they can be called from inside the `messages()` loop:

```rust
repl.register_command("restart", |cmd| {
    repl.exit();
    "Restarting...".into()
});

for input in repl.messages() {
    if input == "shutdown" {
        repl.reply("Shutting down.");
        repl.exit();
        break;
    }
    repl.reply(&process(input));
}
```

## Testing Commands

The `CommandRegistry` is public, so you can test commands without a terminal:

```rust
#[test]
fn test_greet_command() {
    let mut reg = CommandRegistry::new();
    reg.register("greet", |cmd| {
        let name = cmd.strip_prefix("/greet ").unwrap_or("world");
        format!("Hello, {name}!")
    });

    let result = reg.try_handle("/greet Alice").unwrap();
    assert!(matches!(result, CommandResult::Output(text) if text == "Hello, Alice!"));
}
```
