---
title: "Pi Extensions -- pi-runtime-extensions"
---

# pi-runtime-extensions

**Load/unload Pi extensions dynamically during a running session.**

pi-runtime-extensions adds three commands for managing extensions without restarting Pi:

| Command | Action |
|---------|--------|
| `/ext:load <path>` | Load an extension into the current runtime |
| `/ext:list` | Toggle runtime extensions on/off |
| `/ext:unload` | Remove runtime extensions |

## Goal

Let you bring an extension into the current Pi runtime without restarting Pi with `-e ...`, while keeping the load temporary and easy to undo.

## Why It Exists

Normally, loading a Pi extension requires restarting Pi with the `-e` flag. For quick experiments or one-off tasks, restarting is disruptive. pi-runtime-extensions lets you load extensions on the fly and unload them when done.

## Use Cases

| Scenario | Command |
|----------|---------|
| Quick experiment | `/ext:load npm:experimental-extension` |
| Clean up after work | `/ext:unload` |
| See what's loaded | `/ext:list` |

## Install

```bash
pi install npm:pi-runtime-extensions

# or
pi -e npm:pi-runtime-extensions
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-runtime-extensions` |
| Commands | `/ext:load`, `/ext:list`, `/ext:unload` |
| Pattern | Dynamic extension management |
