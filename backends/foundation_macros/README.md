# foundation_macros

Procedural macros for the ewe_platform foundation crates: the `#[valtron]` /
`#[valtron_test]` test-harness macros, `#[theme!]` / `#[derive(ThemeTokens)]`,
`#[derive(TypeUuid)]`, the `#[action]` attribute, and the compression/mime helpers.

## Declare an action

Use the attribute from `foundation_ui_runtime`. Its generated value implements the runtime's `Action` trait.
The action receives a typed event and writes its output to the context receiver.
See the [tested counter example](../foundation_ui_runtime/README.md#register-and-run-an-action) for declaration and registration.

## Publishing

`foundation_macros` and `foundation_core` must be published together, at the same version.

The `#[valtron]` and `#[valtron_test]` macros expand to code that calls
`foundation_core::valtron` symbols directly (`initialize_pool`,
`initialize_current_thread`, `DEFAULT_TEST_SHUTDOWN_GRACE`,
`__private::block_on_future`, and the `valtron::trace` writers). A change to any
of those symbols breaks the macro output for every consumer, so a
`foundation_macros` release must always ship with the matching `foundation_core`.

release-plz enforces this with a shared `version_group` in `release-plz.toml`. Do
not bump one crate by hand without the other.

### Conditional markup

`html!` evaluates an `Either` condition once per render. False selects `Left`. True selects `Right`.
The selected children replace the directive in the surrounding template.
See the [runtime example](../foundation_ui_runtime/README.md#select-one-branch).

### Declare the type of an action input

`#[action]` accepts `Interaction<S>` for ordinary text. For example, `Interaction<()>` carries the text from a submit event.
Declare a second type to receive structured input. The generated action retains this type for registration and event bindings.

```rust
#[derive(serde::Deserialize)]
struct Row { id: u64 }

#[action(server)]
async fn select(ctx: &Context, event: Interaction<(), Row>) -> ActionResult {
    ctx.store(event.input_value.id);
    Ok(())
}
```

The form author sends the value. A browser or keyboard submit event continues to send text by default.
