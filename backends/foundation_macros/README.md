# foundation_macros

Procedural macros for the ewe_platform foundation crates: the `#[valtron]` /
`#[valtron_test]` test-harness macros, `#[theme!]` / `#[derive(ThemeTokens)]`,
`#[derive(TypeUuid)]`, and the compression/mime helpers.

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
