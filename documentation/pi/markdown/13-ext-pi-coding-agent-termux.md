---
title: "Pi Extensions -- pi-coding-agent-termux"
---

# pi-coding-agent-termux

**DEPRECATED** -- Termux support has been merged upstream.

This extension was a Termux port of the Pi coding agent, enabling mobile development on Android devices via the Termux terminal emulator.

## Migration Path

As of Pi v0.51.0, Termux support is built into the upstream Pi package directly. This extension is no longer needed.

```bash
# Uninstall this port
npm uninstall -g @vaclav-synacek/pi-coding-agent-termux

# Use upstream directly
npm install -g @mariozechner/pi-coding-agent
```

## Historical Context

This extension existed because the upstream Pi package had incompatibilities with Termux's Node.js environment (different library paths, missing system dependencies). The upstream integration resolved these issues natively.
