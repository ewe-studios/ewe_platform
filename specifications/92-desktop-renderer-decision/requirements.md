---
description: "Select the Foundation UI desktop renderer."
status: deferred
created: 2026-11-27
has_features: true
has_fundamentals: false
related_specs: ["69-foundation-ui-v2", "52-tauri-foundation-platform", "91-windows-resource-staging"]
---

# Specification 92. Select the Foundation UI desktop renderer

## Glossary

| Term | Meaning |
| --- | --- |
| User interface (UI) | The controls a person sees and uses. |
| Graphics processing unit (GPU) | The processor that draws the desktop scene. |
| GPUI | The name of the Rust UI framework from Zed. |
| Renderer | The part that turns Foundation UI frame operations into visible desktop controls. |
| Mount | The surface that applies a frame. Native uses toolkit controls. Web uses page controls inside a browser. |
| Adapter | The code that translates between the Foundation UI model and the chosen renderer. |
| Capability | An operation or control that the desktop host can apply and can declare during negotiation. |

## Goal and scope

Use Tauri with the existing browser renderer for first product desktop delivery on the Web mount.
Reuse application actions and browser frame application. See [Specification 52](../completed/52-tauri-foundation-platform/requirements.md) for the shell.

Use two standalone toolkit examples to compare GPUI Fast and GPUI Kit. Review those results before considering a Native mount prototype.
For that prototype, the baseline uses the Zed GPUI snapshot with GPUI Kit Base. GPUI Fast is an alternate engine.
Require equivalent release checks before adopting either engine.

The owner needs a desktop host that opens a window, applies Foundation UI frames, and sends events to the same Rust actions. The owner needs a support decision and cost comparison before the host takes a dependency on an external renderer. When a candidate fails a required platform check, the owner needs the failing case and the cost of the available fallback.

The [desktop backend evaluation](features/01-backend-evaluation/feature.md) owns the source review, comparison, recommendation and acceptance gates. Specification 69 owns the wire format, runtime, WebView control and mobile hosts. Specification 52 owns the existing Tauri application shell. Specification 91 owns its Windows resource verification. The Native mount implementation plan starts after Specification 69 closes and the owner approves the updated plan.

## Architecture

```mermaid
flowchart LR
    Application[Foundation UI application] --> Runtime[Foundation UI runtime and wire]
    Runtime --> Host[Desktop host]
    Host --> Adapter[Desktop renderer adapter]
    Adapter --> Platform[Desktop window and controls]
    Platform -->|Input| Adapter
    Host -->|Event and action dispatch| Runtime
```

The desktop host links the shared runtime to the selected platform renderer.

| Feature | Owns | Status |
| --- | --- | --- |
| [01. Evaluate the desktop backend](features/01-backend-evaluation/feature.md) | Source review, toolkit examples and a later Native mount prototype | Source review complete. Toolkit example plan awaits approval. Native mount prototype deferred. |
| [02. Run Todo in a Tauri desktop window](features/02-tauri-todo-check/feature.md) | First Web mount desktop check | macOS check complete. Other platform checks open. |

## Acceptance

- The owner can identify the revision, dependency cost, control coverage and platform risks for each candidate.
- The standalone toolkit examples require explicit approval of their plan and reference.
- The Native mount prototype requires Specification 69 closure and explicit approval of its plan.
- The same Foundation UI example exercises the shared contract on each desktop platform before production adoption.
