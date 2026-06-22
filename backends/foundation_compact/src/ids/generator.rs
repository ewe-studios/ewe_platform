// SCRU128 generator and related items.
// Original: https://github.com/scru128/rust (MIT OR Apache-2.0)

#[cfg(not(feature = "std"))]
use core as std;
use std::{fmt, iter};

use super::{Id, MAX_COUNTER_HI, MAX_COUNTER_LO, MAX_TIMESTAMP};

/// A trait that defines the minimum random number generator interface for [`Generator`].
pub trait RandSource {
    /// Returns the next random `u32`.
    fn next_u32(&mut self) -> u32;
}

/// A trait that defines the minimum system clock interface for [`Generator`].
pub trait TimeSource {
    /// Returns the current Unix timestamp in milliseconds.
    fn unix_ts_ms(&mut self) -> u64;
}

/// Represents a SCRU128 ID generator that encapsulates the counters and other internal state and
/// guarantees the monotonic order of IDs generated within the same millisecond.
///
/// # Generator functions
///
/// The generator comes with four different methods that generate a SCRU128 ID:
///
/// | Flavor                        | Timestamp | On big clock rewind |
/// | ----------------------------- | --------- | ------------------- |
/// | [`generate`]                  | Now       | Resets generator    |
/// | [`generate_or_abort`]         | Now       | Returns `None`      |
/// | [`generate_or_reset_with_ts`] | Argument  | Resets generator    |
/// | [`generate_or_abort_with_ts`] | Argument  | Returns `None`      |
///
/// All of the four return a monotonically increasing ID by reusing the previous `timestamp` even
/// if the one provided is smaller than the immediately preceding ID's. However, when such a clock
/// rollback is considered significant (by default, more than ten seconds):
///
/// 1.  `generate` (`or_reset`) methods reset the generator and return a new ID based on the given
///     `timestamp`, breaking the increasing order of IDs.
/// 2.  `or_abort` variants abort and return `None` immediately.
///
/// [`generate`]: Generator::generate
/// [`generate_or_abort`]: Generator::generate_or_abort
/// [`generate_or_reset_with_ts`]: Generator::generate_or_reset_with_ts
/// [`generate_or_abort_with_ts`]: Generator::generate_or_abort_with_ts
#[derive(Clone)]
pub struct Generator<R, T = StdSystemTime> {
    timestamp: u64,
    counter_hi: u32,
    counter_lo: u32,

    /// The timestamp at the last renewal of `counter_hi` field.
    ts_counter_hi: u64,

    /// The random number generator used by the generator.
    rand_source: R,

    /// The system clock used by the generator.
    time_source: T,

    /// The amount of `timestamp` rollback that is considered significant (in milliseconds).
    rollback_allowance: u64,

    /// Optional machine/node id folded into the HIGH bits of the 32-bit `entropy`
    /// field. `machine_id_bits == 0` (the default) means "no machine id" — the
    /// entropy field is fully random, exactly as upstream scru128.
    ///
    /// WHY: scru128 is 128 bits, fully allocated (timestamp 48 + `counter_hi` 24 +
    /// `counter_lo` 24 + entropy 32). To make ids machine-attributable WITHOUT
    /// growing past 128 bits or disturbing monotonic ordering, we carve the
    /// machine id out of the `entropy` field's high bits — entropy is the least
    /// significant field, so ordering (timestamp → `counter_hi` → `counter_lo` →
    /// entropy) is still established by the timestamp+counter before entropy is
    /// ever compared, and per-id uniqueness is preserved by the remaining random
    /// low bits.
    machine_id: u32,
    /// How many high bits of `entropy` are reserved for `machine_id` (0..=32).
    machine_id_bits: u8,
}

