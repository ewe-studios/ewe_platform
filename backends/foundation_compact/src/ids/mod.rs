// SCRU128 monotonic IDs — folded from foundation_rng.
// Original: https://github.com/scru128/rust (MIT OR Apache-2.0)

#[cfg(not(feature = "std"))]
use core as std;

mod rand_adapter;
#[cfg(feature = "global_gen")]
mod global_gen;

pub mod generator;
pub mod id;

pub use generator::{Generator, RandSource, StdSystemTime, TimeSource};
pub use id::{FieldError, Id, ParseError};
pub use rand_adapter::Adapter;

#[cfg(feature = "global_gen")]
pub use global_gen::{new, new_string};

/// The maximum value of 48-bit `timestamp` field.
const MAX_TIMESTAMP: u64 = 0xffff_ffff_ffff;

/// The maximum value of 24-bit `counter_hi` field.
const MAX_COUNTER_HI: u32 = 0xff_ffff;

/// The maximum value of 24-bit `counter_lo` field.
const MAX_COUNTER_LO: u32 = 0xff_ffff;

/// Generate a new SCRU128 ID using a thread-local generator.
///
/// Uses our vendored entropy + ChaCha12 RNG + cross-platform SystemTime.
/// Works on all targets including `wasm32-unknown-unknown`.
#[cfg(feature = "std")]
pub fn new_scru128() -> Id {
    use std::cell::RefCell;

    type DefaultGen = Generator<Adapter<crate::rng::ThreadRng>, StdSystemTime>;
    thread_local! {
        static GEN: RefCell<DefaultGen> = RefCell::new(
            Generator::with_rand_and_time_sources(
                Adapter(crate::rng::rng()),
                StdSystemTime,
            )
        );
    }
    GEN.with_borrow_mut(|g| g.generate())
}

/// Generate a new SCRU128 ID as a 25-digit canonical string.
///
/// Works on all targets including `wasm32-unknown-unknown`.
#[cfg(feature = "std")]
pub fn new_scru128_string() -> String {
    new_scru128().to_string()
}

/// Number of high `entropy` bits reserved for the machine id by
/// [`new_scru128_with_machine_id`]. 12 bits → up to 4096 distinguishable
/// machines, leaving 20 bits of per-id randomness in the entropy field.
pub const DEFAULT_MACHINE_ID_BITS: u8 = 12;

/// Generate a monotonic SCRU128 ID that carries a `machine_id` in the high
/// [`DEFAULT_MACHINE_ID_BITS`] of its entropy field.
///
/// WHY: lets callers (e.g. session ids) be machine-attributable while keeping
/// the strict monotonic ordering the thread-local generator guarantees — the
/// machine id lives in the least-significant field, so ordering is unaffected.
///
/// The generator is thread-local and keyed by `machine_id`, so repeated calls
/// with the same `machine_id` share one monotonic counter (ordering preserved);
/// the machine id is masked to [`DEFAULT_MACHINE_ID_BITS`].
#[cfg(feature = "std")]
pub fn new_scru128_with_machine_id(machine_id: u32) -> Id {
    use std::cell::RefCell;

    type DefaultGen = Generator<Adapter<crate::rng::ThreadRng>, StdSystemTime>;
    thread_local! {
        // Keyed cache: (machine_id, generator). The common case is a single
        // stable machine id per process, so a one-entry cache is enough and
        // keeps the monotonic counter shared across calls.
        static GEN: RefCell<Option<(u32, DefaultGen)>> = const { RefCell::new(None) };
    }
    GEN.with_borrow_mut(|slot| {
        let needs_new = !matches!(slot, Some((mid, _)) if *mid == machine_id);
        if needs_new {
            let gen = Generator::with_rand_and_time_sources(
                Adapter(crate::rng::rng()),
                StdSystemTime,
            )
            .with_machine_id(machine_id, DEFAULT_MACHINE_ID_BITS);
            *slot = Some((machine_id, gen));
        }
        // Safe: set just above when needed.
        slot.as_mut().map(|(_, g)| g.generate()).unwrap()
    })
}

