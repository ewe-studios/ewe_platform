---
title: "Pi Extensions -- pi-powerline-footer"
---

# pi-powerline-footer

**Powerline-style status bar for Pi's editor with working vibes.**

pi-powerline-footer customizes the default Pi editor with a powerline-style status bar, welcome overlay, and AI-generated "vibes" for loading messages. Inspired by Powerlevel10k and oh-my-pi.

## Features

### Editor Stash

Press `Alt+S` to save your editor content and clear the editor. Type a quick prompt, and your stashed text auto-restores when the agent finishes. Toggles between stash, pop, and update-existing-stash. A stash indicator appears in the powerline bar.

### Working Vibes

AI-generated themed loading messages. Set `/vibe star trek` and your "Working..." becomes "Running diagnostics..." or "Engaging warp drive...". Supports any theme: pirate, zen, noir, cowboy, etc.

### Welcome Overlay

Branded splash screen shown as centered overlay on startup. Shows:
- Gradient logo
- Model info
- Keyboard shortcuts tips
- Loaded AGENTS.md/extensions/skills/templates counts
- Recent sessions

Auto-dismisses after 30 seconds or on any key press.

### Rounded Box Design

Status renders directly in the editor's top border, not as a separate footer.

## Screenshot

The status bar shows: model name, thinking level, active extensions, stash indicator, and current vibe theme -- all styled with Powerlevel10k-inspired segment separators.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-powerline-footer` |
| Inspiration | Powerlevel10k, oh-my-pi |
| Key binding | Alt+S (stash) |
