//! `ids::id` tests (moved from `src/ids/id.rs`).

use foundation_compact::ids::{Generator, Id, RandSource, StdSystemTime, TimeSource};

/// A generator with a real RNG and the test time source.
fn for_testing() -> Generator<impl RandSource, impl TimeSource> {
    struct MockRandSource;
    impl RandSource for MockRandSource {
        fn next_u32(&mut self) -> u32 {
            use foundation_compact::rng::Rng;
            foundation_compact::rng::rng().next_u32()
        }
    }
    fn time_source() -> impl TimeSource {
        #[cfg(feature = "std")]
        {
            StdSystemTime
        }
        #[cfg(not(feature = "std"))]
        {
            struct MockTimeSource(u64);
            impl TimeSource for MockTimeSource {
                fn unix_ts_ms(&mut self) -> u64 {
                    self.0 += 8;
                    self.0
                }
            }
            MockTimeSource(0x0123_4567_89ab)
        }
    }
    Generator::with_rand_and_time_sources(MockRandSource, time_source())
}

const MAX_UINT48: u64 = (1 << 48) - 1;
const MAX_UINT24: u32 = (1 << 24) - 1;
const MAX_UINT32: u32 = u32::MAX;

#[test]
fn encodes_and_decodes_prepared_cases_correctly() {
    #[allow(clippy::type_complexity)]
    let cases: &[((u64, u32, u32, u32), &str)] = &[
        ((0, 0, 0, 0), "0000000000000000000000000"),
        ((MAX_UINT48, 0, 0, 0), "F5LXX1ZZ5K6TP71GEEH2DB7K0"),
        ((MAX_UINT48, 0, 0, 0), "f5lxx1zz5k6tp71geeh2db7k0"),
        ((0, MAX_UINT24, 0, 0), "0000000005GV2R2KJWR7N8XS0"),
        ((0, MAX_UINT24, 0, 0), "0000000005gv2r2kjwr7n8xs0"),
        ((0, 0, MAX_UINT24, 0), "00000000000000JPIA7QL4HS0"),
        ((0, 0, MAX_UINT24, 0), "00000000000000jpia7ql4hs0"),
        ((0, 0, 0, MAX_UINT32), "0000000000000000001Z141Z3"),
        ((0, 0, 0, MAX_UINT32), "0000000000000000001z141z3"),
        (
            (MAX_UINT48, MAX_UINT24, MAX_UINT24, MAX_UINT32),
            "F5LXX1ZZ5PNORYNQGLHZMSP33",
        ),
        (
            (MAX_UINT48, MAX_UINT24, MAX_UINT24, MAX_UINT32),
            "f5lxx1zz5pnorynqglhzmsp33",
        ),
    ];

    for e in cases {
        let from_fields = Id::try_from_fields(e.0 .0, e.0 .1, e.0 .2, e.0 .3).unwrap();
        let from_string = e.1.parse::<Id>().unwrap();

        assert_eq!(from_fields, from_string);
        assert_eq!(
            from_fields.to_u128(),
            u128::from_str_radix(e.1, 36).unwrap()
        );
        assert_eq!(
            from_string.to_u128(),
            u128::from_str_radix(e.1, 36).unwrap()
        );
        assert_eq!(
            from_fields.to_bytes(),
            u128::from_str_radix(e.1, 36).unwrap().to_be_bytes()
        );
        assert_eq!(
            from_string.to_bytes(),
            u128::from_str_radix(e.1, 36).unwrap().to_be_bytes()
        );
        assert_eq!(
            (
                (
                    from_fields.timestamp(),
                    from_fields.counter_hi(),
                    from_fields.counter_lo(),
                    from_fields.entropy(),
                ),
                &from_fields.encode() as &str
            ),
            (e.0, e.1.to_lowercase().as_str())
        );
        assert_eq!(
            (
                (
                    from_string.timestamp(),
                    from_string.counter_hi(),
                    from_string.counter_lo(),
                    from_string.entropy(),
                ),
                &from_string.encode() as &str
            ),
            (e.0, e.1.to_lowercase().as_str())
        );
        #[cfg(feature = "std")]
        assert_eq!(from_fields.to_string(), e.1.to_lowercase());
        #[cfg(feature = "std")]
        assert_eq!(from_string.to_string(), e.1.to_lowercase());
    }
}

