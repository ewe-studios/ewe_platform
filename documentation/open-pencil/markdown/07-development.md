# OpenPencil -- Development

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) (package manager)
- [Rust](https://rustup.rs/) (for Tauri desktop builds)
- Platform-specific Tauri prerequisites ([Tauri v2 guide](https://v2.tauri.app/start/prerequisites/))

### Setup

```sh
bun install
bun run dev        # Dev server at localhost:1420
bun run tauri dev  # Desktop app (requires Rust)
```

### Project Structure

```
packages/
  core/           @open-pencil/core -- Engine (scene graph, renderer, layout, file formats, tools)
  vue/            @open-pencil/vue  -- Headless Vue SDK
  cli/            @open-pencil/cli  -- Headless CLI
  mcp/            @open-pencil/mcp  -- MCP server (stdio + HTTP)
  docs/           Documentation site (openpencil.dev)
src/              Vue app (components, composables, stores)
desktop/          Tauri v2 (Rust + config)
tests/            E2E (188 tests) + unit (764 tests)
```

## Quality Gates

| Command | Description |
|---------|-------------|
| `bun run check` | Full type check + linting |
| `bun run test` | E2E visual regression tests (Playwright, 188 tests) |
| `bun run test:unit` | Unit tests for the engine (Bun, 764 tests) |
| `bun run format` | Code formatting (oxfmt) |
| `bun run lint` | Linting with type-aware oxlint + tsgolint |
| `bun run test:dupes` | Duplicate code detection (jscpd) |

### Linting

Uses oxlint with TypeScript type awareness:

```sh
bun run lint  # oxlint -c oxlint.json --type-aware --type-check
```

Covers: `src/`, `packages/core/src/`, `packages/cli/src/`, `packages/mcp/src/`

### Type Checking

```sh
bun run check           # Full check
bun run check:vue       # Vue-specific type check (vue-tsc)
```

### Testing Strategy

- **E2E tests** (Playwright): Visual regression testing against known design files, comparing rendered output
- **Unit tests** (Bun): Engine-level tests for scene graph operations, layout, rendering, file format parsing
- **Visual comparison**: `scripts/visual-compare.ts` for screenshot-based diffing

## Desktop Builds

### Prerequisites

Requires [Rust](https://rustup.rs/) and platform-specific prerequisites. See the [Tauri v2 guide](https://v2.tauri.app/start/prerequisites/).

### Building

```sh
bun run tauri build
```

Outputs:

- **macOS**: `.app` bundle + `.dmg` installer
- **Windows**: `.exe` installer + `.msi`
- **Linux**: `.deb`, `.AppImage`

### Size

The desktop app is approximately **~7 MB** (compressed), thanks to Tauri v2 using the system webview instead of bundling Chromium.

## Desktop Configuration

Tauri v2 configuration in `desktop/tauri.conf.json`:

- Window settings (title, size, minimum dimensions)
- Security policy (CSP, asset access)
- Plugin configuration (file system, dialogs, shell, opener)
- Build settings (frontend distribution path)

### Capabilities

Rust capabilities defined in `desktop/capabilities/` control which Tauri APIs are available:

- File system read/write
- Dialog opening
- Shell command execution
- URL opening

## Web App (PWA)

The app can run as a Progressive Web App:

```sh
bun run dev       # Development
bun run build     # Production build
bun run preview   # Preview production build
```

PWA features:

- Service worker via vite-plugin-pwa
- Workbox for caching
- Offline support
- Installable on supported browsers

## Documentation Site

The documentation site at openpencil.dev is built from `packages/docs/`:

```sh
bun run docs:dev      # Development server
bun run docs:build    # Production build
bun run docs:preview  # Preview production build
```

## Contributing

The project welcomes contributions. Key areas:

- **Figma compatibility** -- Improving import/export fidelity
- **Rendering** -- Skia CanvasKit feature coverage
- **Layout** -- Yoga grid layout edge cases
- **Tools** -- New design tools for the AI registry
- **Lint rules** -- Additional quality checks
- **Vue SDK** -- More headless components
- **MCP** -- Tool enhancements and bug fixes

## Roadmap

| Feature | Status | Description |
|---------|--------|-------------|
| 100% Figma compatibility | In progress | Full import/export fidelity, rendering parity, automated compatibility coverage |
| Prototyping | Planned | Frame transitions, interaction triggers, overlay management, preview mode |
| Shader effects (SkSL) | Planned | Custom visual effects via GPU shaders |
| Raster tile caching | Planned | Instant zoom/pan for complex documents |
| Component libraries | Planned | Publish, share, and consume design systems across files |
| CI tools | Planned | Design linting, code export, visual regression in pipelines |
| Grid child positioning UI | Planned | Column/row span controls, grid overlay on canvas |
| Windows code signing | Planned | Azure Authenticode certificates |

## Acknowledgments

Thanks to [@sld0Ant](https://github.com/sld0Ant) (Anton Soldatov) for creating and maintaining the [documentation site](https://openpencil.dev).

## License

MIT

## See Also

- [Overview](00-overview.md) -- What OpenPencil is
- [Architecture](01-architecture.md) -- System design and package structure
