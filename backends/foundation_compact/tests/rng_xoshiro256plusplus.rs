use foundation_compact::rng::{Rng, SeedableRng, Xoshiro256PlusPlus};

#[test]
fn reference() {
    let mut rng = Xoshiro256PlusPlus::from_seed([
        1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0,
        0, 0, 0,
    ]);
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
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(0);
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