#[test]
fn returns_error_if_an_invalid_string_representation_is_supplied() {
    use foundation_compact::ids::ParseErrorKind::{self, *};
    fn invalid_digit(c: char, position: usize) -> ParseErrorKind {
        let mut utf8_char = [0u8; 4];
        c.encode_utf8(&mut utf8_char);
        InvalidDigit {
            utf8_char,
            position,
        }
    }

    let cases = [
        ("", InvalidLength { n_bytes: 0 }),
        (" 036z8puq4tsxsigk6o19y164q", InvalidLength { n_bytes: 26 }),
        ("036z8puq54qny1vq3hcbrkweb ", InvalidLength { n_bytes: 26 }),
        (" 036z8puq54qny1vq3helivwax ", InvalidLength { n_bytes: 27 }),
        ("+036z8puq54qny1vq3hfcv3ss0", InvalidLength { n_bytes: 26 }),
        ("-036z8puq54qny1vq3hhy8u1ch", InvalidLength { n_bytes: 26 }),
        ("+36z8puq54qny1vq3hjq48d9p", invalid_digit('+', 0)),
        ("-36z8puq5a7j0ti08oz6zdrdy", invalid_digit('-', 0)),
        ("036z8puq5a7j0t_08p2cdz28v", invalid_digit('_', 14)),
        ("036z8pu-5a7j0ti08p3ol8ool", invalid_digit('-', 7)),
        ("036z8puq5a7j0ti08p4j 6cya", invalid_digit(' ', 20)),
        ("f5lxx1zz5pnorynqglhzmsp34", OutOfU128Range),
        ("zzzzzzzzzzzzzzzzzzzzzzzzz", OutOfU128Range),
        ("039o\tvvklfmqlqe7fzllz7c7t", invalid_digit('\t', 4)),
        ("039onvvklfmqlq漢字fgvd1", invalid_digit('漢', 14)),
        ("039onvvkl🤣qe7fzr2hdoqu", invalid_digit('🤣', 9)),
        ("頭onvvklfmqlqe7fzrhtgcfz", invalid_digit('頭', 0)),
        ("039onvvklfmqlqe7fztft5尾", invalid_digit('尾', 22)),
        ("039漢字a52xp4bvf4sn94e09cja", InvalidLength { n_bytes: 29 }),
        ("039ooa52xp4bv😘sn97642mwl", InvalidLength { n_bytes: 27 }),
    ];

    for e in cases {
        let result = e.0.parse::<Id>();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, e.1);
    }
}

#[test]
fn has_symmetric_converters_from_to_various_values() {
    let cases = [
        Id::try_from_fields(0, 0, 0, 0).unwrap(),
        Id::try_from_fields(MAX_UINT48, 0, 0, 0).unwrap(),
        Id::try_from_fields(0, MAX_UINT24, 0, 0).unwrap(),
        Id::try_from_fields(0, 0, MAX_UINT24, 0).unwrap(),
        Id::try_from_fields(0, 0, 0, MAX_UINT32).unwrap(),
        Id::try_from_fields(MAX_UINT48, MAX_UINT24, MAX_UINT24, MAX_UINT32).unwrap(),
    ];

    let cases = {
        let mut v = cases.to_vec();
        let mut g = for_testing();
        for _ in 0..1000 {
            v.push(g.generate());
        }
        v
    };

    for e in cases {
        assert_eq!(Id::try_from_str(&e.encode()).unwrap(), e);
        assert_eq!(e.encode().parse::<Id>().unwrap(), e);
        #[cfg(feature = "std")]
        assert_eq!(e.to_string().parse::<Id>().unwrap(), e);
        #[cfg(feature = "std")]
        assert_eq!(Id::try_from(String::from(e)).unwrap(), e);
        assert_eq!(Id::from_u128(e.to_u128()), e);
        assert_eq!(Id::from(u128::from(e)), e);
        assert_eq!(Id::from_bytes(e.to_bytes()), e);
        assert_eq!(Id::from(<[u8; 16]>::from(e)), e);
        assert_eq!(Id::from_bytes(*e.as_bytes()), e);
        assert_eq!(
            Id::try_from_fields(e.timestamp(), e.counter_hi(), e.counter_lo(), e.entropy())
                .unwrap(),
            e
        );
    }
}