impl<R, T> Generator<R, T> {
    /// Creates a generator object with specified random number generator and system clock.
    pub const fn with_rand_and_time_sources(rand_source: R, time_source: T) -> Self {
        Self {
            timestamp: 0,
            counter_hi: 0,
            counter_lo: 0,
            ts_counter_hi: 0,
            rand_source,
            time_source,
            rollback_allowance: 10_000,
            machine_id: 0,
            machine_id_bits: 0,
        }
    }

    /// Sets the `rollback_allowance` parameter of the generator.
    ///
    /// The `rollback_allowance` parameter specifies the amount of `timestamp` rollback that is
    /// considered significant. The default value is `10_000` (milliseconds).
    ///
    /// # Panics
    ///
    /// Panics if `rollback_allowance` exceeds the 48-bit timestamp range.
    pub fn set_rollback_allowance(&mut self, rollback_allowance: u64) {
        assert!(
            rollback_allowance <= MAX_TIMESTAMP,
            "`rollback_allowance` out of reasonable range"
        );
        self.rollback_allowance = rollback_allowance;
    }

    /// Reserve the top `bits` of the 32-bit `entropy` field for a machine/node id.
    ///
    /// Every generated id then carries `machine_id` (masked to `bits`) in the
    /// high `bits` of its entropy field, with the remaining `32 - bits` filled
    /// randomly per id. This keeps ids monotonic (the machine id sits in the
    /// least-significant field, below timestamp+counter) AND machine-attributable.
    ///
    /// `bits == 0` disables the feature (fully random entropy, upstream behaviour).
    ///
    /// # Panics
    ///
    /// Panics if `bits > 32`.
    pub fn set_machine_id(&mut self, machine_id: u32, bits: u8) {
        assert!(bits <= 32, "`bits` must be in 0..=32 (entropy is 32-bit)");
        self.machine_id_bits = bits;
        // Mask the machine id to the reserved width so callers can't overflow it.
        self.machine_id = if bits == 0 {
            0
        } else if bits == 32 {
            machine_id
        } else {
            machine_id & ((1u32 << bits) - 1)
        };
    }

    /// Builder form of [`set_machine_id`](Self::set_machine_id).
    #[must_use]
    pub fn with_machine_id(mut self, machine_id: u32, bits: u8) -> Self {
        self.set_machine_id(machine_id, bits);
        self
    }

    /// Compose the 32-bit entropy field: `machine_id` in the high `machine_id_bits`,
    /// `random` in the remaining low bits. With `machine_id_bits == 0` this is just
    /// `random` (upstream behaviour).
    fn entropy_with_machine_id(&self, random: u32) -> u32 {
        match self.machine_id_bits {
            0 => random,
            32 => self.machine_id,
            bits => {
                let low_bits = 32 - bits;
                let low_mask = (1u32 << low_bits) - 1;
                (self.machine_id << low_bits) | (random & low_mask)
            }
        }
    }

    /// Resets the internal state of the generator.
    pub(crate) fn reset_state(&mut self) {
        self.timestamp = 0;
        self.counter_hi = 0;
        self.counter_lo = 0;
        self.ts_counter_hi = 0;
    }

    /// Returns a mutable reference to the inner random number source.
    #[cfg(feature = "global_gen")]
    pub(crate) fn rand_source_mut(&mut self) -> &mut R {
        &mut self.rand_source
    }
}

impl<R: RandSource, T: TimeSource> Generator<R, T> {
    /// Generates a new SCRU128 ID object from the current `timestamp`, or resets the generator
    /// upon significant timestamp rollback.
    pub fn generate(&mut self) -> Id {
        let timestamp = self.time_source.unix_ts_ms();
        self.generate_or_reset_with_ts(timestamp)
    }

    /// Generates a new SCRU128 ID object from the current `timestamp`, or returns `None` upon
    /// significant timestamp rollback.
    pub fn generate_or_abort(&mut self) -> Option<Id> {
        let timestamp = self.time_source.unix_ts_ms();
        self.generate_or_abort_with_ts(timestamp)
    }