/// Current unix time in milliseconds via the cross-platform `SystemTime`.
#[cfg(feature = "std")]
fn now_unix_ms() -> u64 {
    crate::SystemTime::now()
        .duration_since(crate::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A stable 64-bit hash (FNV-1a) — small, dependency-free, stable across runs.
/// Used to derive a reproducible id from a name.
#[must_use]
pub fn stable_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Derive a **deterministic, sortable** SCRU128 id from a human-friendly name.
///
/// Unlike [`new_scru128`] this is NOT drawn from the monotonic counter — it is
/// *derived*, so the same `name` + same `machine_id` within one millisecond
/// reproduces the same id (handy as a resume/lookup key). Cross-millisecond
/// ordering is preserved by the 48-bit timestamp; the `machine_id` occupies the
/// high [`DEFAULT_MACHINE_ID_BITS`] of the entropy field (mirroring
/// [`new_scru128_with_machine_id`]'s layout), and a stable hash of `name` fills
/// `counter_hi | counter_lo` plus the low entropy bits.
///
/// Pass `machine_id = 0` for a machine-agnostic derived id.
#[cfg(feature = "std")]
#[must_use]
pub fn scru128_from_name_with_machine_id(name: &str, machine_id: u32) -> Id {
    let ts = now_unix_ms();
    let bits = u32::from(DEFAULT_MACHINE_ID_BITS);
    let mid = if bits == 0 { 0 } else { machine_id & ((1u32 << bits) - 1) };
    let h = stable_hash(name.as_bytes());

    let counter_hi = ((h >> 40) as u32) & MAX_COUNTER_HI;
    let counter_lo = ((h >> 16) as u32) & MAX_COUNTER_LO;
    // entropy: machine id in the high `bits`, hash in the low `32 - bits`.
    let entropy = if bits == 0 {
        h as u32
    } else if bits == 32 {
        mid
    } else {
        let low = 32 - bits;
        (mid << low) | ((h as u32) & ((1u32 << low) - 1))
    };

    Id::try_from_fields(ts, counter_hi, counter_lo, entropy)
        .expect("hash/machine fields are masked into range")
}

/// Derive a deterministic SCRU128 id from a name with no machine id
/// (`machine_id = 0`). See [`scru128_from_name_with_machine_id`].
#[cfg(feature = "std")]
#[must_use]
pub fn scru128_from_name(name: &str) -> Id {
    scru128_from_name_with_machine_id(name, 0)
}

/// Extract the machine id from the high [`DEFAULT_MACHINE_ID_BITS`] of an id's
/// 32-bit entropy field — the inverse of the machine-id encoding used by
/// [`new_scru128_with_machine_id`] and [`scru128_from_name_with_machine_id`].
#[must_use]
pub fn machine_id_of(id: &Id) -> u32 {
    let bits = u32::from(DEFAULT_MACHINE_ID_BITS);
    match bits {
        0 => 0,
        32 => id.entropy(),
        _ => id.entropy() >> (32 - bits),
    }
}

#[cfg(all(test, feature = "std"))]
mod machine_id_api_tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic_and_distinguishing() {
        assert_eq!(stable_hash(b"fix-bug"), stable_hash(b"fix-bug"));
        assert_ne!(stable_hash(b"fix-bug"), stable_hash(b"add-feature"));
        // Known FNV-1a property: empty input hashes to the offset basis.
        assert_eq!(stable_hash(b""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn new_with_machine_id_is_monotonic_and_attributable() {
        let machine = 0x3FF;
        let ids: Vec<Id> = (0..2_000).map(|_| new_scru128_with_machine_id(machine)).collect();
        for w in ids.windows(2) {
            // Strictly increasing even across same-millisecond bursts.
            assert!(w[0] < w[1], "ids must be strictly increasing");
        }
        for id in &ids {
            assert_eq!(machine_id_of(id), machine);
        }
    }

    #[test]
    fn new_with_machine_id_masks_to_default_bits() {
        // A machine id wider than DEFAULT_MACHINE_ID_BITS is masked down.
        let bits = u32::from(DEFAULT_MACHINE_ID_BITS);
        let wide = 0xFFFF_FFFFu32;
        let id = new_scru128_with_machine_id(wide);
        let expected = if bits == 32 { wide } else { wide & ((1u32 << bits) - 1) };
        assert_eq!(machine_id_of(&id), expected);
    }

    #[test]
    fn from_name_is_reproducible_within_a_millisecond() {
        // Same name + machine in the same ms ⇒ identical id (resume key).
        // Retry to dodge a millisecond boundary between the two calls.
        let mut matched = false;
        for _ in 0..8 {
            let a = scru128_from_name_with_machine_id("resume-me", 7);
            let b = scru128_from_name_with_machine_id("resume-me", 7);
            if a.timestamp() == b.timestamp() {
                assert_eq!(a, b);
                matched = true;
                break;
            }
        }
        assert!(matched, "two calls should land in the same millisecond at least once");
    }

    #[test]
    fn from_name_distinguishes_names_and_machines() {
        // Different names differ in the hashed counter/entropy region.
        let a = scru128_from_name_with_machine_id("alpha", 1);
        let b = scru128_from_name_with_machine_id("beta", 1);
        assert_ne!(a.counter_hi() ^ a.counter_lo(), b.counter_hi() ^ b.counter_lo());
        // Same name on different machines carries different machine ids.
        let m1 = scru128_from_name_with_machine_id("same", 1);
        let m2 = scru128_from_name_with_machine_id("same", 2);
        assert_eq!(machine_id_of(&m1), 1);
        assert_eq!(machine_id_of(&m2), 2);
    }

    #[test]
    fn from_name_embeds_machine_id_like_the_generator() {
        let id = scru128_from_name_with_machine_id("x", 42);
        assert_eq!(machine_id_of(&id), 42);
        // The no-machine-id variant carries 0.
        let plain = scru128_from_name("x");
        assert_eq!(machine_id_of(&plain), 0);
    }

    #[test]
    fn from_name_is_time_ordered_across_milliseconds() {
        let a = scru128_from_name("label");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = scru128_from_name("label");
        // Same derived name, later call ⇒ later timestamp ⇒ sorts after.
        assert!(b.timestamp() >= a.timestamp());
        if b.timestamp() > a.timestamp() {
            assert!(b > a);
        }
    }
}
