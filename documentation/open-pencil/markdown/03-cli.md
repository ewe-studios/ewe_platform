# OpenPencil -- CLI

## Overview

The OpenPencil CLI (`@open-pencil/cli`) provides headless access to design files from the terminal. It works with both `.fig` and `.pen` files and can also connect to the running desktop app via RPC for live-canvas operations.

```sh
bun add -g @open-pencil/cli
```

All commands support `--json` for machine-readable output, making them suitable for CI pipelines and automation scripts.

## Commands

### tree

Display the node tree of a design file.

```sh
open-pencil tree design.fig
open-pencil tree design.pen --depth 3
```

```
[0] [page] "Getting started" (0:46566)
  [0] [section] "" (0:46567)
    [0] [frame] "Body" (0:46568)
      [0] [frame] "Introduction" (0:46569)
        [0] [frame] "Introduction Card" (0:46570)
          [0] [frame] "Guidance" (0:46571)
```

### find

Search for nodes by name or type.

```sh
open-pencil find design.pen --type TEXT
open-pencil find design.fig --name "Button"
```

### node

Get details of a specific node by ID.

```sh
open-pencil node design.fig --id 1:23
open-pencil node design.fig --id 1:23 --json
```

### info

Show file metadata: page count, node counts, file format info.

```sh
open-pencil info design.fig
```

### query

Execute XPath queries against the scene graph.

```sh
open-pencil query design.fig "//FRAME"
open-pencil query design.fig "//FRAME[@width < 300]"
open-pencil query design.fig "//TEXT[contains(@name, 'Button')]"
open-pencil query design.fig "//*[@cornerRadius > 0]"
open-pencil query design.fig "//SECTION//TEXT"
```

### export

Render nodes to various output formats.

```sh
# Raster exports
open-pencil export design.fig                              # PNG, all pages
open-pencil export design.fig -f jpg -s 2 -q 90            # JPG at 2x, quality 90
open-pencil export design.fig -f webp --selection          # WEBP of selection

# Vector exports
open-pencil export design.fig -f svg                       # SVG

# Code exports
open-pencil export design.fig -f jsx --style tailwind      # Tailwind JSX
open-pencil export design.fig -f jsx --style inline        # Inline style JSX

# Figma export
open-pencil export design.fig -f fig --page "Page 1"       # Export a page as .fig
```

### convert

Convert between supported document formats.

```sh
open-pencil convert design.pen output.fig     # .pen to .fig
open-pencil convert design.fig output.pen     # .fig to .pen
```

### lint

Check design files for quality issues.

```sh
open-pencil lint design.fig
open-pencil lint design.pen --preset strict
open-pencil lint design.fig --rule color-contrast
open-pencil lint design.fig --list-rules
```

**Lint output example:**

```
design.fig
  WARN  [no-default-names] Frame "Frame 1" has a default name  page:1, id:0:46571
  WARN  [no-empty-frames] Frame "" has no children              page:1, id:0:46572
  WARN  [min-text-size] Text "Note" is 10px (min 12px)         page:1, id:0:46573
```

### analyze

Audit design systems from the terminal.

```sh
open-pencil analyze colors design.fig         # Color palette audit
open-pencil analyze typography design.fig     # Typography audit
open-pencil analyze spacing design.fig        # Spacing consistency
open-pencil analyze clusters design.fig       # Repeated component patterns
```

**Colors output:**

```
#1d1b20  ██████████████████████████████ 17155×
#49454f  ██████████████████████████████ 9814×
#ffffff  ██████████████████████████████ 8620×
#6750a4  ██████████████████████████████ 3967×
```

**Clusters output:**

```
3771× frame "container" (100% match)
     size: 40×40, structure: Frame > [Frame]

2982× instance "Checkboxes" (100% match)
     size: 48×48, structure: Instance > [Frame]
```

### eval

Execute Figma Plugin API scripts against a design file.

```sh
# Read-only
open-pencil eval design.fig -c "figma.currentPage.children.length"

# Modify and write back
open-pencil eval design.fig -c "figma.currentPage.selection.forEach(n => n.opacity = 0.5)" -w
```

### variables

Extract design tokens (variables) from a file.

```sh
open-pencil variables design.fig
open-pencil variables design.fig --json
```

### pages

List pages in a design file.

```sh
open-pencil pages design.fig
```

### selection

Show currently selected nodes (when connected to running app).

```sh
open-pencil selection
```

### formats

List supported import/export formats.

```sh
open-pencil formats
```

## Live Mode

When the desktop app is running, omit the file argument to connect via RPC and operate on the live canvas:

```sh
open-pencil tree                               # Inspect the live document
open-pencil export -f png                      # Screenshot the current canvas
open-pencil eval -c "figma.currentPage.name"   # Query the editor
```

This enables automation scripts, CI pipelines, and AI agents to interact with the editor in real time.

## JSON Output

Every command supports `--json` for machine-readable output:

```sh
open-pencil tree design.fig --json
open-pencil query design.fig "//FRAME" --json
open-pencil lint design.fig --json
open-pencil analyze colors design.fig --json
```

## Architecture

The CLI is built on [citty](https://github.com/unjs/citty) for command definition and parsing. Each command:

1. Loads the file via `@open-pencil/core` I/O system
2. Parses the Kiwi/ZIP format into a scene graph
3. Executes the operation headlessly (no renderer needed for most commands)
4. Outputs results to stdout (text or JSON)

For live-mode commands, the CLI connects to the running desktop app via WebSocket RPC instead of loading files.

## See Also

- [Core Engine](02-core-engine.md) -- The engine the CLI operates on
- [AI & MCP](04-ai-mcp.md) -- MCP server for agent-based CLI access
