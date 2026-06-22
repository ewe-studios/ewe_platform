// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::convert::Infallible;
use rand_core::{SeedableRng, TryRng, utils};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A xoshiro256++ random number generator.
///
/// The xoshiro256++ algorithm is not suitable for cryptographic purposes, but
/// is very fast and has excellent statistical properties.
///
/// The algorithm used here is translated from [the `xoshiro256plusplus.c`
/// reference source code](http://xoshiro.di.unimi.it/xoshiro256plusplus.c) by
/// David Blackman and Sebastiano Vigna.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Xoshiro256PlusPlus {
    s: [u64; 4],
}

impl SeedableRng for Xoshiro256PlusPlus {
    type Seed = [u8; 32];

    /// Create a new `Xoshiro256PlusPlus`.  If `seed` is entirely 0, it will be
    /// mapped to a different seed.
    #[inline]
    fn from_seed(seed: [u8; 32]) -> Xoshiro256PlusPlus {
        let state = utils::read_words(&seed);
        // Check for zero on aligned integers for better code generation.
        // Furtermore, seed_from_u64(0) will expand to a constant when optimized.
        if state.iter().all(|&x| x == 0) {
            return Self::seed_from_u64(0);
        }
        Xoshiro256PlusPlus { s: state }
    }

    /// Create a new `Xoshiro256PlusPlus` from a `u64` seed.
    ///
    /// This uses the `SplitMix64` generator internally.
    #[inline]
    fn seed_from_u64(mut state: u64) -> Self {
        const PHI: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut s = [0; 4];
        for i in &mut s {
            state = state.wrapping_add(PHI);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z = z ^ (z >> 31);
            *i = z;
        }
        // By using a non-zero PHI we are guaranteed to generate a non-zero state
        // Thus preventing a recursion between from_seed and seed_from_u64.
        debug_assert_ne!(s, [0; 4]);
        Xoshiro256PlusPlus { s }
    }
}

impl TryRng for Xoshiro256PlusPlus {
    type Error = Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        // The lowest bits have some linear dependencies, so we use the
        // upper bits instead.
        self.try_next_u64().map(|val| (val >> 32) as u32)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        let res = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);

        let t = self.s[1] << 17;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;

        self.s[3] = self.s[3].rotate_left(45);

        Ok(res)
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        utils::fill_bytes_via_next_word(dst, || self.try_next_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::Xoshiro256PlusPlus;
    use rand_core::{Rng, SeedableRng};

    #[test]
    fn reference() {
        let mut rng = Xoshiro256PlusPlus::from_seed([
            1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0,
            0, 0, 0,
        ]);
        // These values were produced with the reference implementation:
        // http://xoshiro.di.unimi.it/xoshiro256plusplus.c
        let expected = [
            41_943_041,
            58_720_359,
            3_588_806_011_781_223,
            3_591_011_842_654_386,
            9_228_616_714_210_784_205,
            9_973_669_472_204_895_162,
            14_011_001_112_246_962_877,
            12_406_186_145_184_390_807,
            15_849_039_046_786_891_736,
            10_450_023_813_501_588_000,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u64(), e);
        }
    }

    #[test]
    fn stable_seed_from_u64_and_from_seed() {
        // We don't guarantee value-stability for SmallRng but this
        // could influence keeping stability whenever possible (e.g. after optimizations).
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(0);
        // from_seed([0; 32]) should produce the same state as seed_from_u64(0).
        let mut rng_from_seed_0 = Xoshiro256PlusPlus::from_seed([0; 32]);
        let expected = [
            5_987_356_902_031_041_503,
            7_051_070_477_665_621_255,
            6_633_766_593_972_829_180,
            211_316_841_551_650_330,
            9_136_120_204_379_184_874,
            379_361_710_973_160_858,
            15_813_423_377_499_357_806,
            15_596_884_590_815_070_553,
            5_439_680_534_584_881_407,
            1_369_371_744_833_522_710,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u64(), e);
            assert_eq!(rng_from_seed_0.next_u64(), e);
        }
    }
}
