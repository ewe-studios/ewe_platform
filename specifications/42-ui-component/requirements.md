# Specification 42 — UI Components (foundation_wasm_ui way)

Headless, composable UI components over the spec-39 stack — designed for our
architecture (typed `Html`, signals, DomOps over a wire, server or wasm
producers), learning from base-ui/headless-ui but **not** porting the React
model. See [plan.md](plan.md) for the originating notes.

## Ground rules (from plan.md)

- **Not everything is a signal.** Static content is set once as plain values;
  signals are reserved for what actually changes. Component specs must state
  which parts are static and which are reactive.
- **It's all just functions.** There is NO component trait/class with
  lifecycle methods. A component is `fn(...) -> Html` (pure) or
  `fn(&Context, &SharedInstructionReceiver, ...) -> Html` (reactive,
  self-mounting). Slots make function composition complete (feature 00);
  nothing else is needed.
- **One rendering contract, any target.** The `html!` macro expands to
  target-agnostic code (verified: zero `cfg(target_arch)` in the expansion
  path; the reactive suite runs natively against `MockProtocol`). Pure vs
  reactive is "value only" vs "value + mounted live instance" — never
  "server vs wasm". Both forms must produce the SAME DOM shape (the morph
  contract).

## Feature Index

| Feature | Description | Status |
|---------|-------------|--------|
| [00-slot-composition](features/00-slot-composition/) | `Render` trait, `Slot`, typed slot structs, `<Fragment>` splice, span text slots, `mount_fragment`, `Html::to_markup` — fixes the three confirmed reactive-composition failure modes | **COMPLETE** (2026-06-13) |
| 01-reactive-structure | `<Show>` / `<For>` built-ins: signal-driven conditional + keyed-list mounting (deliberately deferred out of feature 00; required by the F6 pickers and toast lists in feature 05) | PLANNED |
| [03-app-bootstrap](features/03-app-bootstrap/) | `App` — one-line wiring with protocol presets (arrow default) + SERVER presets (FrameSink frames for WS/SSE streaming) + Context handle-Clone | **COMPLETE** (2026-06-13) |
| [04-mount-protocol-negotiation](features/04-mount-protocol-negotiation/) | `protocol` attribute on mounts + WS binary-frame envelope sniffing; documents the existing HTTP content-type / SSE event-name negotiation | SPEC'D |
| [05-headless-components](features/05-headless-components/) | THE CATALOG: full base-ui review (38 components, 7 shared machinery modules) merged with the initial headless-ui draft — adopt data-attribute styling/hidden-input/field-state, reject React-isms; every component marked static-vs-signal | SPEC'D |
| [06-auth-ui-package](features/06-auth-ui-package/) | Auth UI built from the catalog (rauthy-based breakdown; LAST feature per its own TODO) | DRAFT (moved from spec-39) |

## Order of work

00 → 03 → 04 → feature 05's machinery + early families (F1-F4, F7) → 01
(`<Show>`/`<For>`, needed before the F5/F6 pickers and toast lists) → the
remaining families → 06 last (its own TODO says so).
