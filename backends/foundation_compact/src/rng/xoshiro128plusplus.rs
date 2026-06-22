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

/// A xoshiro128++ random number generator.
///
/// The xoshiro128++ algorithm is not suitable for cryptographic purposes, but
/// is very fast and has excellent statistical properties.
///
/// The algorithm used here is translated from [the `xoshiro128plusplus.c`
/// reference source code](http://xoshiro.di.unimi.it/xoshiro128plusplus.c) by
/// David Blackman and Sebastiano Vigna.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Xoshiro128PlusPlus {
    s: [u32; 4],
}

impl SeedableRng for Xoshiro128PlusPlus {
    type Seed = [u8; 16];

    /// Create a new `Xoshiro128PlusPlus`.  If `seed` is entirely 0, it will be
    /// mapped to a different seed.
    #[inline]
    fn from_seed(seed: [u8; 16]) -> Xoshiro128PlusPlus {
        let state = utils::read_words(&seed);
        // Check for zero on aligned integers for better code generation.
        // Furtermore, seed_from_u64(0) will expand to a constant when optimized.
        if state.iter().all(|&x| x == 0) {
            return Self::seed_from_u64(0);
        }
        Xoshiro128PlusPlus { s: state }
    }

    /// Create a new `Xoshiro128PlusPlus` from a `u64` seed.
    ///
    /// This uses the `SplitMix64` generator internally.
    #[inline]
    fn seed_from_u64(mut state: u64) -> Self {
        const PHI: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut s = [0; 4];
        for i in s.chunks_exact_mut(2) {
            state = state.wrapping_add(PHI);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z = z ^ (z >> 31);
            #[allow(clippy::cast_possible_truncation)]
            {
                i[0] = z as u32;
            }
            i[1] = (z >> 32) as u32;
        }
        // By using a non-zero PHI we are guaranteed to generate a non-zero state
        // Thus preventing a recursion between from_seed and seed_from_u64.
        debug_assert_ne!(s, [0; 4]);
        Xoshiro128PlusPlus { s }
    }
}

impl TryRng for Xoshiro128PlusPlus {
    type Error = Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        let res = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(7)
            .wrapping_add(self.s[0]);

        let t = self.s[1] << 9;

        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];

        self.s[2] ^= t;

        self.s[3] = self.s[3].rotate_left(11);

        Ok(res)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        utils::next_u64_via_u32(self)
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        utils::fill_bytes_via_next_word(dst, || self.try_next_u32())
    }
}

#[cfg(test)]
mod tests {
    use super::Xoshiro128PlusPlus;
    use rand_core::{Rng, SeedableRng};

    #[test]
    fn reference() {
        let mut rng =
            Xoshiro128PlusPlus::from_seed([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
        // These values were produced with the reference implementation:
        // http://xoshiro.di.unimi.it/xoshiro128plusplus.c
        let expected = [
            641,
            1_573_767,
            3_222_811_527,
            3_517_856_514,
            836_907_274,
            4_247_214_768,
            3_867_114_732,
            1_355_841_295,
            495_546_011,
            621_204_420,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u32(), e);
        }
    }

    #[test]
    fn stable_seed_from_u64_and_from_seed() {
        // We don't guarantee value-stability for SmallRng but this
        // could influence keeping stability whenever possible (e.g. after optimizations).
        let mut rng = Xoshiro128PlusPlus::seed_from_u64(0);
        // from_seed([0; 16]) should produce the same state as seed_from_u64(0).
        let mut rng_from_seed_0 = Xoshiro128PlusPlus::from_seed([0; 16]);
        let expected = [
            1_179_900_579,
            1_938_959_192,
            3_089_844_957,
            3_657_088_315,
            1_015_453_891,
            479_942_911,
            3_433_842_246,
            669_252_886,
            3_985_671_746,
            2_737_205_563,
        ];
        for &e in &expected {
            assert_eq!(rng.next_u32(), e);
            assert_eq!(rng_from_seed_0.next_u32(), e);
        }
    }
}
