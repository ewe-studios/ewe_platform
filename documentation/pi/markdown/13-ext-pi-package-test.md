---
title: "Pi Extensions -- pi-package-test"
---

# pi-package-test

**Reference package demonstrating Pi's package system features.**

pi-package-test is not a functional extension -- it's a reference implementation showing what a Pi package can include and how it's structured.

## What This Demonstrates

### Package Structure

A Pi package can include multiple resource types:

```
pi-package-test/
├── extensions/          # *.ts or *.js files (runtime extensions)
├── skills/              # Directories with SKILL.md (domain knowledge)
├── themes/              # *.json files (editor themes)
├── prompts/             # *.md files (prompt templates)
├── node_modules/        # Bundled dependencies (optional)
└── package.json         # Package metadata
```

### Resource Types

| Resource Type | Purpose | Discovered By |
|--------------|---------|---------------|
| Extensions | Runtime code (tools, commands) | Pi extension loader |
| Skills | Domain knowledge + instructions | Agent auto-discovery |
| Themes | Editor appearance | Theme picker |
| Prompts | Slash command templates | Template system |

### Use as Template

Copy this package as a starting point for your own Pi extensions:

```bash
cp -r pi-package-test my-pi-extension
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-package-test` |
| Purpose | Reference/template package |
| Functional | No -- demonstration only |
