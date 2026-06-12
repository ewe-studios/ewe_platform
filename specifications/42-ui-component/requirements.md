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
| [00-slot-composition](features/00-slot-composition/) | `Render` trait, `Slot`, typed slot structs, `<Fragment>` splice, span text slots, `mount_fragment`, `Html::to_markup` — fixes the three confirmed reactive-composition failure modes | SPEC'D |
| [01-headless-ui-components](features/01-headless-ui-components/) | The component catalog (regenerate from base-ui review per plan.md, on top of feature 00) | DRAFT (moved from spec-39) |
| [02-auth-ui-package](features/02-auth-ui-package/) | Auth UI built from the catalog | DRAFT (moved from spec-39) |
| [03-app-bootstrap](features/03-app-bootstrap/) | `App` — one-line wiring of signals/context/runtime/receiver with protocol presets; `app.context() -> (Context, SharedInstructionReceiver)` | SPEC'D |
| [04-mount-protocol-negotiation](features/04-mount-protocol-negotiation/) | `protocol` attribute on mounts + WS binary-frame envelope sniffing; documents the existing HTTP content-type / SSE event-name negotiation | SPEC'D |
| 05-reactive-structure | `<Show>` / `<For>` built-ins: signal-driven conditional + keyed-list mounting (deliberately deferred out of feature 00) | PLANNED |

## Order of work

00 → 03 → 04, then the base-ui survey regenerates 01 (component-by-component
features), 02 builds on the catalog, 05 lands when the catalog demonstrates
the concrete need (lists, comboboxes).
