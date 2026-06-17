# Feature 08: Headless UI Components

## Description

Create `foundation_ui_components` — a library of unstyled, accessible UI components built on `foundation_wasm_ui`. Inspired by headlessui's compound component pattern: no visual styles, complete accessibility (WAI-ARIA, keyboard navigation), and composable API.

## Crate Structure

```
backends/foundation_ui_components/
├── Cargo.toml
│   [dependencies]
│   foundation_wasm_ui = { workspace = true, features = ["full"] }
│
└── src/
    ├── lib.rs
    ├── button.rs             # Button with keyboard support
    ├── dialog.rs             # Modal with focus trap, escape to close
    ├── menu.rs               # Dropdown menu with arrow key nav
    ├── tabs.rs               # Tab group with automatic activation
    ├── popover.rs            # Popover with outside click close
    ├── disclosure.rs         # Collapsible content
    ├── combobox.rs           # Searchable select with filtering
    ├── listbox.rs            # Custom select dropdown
    ├── switch.rs             # Toggle switch
    ├── checkbox.rs           # Checkbox with label
    ├── input.rs              # Text input with validation
    ├── toast.rs              # Notification toast (auto-dismiss)
    ├── skeleton.rs           # Loading placeholder
    └── transitions/          # Enter/leave animations
        └── mod.rs
```

## Component Examples

### Button

```rust
pub struct Button {
    variant: Signal<ButtonVariant>,  // primary, secondary, ghost, danger
    disabled: Signal<bool>,
    loading: Signal<bool>,
    on_click: Signal<Option<Callback>>,
}
// Keyboard: Enter/Space activates, disabled prevents focus
// ARIA: role="button", aria-disabled, aria-busy (loading)
```

### Dialog (Compound Component Pattern)

```rust
pub struct Dialog {
    open: Signal<bool>,
    on_close: Signal<Option<Callback>>,
    title: Signal<String>,
    description: Signal<String>,
}

// Usage in html! macro:
html! {
    <dialog :open={dialog_open}>
        <dialog:title>{title}</dialog:title>
        <dialog:description>{description}</dialog:description>
        <dialog:panel>
            {children}
        </dialog:panel>
        <dialog:close-button />
    </dialog>
}

// Keyboard: Escape closes, Tab trapped inside panel
// ARIA: role="dialog", aria-modal="true", aria-labelledby, aria-describedby
// Focus: trapped inside, returns to trigger on close
```

### Combobox

```rust
pub struct Combobox<T> {
    query: Signal<String>,
    selected: Signal<Option<T>>,
    options: Signal<Vec<T>>,
    open: Signal<bool>,
    on_select: Signal<Option<Callback<T>>>,
    filter: Signal<Option<Fn(&T, &str) -> bool>>,
}
// Keyboard: Arrow keys navigate, Enter selects, Escape closes
// ARIA: role="combobox", aria-expanded, aria-activedescendant
// Filtering: client-side by default, server-side via callback
```

### Tabs

```rust
pub struct TabGroup {
    selected_index: Signal<usize>,
    on_change: Signal<Option<Callback<usize>>>,
    tabs: Signal<Vec<TabDef>>,
}
// Keyboard: Left/Right arrows switch tabs, Home/End jump to first/last
// ARIA: role="tablist", role="tab", role="tabpanel", aria-selected
// Activation: automatic (arrow key switches) or manual
```

## Accessibility Standards

All components follow WAI-ARIA Authoring Practices:

| Component | ARIA Roles | Keyboard | Focus |
|-----------|-----------|----------|-------|
| Button | `button` | Enter, Space | Tab |
| Dialog | `dialog`, `aria-modal` | Escape, Tab trap | Auto-focus first element |
| Menu | `menu`, `menuitem` | Arrow keys, Escape | Roving tabindex |
| Tabs | `tablist`, `tab`, `tabpanel` | Arrow keys, Home/End | Roving tabindex |
| Popover | N/A | Escape | Returns to trigger |
| Combobox | `combobox`, `listbox`, `option` | Arrow keys, Enter, Escape | Input focused |
| Switch | `switch`, `aria-checked` | Enter, Space | Tab |
| Checkbox | `checkbox`, `aria-checked` | Enter, Space | Tab |

## Compound Component Pattern

Inspired by headlessui's approach — components expose sub-components via namespacing:

```rust
// In Rust:
impl Menu {
    pub fn button() -> MenuButton { ... }
    pub fn items() -> MenuItems { ... }
    pub fn item() -> MenuItem { ... }
}

// In template:
html! {
    <menu>
        <menu:button>Options</menu:button>
        <menu:items>
            <menu:item on:select={on_edit}>Edit</menu:item>
            <menu:item on:select={on_delete}>Delete</menu:item>
        </menu:items>
    </menu>
}
```

## Transition System

```rust
pub struct Transition {
    show: Signal<bool>,
    enter_from: Signal<String>,   // CSS classes for enter start
    enter_to: Signal<String>,     // CSS classes for enter end
    leave_from: Signal<String>,   // CSS classes for leave start
    leave_to: Signal<String>,     // CSS classes for leave end
    duration: Signal<Duration>,
}

// Usage:
html! {
    <transition
        :show={dialog_open}
        enter-from="opacity-0 scale-95"
        enter-to="opacity-100 scale-100"
        leave-from="opacity-100 scale-100"
        leave-to="opacity-0 scale-95"
        duration={300ms}
    >
        <dialog:panel>...</dialog:panel>
    </transition>
}
```

Transitions use CSS animations (no JS animation library). The transition component adds/removes classes and listens for `transitionend` events.

## Dependencies

- `foundation_wasm_ui = { workspace = true }`

## Testing

Each component has:
- Renders correctly in isolation
- Keyboard navigation works (arrow keys, Enter, Escape, Tab)
- ARIA attributes are correct (verified via DOM inspection)
- Focus management (auto-focus, focus trap, focus restoration)
- State management (open/closed, selected/deselected)
- Compound component composition works
- Transitions apply correct CSS classes
