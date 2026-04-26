---
title: "Pi Extensions -- pi-interactive-shell"
---

# pi-interactive-shell

**Run interactive CLIs in an observable TUI overlay.**

pi-interactive-shell lets Pi autonomously run interactive CLIs (editors, REPLs, database shells, long-running processes) in an observable overlay. Pi controls the subprocess while you watch -- take over anytime.

## Tool API

```typescript
interactive_shell({ command: 'vim config.yaml' })
```

## Why It Exists

Some tasks need interactive CLIs -- editors, REPLs, database shells, long-running processes. Standard bash tools can't handle these. pi-interactive-shell provides an overlay where:

- Pi controls the subprocess (sends keystrokes, reads output)
- You watch in real-time through a TUI overlay
- You can take over control at any point
- Pi can return control to you gracefully

## Use Cases

| Scenario | Example |
|----------|---------|
| Code editing | `vim src/main.py` |
| Database shells | `psql -d production` |
| REPLs | `python`, `node`, `ruby` |
| Interactive installers | `npm init`, `cargo init` |
| Long-running processes | `tail -f logs`, `htop` |

## Requirements

Requires the Pi TUI framework. The interactive shell runs as a subprocess within Pi's event system.

## Install

```bash
pi install npm:pi-interactive-shell
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-interactive-shell` |
| Requires | Pi TUI framework |
| Tool | `interactive_shell` |
| Control | Agent-driven with user takeover |