#[test]
fn supports_comparison_operators() {
    #[cfg(feature = "std")]
    let hash = {
        use std::hash::BuildHasher as _;
        let s = std::collections::hash_map::RandomState::new();
        move |value: &Id| s.hash_one(value)
    };

    let ordered = [
        Id::try_from_fields(0, 0, 0, 0).unwrap(),
        Id::try_from_fields(0, 0, 0, 1).unwrap(),
        Id::try_from_fields(0, 0, 0, MAX_UINT32).unwrap(),
        Id::try_from_fields(0, 0, 1, 0).unwrap(),
        Id::try_from_fields(0, 0, MAX_UINT24, 0).unwrap(),
        Id::try_from_fields(0, 1, 0, 0).unwrap(),
        Id::try_from_fields(0, MAX_UINT24, 0, 0).unwrap(),
        Id::try_from_fields(1, 0, 0, 0).unwrap(),
        Id::try_from_fields(2, 0, 0, 0).unwrap(),
    ];

    let ordered = {
        let mut v = ordered.to_vec();
        let mut g = for_testing();
        for _ in 0..1000 {
            v.push(g.generate());
        }
        v
    };

    let mut prev = &ordered[0];
    for curr in &ordered[1..] {
        assert_ne!(curr, prev);
        assert_ne!(prev, curr);
        #[cfg(feature = "std")]
        assert_ne!(hash(curr), hash(prev));
        assert!(curr > prev);
        assert!(curr >= prev);
        assert!(prev < curr);
        assert!(prev <= curr);

        let clone = &curr.clone();
        assert_eq!(curr, clone);
        assert_eq!(clone, curr);
        #[cfg(feature = "std")]
        assert_eq!(hash(curr), hash(clone));
        assert!(curr >= clone);
        assert!(clone >= curr);
        assert!(curr <= clone);
        assert!(clone <= curr);

        prev = curr;
    }
}

#[cfg(feature = "serde")]
mod serde_tests {
    use foundation_compact::ids::Id;
    use serde_test::{Configure, Token};

    #[test]
    fn serializes_and_deserializes_prepared_cases_correctly() {
        let cases = [
            (
                "037arkzbgn93kdu9h3pw2ow2l",
                &[
                    1, 128, 178, 254, 34, 56, 72, 100, 6, 87, 159, 252, 102, 145, 202, 93,
                ],
            ),
            (
                "037arkzbh94jvgjmm6jtwgztq",
                &[
                    1, 128, 178, 254, 34, 60, 72, 100, 6, 194, 191, 219, 2, 6, 125, 94,
                ],
            ),
            (
                "037arkzbheley7unpvcjf5k4z",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 140, 185, 18, 16, 51,
                ],
            ),
            (
                "037arkzbheley7unpvel8zyp1",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 141, 195, 39, 182, 101,
                ],
            ),
            (
                "037arkzbheley7unpvgefdinq",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 142, 174, 14, 198, 182,
                ],
            ),
            (
                "037arkzbheley7unpvhsywho2",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 143, 100, 55, 67, 114,
                ],
            ),
            (
                "037arkzbheley7unpvjlr4ot7",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 144, 77, 179, 181, 155,
                ],
            ),
            (
                "037arkzbheley7unpvmfm8457",
                &[
                    1, 128, 178, 254, 34, 61, 72, 100, 6, 48, 162, 145, 188, 211, 88, 251,
                ],
            ),
        ];

        for (text, bytes) in cases {
            let e = text.parse::<Id>().unwrap();
            serde_test::assert_tokens(&e.readable(), &[Token::Str(text)]);
            serde_test::assert_tokens(&e.compact(), &[Token::Bytes(bytes)]);

            serde_test::assert_de_tokens(&e.readable(), &[Token::Bytes(bytes)]);
            serde_test::assert_de_tokens(&e.compact(), &[Token::Str(text)]);

            serde_test::assert_de_tokens(&e.readable(), &[Token::Bytes(text.as_bytes())]);
            serde_test::assert_de_tokens(&e.compact(), &[Token::Bytes(text.as_bytes())]);
        }
    }
}
