use super::*;
use std::cell;

impl Generator<()> {
    pub(crate) fn for_testing() -> Generator<impl RandSource, impl TimeSource> {
        Generator::with_rand_and_time_sources(new_rand_source(), new_time_source())
    }
}

fn new_rand_source() -> impl RandSource {
    struct MockRandSource;
    impl RandSource for MockRandSource {
        fn next_u32(&mut self) -> u32 {
            use crate::rng::Rng;
            crate::rng::rng().next_u32()
        }
    }
    MockRandSource
}

fn new_time_source() -> impl TimeSource {
    #[cfg(feature = "std")]
    return StdSystemTime;

    #[cfg(not(feature = "std"))]
    {
        struct MockTimeSource(u64);
        impl TimeSource for MockTimeSource {
            fn unix_ts_ms(&mut self) -> u64 {
                self.0 += 8;
                self.0
            }
        }
        MockTimeSource(0x0123_4567_89abu64)
    }
}

#[test]
fn reads_timestamp_from_time_source() {
    struct PeekableTimeSource<'a, T>(&'a cell::Cell<u64>, T);
    impl<T: TimeSource> TimeSource for PeekableTimeSource<'_, T> {
        fn unix_ts_ms(&mut self) -> u64 {
            self.0.set(self.1.unix_ts_ms());
            self.0.get()
        }
    }

    let ts = cell::Cell::default();
    let time_source = PeekableTimeSource(&ts, new_time_source());
    let mut g = Generator::with_rand_and_time_sources(new_rand_source(), time_source);

    assert_eq!(g.generate().timestamp(), ts.get());
    assert_eq!(g.generate().timestamp(), ts.get());
    assert_eq!(g.generate_or_abort().unwrap().timestamp(), ts.get());
    assert_eq!(g.generate_or_abort().unwrap().timestamp(), ts.get());
    assert_eq!(g.generate().timestamp(), ts.get());
    assert_eq!(g.generate_or_abort().unwrap().timestamp(), ts.get());
    assert_eq!(g.generate().timestamp(), ts.get());
    assert_eq!(g.generate_or_abort().unwrap().timestamp(), ts.get());
}

#[test]
fn handle_clock_rollback() {
    const DEFAULT_ROLLBACK_ALLOWANCE: u64 = 10_000;

    struct CellTimeSource<'a>(&'a cell::Cell<u64>);

    impl TimeSource for CellTimeSource<'_> {
        fn unix_ts_ms(&mut self) -> u64 {
            self.0.get()
        }
    }

    for rollback_allowance in [DEFAULT_ROLLBACK_ALLOWANCE, 5_000, 20_000] {
        let ts = cell::Cell::new(0);
        let [mut g0, mut g1, mut g2, mut g3] = [
            Generator::with_rand_and_time_sources(new_rand_source(), CellTimeSource(&ts)),
            Generator::with_rand_and_time_sources(new_rand_source(), CellTimeSource(&ts)),
            Generator::with_rand_and_time_sources(new_rand_source(), CellTimeSource(&ts)),
            Generator::with_rand_and_time_sources(new_rand_source(), CellTimeSource(&ts)),
        ];

        if rollback_allowance != DEFAULT_ROLLBACK_ALLOWANCE {
            g0.set_rollback_allowance(rollback_allowance);
            g1.set_rollback_allowance(rollback_allowance);
            g2.set_rollback_allowance(rollback_allowance);
            g3.set_rollback_allowance(rollback_allowance);
        }

        let methods: [(&mut dyn FnMut() -> Option<Id>, bool); 4] = [
            (&mut || Some(g0.generate()), true),
            (&mut || g1.generate_or_abort(), false),
            (&mut || Some(g2.generate_or_reset_with_ts(ts.get())), true),
            (&mut || g3.generate_or_abort_with_ts(ts.get()), false),
        ];

        for (generate, is_reset) in methods {
            let mut ts_base = new_time_source().unix_ts_ms();

            ts.set(ts_base);
            let mut prev = generate().unwrap();
            assert_eq!(prev.timestamp(), ts_base);

            for _ in 0..50 {
                let curr = generate().unwrap();
                assert!(prev < curr);
                assert!(curr.timestamp() >= ts_base);
                prev = curr;
            }

            for i in 0..50_000u64 {
                ts.set(ts_base - i.min(rollback_allowance - 1));
                let curr = generate().unwrap();
                assert!(prev < curr);
                assert!(curr.timestamp() >= ts_base);
                prev = curr;
            }

            ts_base += rollback_allowance * 4;
            ts.set(ts_base);
            prev = generate().unwrap();
            assert_eq!(prev.timestamp(), ts_base);

            ts.set(ts_base - rollback_allowance);
            let mut curr = generate();
            assert!(prev < curr.unwrap());
            assert!(curr.unwrap().timestamp() >= ts_base);

            if is_reset {
                prev = curr.unwrap();
                ts.set(ts_base - rollback_allowance - 1);
                curr = generate();
                assert!(prev > curr.unwrap());
                assert_eq!(curr.unwrap().timestamp(), ts_base - rollback_allowance - 1);

                prev = curr.unwrap();
                ts.set(ts_base - rollback_allowance - 2);
                curr = generate();
                assert!(prev < curr.unwrap());
                assert!(curr.unwrap().timestamp() >= ts_base - rollback_allowance - 1);
            } else {
                ts.set(ts_base - rollback_allowance - 1);
                curr = generate();
                assert!(curr.is_none());

                ts.set(ts_base - rollback_allowance - 2);
                curr = generate();
                assert!(curr.is_none());
            }
        }
    }
}