    /// Returns an infinite iterator that produces a new ID for each call of `next()`.
    pub fn iter(&mut self) -> impl Iterator<Item = Id> + use<'_, R, T> {
        iter::from_fn(|| Some(self.generate()))
    }
}

impl<R: RandSource, T> Generator<R, T> {
    /// Generates a new SCRU128 ID object from the `timestamp` passed, or resets the generator upon
    /// significant timestamp rollback.
    ///
    /// # Panics
    ///
    /// Panics if `timestamp` is not a 48-bit positive integer.
    pub fn generate_or_reset_with_ts(&mut self, timestamp: u64) -> Id {
        if let Some(value) = self.generate_or_abort_with_ts(timestamp) {
            value
        } else {
            self.reset_state();
            self.generate_or_abort_with_ts(timestamp).unwrap()
        }
    }

    /// Generates a new SCRU128 ID object from the `timestamp` passed, or returns `None` upon
    /// significant timestamp rollback.
    ///
    /// # Panics
    ///
    /// Panics if `timestamp` is not a 48-bit positive integer.
    pub fn generate_or_abort_with_ts(&mut self, timestamp: u64) -> Option<Id> {
        assert!(
            timestamp != 0 && timestamp <= MAX_TIMESTAMP,
            "`timestamp` must be a 48-bit positive integer"
        );

        if timestamp > self.timestamp {
            self.timestamp = timestamp;
            self.counter_lo = self.rand_source.next_u32() & MAX_COUNTER_LO;
        } else if timestamp + self.rollback_allowance >= self.timestamp {
            self.counter_lo += 1;
            if self.counter_lo > MAX_COUNTER_LO {
                self.counter_lo = 0;
                self.counter_hi += 1;
                if self.counter_hi > MAX_COUNTER_HI {
                    self.counter_hi = 0;
                    self.timestamp += 1;
                    self.counter_lo = self.rand_source.next_u32() & MAX_COUNTER_LO;
                }
            }
        } else {
            return None;
        }

        if self.timestamp - self.ts_counter_hi >= 1_000 || self.ts_counter_hi == 0 {
            self.ts_counter_hi = self.timestamp;
            self.counter_hi = self.rand_source.next_u32() & MAX_COUNTER_HI;
        }

        let random = self.rand_source.next_u32();
        let entropy = self.entropy_with_machine_id(random);
        Some(
            Id::try_from_fields(self.timestamp, self.counter_hi, self.counter_lo, entropy)
                .unwrap(),
        )
    }

    /// Extract the machine id from an id generated with `machine_id_bits` reserved.
    /// Returns the high `machine_id_bits` of the entropy field. Returns 0 if no
    /// machine id is configured.
    #[must_use]
    pub fn machine_id_of(&self, id: &Id) -> u32 {
        match self.machine_id_bits {
            0 => 0,
            32 => id.entropy(),
            bits => id.entropy() >> (32 - bits),
        }
    }
}

impl<R: Default, T: Default> Default for Generator<R, T> {
    fn default() -> Self {
        Self::with_rand_and_time_sources(R::default(), T::default())
    }
}

impl<R: fmt::Debug, T: fmt::Debug> fmt::Debug for Generator<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Generator")
            .field("rand_source", &self.rand_source)
            .field("time_source", &self.time_source)
            .field("rollback_allowance", &self.rollback_allowance)
            .finish_non_exhaustive()
    }
}

/// The default [`TimeSource`] that uses [`crate::SystemTime`].
///
/// Works on all targets: on native, uses `std::time::SystemTime`; on wasm, uses the
/// `js_sys::Date::now()` polyfill from the time module.
#[derive(Clone, Debug, Default)]
pub struct StdSystemTime;

#[cfg(feature = "std")]
impl TimeSource for StdSystemTime {
    #[allow(clippy::cast_possible_truncation)]
    fn unix_ts_ms(&mut self) -> u64 {
        crate::SystemTime::now()
            .duration_since(crate::UNIX_EPOCH)
            .expect("clock may have gone backwards")
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests;
