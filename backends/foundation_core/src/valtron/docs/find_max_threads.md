# Max threads

Find out the max threads you can create

See [source](https://github.com/rayon-rs/rayon/blob/main/rayon-core/src/sleep/counters.rs#L55-L77)

```rust
/// Number of bits used for the thread counters.
#[cfg(target_pointer_width = "64")]
const THREADS_BITS: usize = 16;

#[cfg(target_pointer_width = "32")]
const THREADS_BITS: usize = 8;

/// Bits to shift to select the sleeping threads
/// (used with `select_bits`).
#[allow(clippy::erasing_op)]
const SLEEPING_SHIFT: usize = 0 * THREADS_BITS;

/// Bits to shift to select the inactive threads
/// (used with `select_bits`).
#[allow(clippy::identity_op)]
const INACTIVE_SHIFT: usize = 1 * THREADS_BITS;

/// Bits to shift to select the JEC
/// (use JOBS_BITS).
const JEC_SHIFT: usize = 2 * THREADS_BITS;

/// Max value for the thread counters.
pub(crate) const THREADS_MAX: usize = (1 << THREADS_BITS) - 1;
```