// ============================================================================
// Machine-id support (entropy high-bits) — comprehensive coverage.
// ============================================================================

/// A `TimeSource` that returns a fixed millisecond, so every generated id in a
/// test shares the same timestamp — isolating the counter + entropy behaviour.
struct FixedTimeSource(u64);
impl TimeSource for FixedTimeSource {
    fn unix_ts_ms(&mut self) -> u64 {
        self.0
    }
}

fn fixed_gen(ts: u64, machine_id: u32, bits: u8) -> Generator<impl RandSource, FixedTimeSource> {
    Generator::with_rand_and_time_sources(new_rand_source(), FixedTimeSource(ts))
        .with_machine_id(machine_id, bits)
}

#[test]
fn machine_id_zero_bits_is_upstream_behaviour() {
    // With 0 reserved bits the entropy field is fully random (machine id absent).
    let mut g = fixed_gen(0x0123_4567_89ab, 0xABCD, 0);
    let id = g.generate();
    assert_eq!(g.machine_id_of(&id), 0);
    // Different ids still differ in entropy (random), proving nothing was forced.
    let id2 = g.generate();
    assert_ne!(id, id2);
}

#[test]
fn machine_id_is_encoded_in_high_entropy_bits() {
    let machine = 0x0A5u32; // fits in 12 bits
    let mut g = fixed_gen(0x0123_4567_89ab, machine, 12);
    for _ in 0..256 {
        let id = g.generate();
        // High 12 bits of the 32-bit entropy field carry the machine id exactly.
        assert_eq!(g.machine_id_of(&id), machine);
        assert_eq!(id.entropy() >> 20, machine);
    }
}

#[test]
fn machine_id_is_masked_to_reserved_width() {
    // 12 bits reserved but a wider machine id supplied → only the low 12 bits survive.
    let mut g = fixed_gen(0x0123_4567_89ab, 0xFFFF_FABC, 12);
    let id = g.generate();
    assert_eq!(g.machine_id_of(&id), 0xABC & 0x0FFF);
}

#[test]
fn machine_id_preserves_strict_monotonic_order_within_one_millisecond() {
    // All ids share a timestamp (FixedTimeSource); the counter must still make
    // them strictly increasing despite the machine id occupying entropy bits.
    let mut g = fixed_gen(0x0123_4567_89ab, 0x07F, 12);
    let mut prev = g.generate();
    for _ in 0..5_000 {
        let curr = g.generate();
        assert!(prev < curr, "ids must be strictly increasing within a ms");
        assert_eq!(curr.timestamp(), 0x0123_4567_89ab);
        assert_eq!(g.machine_id_of(&curr), 0x07F);
        prev = curr;
    }
}

#[test]
fn different_machine_ids_partition_the_id_space() {
    let ts = 0x0123_4567_89ab;
    let mut a = fixed_gen(ts, 1, 12);
    let mut b = fixed_gen(ts, 2, 12);
    let ida = a.generate();
    let idb = b.generate();
    assert_eq!(a.machine_id_of(&ida), 1);
    assert_eq!(b.machine_id_of(&idb), 2);
    assert_ne!(ida, idb);
}

#[test]
fn full_32_bit_machine_id_consumes_entire_entropy_field() {
    let machine = 0xDEAD_BEEFu32;
    let mut g = fixed_gen(0x0123_4567_89ab, machine, 32);
    let id = g.generate();
    assert_eq!(g.machine_id_of(&id), machine);
    assert_eq!(id.entropy(), machine);
}

#[test]
#[should_panic(expected = "`bits` must be in 0..=32")]
fn machine_id_bits_above_32_panics() {
    let _ = Generator::with_rand_and_time_sources(new_rand_source(), FixedTimeSource(1))
        .with_machine_id(0, 33);
}

#[test]
fn set_machine_id_is_equivalent_to_builder() {
    let mut a = Generator::with_rand_and_time_sources(new_rand_source(), FixedTimeSource(7))
        .with_machine_id(0x123, 12);
    let mut b = Generator::with_rand_and_time_sources(new_rand_source(), FixedTimeSource(7));
    b.set_machine_id(0x123, 12);
    let ida = a.generate();
    let idb = b.generate();
    assert_eq!(a.machine_id_of(&ida), b.machine_id_of(&idb));
    assert_eq!(a.machine_id_of(&ida), 0x123);
}
