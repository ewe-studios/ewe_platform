use foundation_compact::rng::{Rng, SeedableRng, Xoshiro128PlusPlus};

#[test]
fn reference() {
    let mut rng =
        Xoshiro128PlusPlus::from_seed([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
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
    let mut rng = Xoshiro128PlusPlus::seed_from_u64(0);
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
