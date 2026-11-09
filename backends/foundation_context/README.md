# foundation_context

The shared context for the ewe platform. A `Ctx` holds a typed map, a clock, a config and one
random number generator (RNG).

## Items

- `Clock`: a read-only clock trait. `MonotonicClock` is the real implementation.
- `Config`: the engine configuration. It is a placeholder for now.
- `Ctx`: a cheap handle to the shared state. `clone` shares the map. `fork` copies it.
- `MinimumElement`: names an element that a `Ctx` must carry.

`foundation_core::valtron` re-exports all of these, so existing paths keep working.

## Not in this crate

`SimulatedClock` stays in `foundation_core`. Its `advance` method wakes parked executors
through a crate-private function.

## Features

| Feature | Effect |
| --- | --- |
| `wasm-bindgen` | Picks the `wasm-bindgen` time and entropy backend of `foundation_compact`. |
| `host-abi` | Picks the `host-abi` backend of `foundation_compact`. |

Both features affect only wasm32 builds.

## Publishing

Publish this crate before `foundation_core`. The release tool `release-plz` orders this by
itself.
