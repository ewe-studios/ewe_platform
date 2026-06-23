//! WHY: Verify the `value_quantitization` public API correctly quantizes
//! pointer, integer, and unsigned integer values.
//!
//! WHAT: `value_quantitization`, `TypeOptimization`.
//!
//! HOW: Feed known values through quantization functions and assert the
//! output bytes and optimization type match expectations.

use foundation_wasm::{value_quantitization, TypeOptimization};

#[test]
fn can_quantize_ptr() {
    struct TestCase {
        value: *const u8,
        expected_bytes: Vec<u8>,
        quantization: TypeOptimization,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            value: 20 as *const u8,
            expected_bytes: vec![20],
            quantization: TypeOptimization::QuantizedPtrAsU8,
        },
        TestCase {
            value: 32767 as *const u8,
            expected_bytes: vec![255, 127],
            quantization: TypeOptimization::QuantizedPtrAsU16,
        },
        TestCase {
            value: 2147483647 as *const u8,
            expected_bytes: vec![255, 255, 255, 127],
            quantization: TypeOptimization::QuantizedPtrAsU32,
        },
        TestCase {
            value: 6294967296 as *const u8,
            expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
            quantization: TypeOptimization::None,
        },
        TestCase {
            value: 9223372036854775809 as *const u8,
            expected_bytes: vec![1, 0, 0, 0, 0, 0, 0, 128],
            quantization: TypeOptimization::None,
        },
    ];

    for test_case in test_cases {
        let (content, tq) = value_quantitization::qpointer(test_case.value);
        assert_eq!(
            test_case.expected_bytes, content,
            "Output bytes should match"
        );
        assert_eq!(test_case.quantization, tq, "Quantization type should match");
    }
}

#[test]
fn can_quantize_i128() {
    struct TestCase {
        value: i128,
        expected_bytes: Vec<u8>,
        quantization: TypeOptimization,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            value: 20,
            expected_bytes: vec![20],
            quantization: TypeOptimization::QuantizedInt128AsI8,
        },
        TestCase {
            value: 32767,
            expected_bytes: vec![255, 127],
            quantization: TypeOptimization::QuantizedInt128AsI16,
        },
        TestCase {
            value: 2147483647,
            expected_bytes: vec![255, 255, 255, 127],
            quantization: TypeOptimization::QuantizedInt128AsI32,
        },
        TestCase {
            value: 6294967296,
            expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
            quantization: TypeOptimization::QuantizedInt128AsI64,
        },
        TestCase {
            value: 9223372036854775809,
            expected_bytes: vec![0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 128],
            quantization: TypeOptimization::None,
        },
    ];

    for test_case in test_cases {
        let (content, tq) = value_quantitization::qi128(test_case.value);
        assert_eq!(
            test_case.expected_bytes, content,
            "Output bytes should match"
        );
        assert_eq!(test_case.quantization, tq, "Quantization type should match");
    }
}

#[test]
fn can_quantize_u128() {
    struct TestCase {
        value: u128,
        expected_bytes: Vec<u8>,
        quantization: TypeOptimization,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            value: 20,
            expected_bytes: vec![20],
            quantization: TypeOptimization::QuantizedUint128AsU8,
        },
        TestCase {
            value: 65535,
            expected_bytes: vec![255, 255],
            quantization: TypeOptimization::QuantizedUint128AsU16,
        },
        TestCase {
            value: 4294967295,
            expected_bytes: vec![255, 255, 255, 255],
            quantization: TypeOptimization::QuantizedUint128AsU32,
        },
        TestCase {
            value: 6294967296,
            expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
            quantization: TypeOptimization::QuantizedUint128AsU64,
        },
        TestCase {
            value: 18446744073709551619,
            expected_bytes: vec![1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0],
            quantization: TypeOptimization::None,
        },
    ];

    for test_case in test_cases {
        let (content, tq) = value_quantitization::qu128(test_case.value);
        assert_eq!(
            test_case.expected_bytes, content,
            "Output bytes should match"
        );
        assert_eq!(test_case.quantization, tq, "Quantization type should match");
    }
}

#[test]
fn can_quantize_u64() {
    struct TestCase {
        value: u64,
        expected_bytes: Vec<u8>,
        quantization: TypeOptimization,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            value: 20,
            expected_bytes: vec![20],
            quantization: TypeOptimization::QuantizedUint64AsU8,
        },
        TestCase {
            value: 65535,
            expected_bytes: vec![255, 255],
            quantization: TypeOptimization::QuantizedUint64AsU16,
        },
        TestCase {
            value: 4294967295,
            expected_bytes: vec![255, 255, 255, 255],
            quantization: TypeOptimization::QuantizedUint64AsU32,
        },
        TestCase {
            value: 6294967296,
            expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
            quantization: TypeOptimization::None,
        },
        TestCase {
            value: 4294967296,
            expected_bytes: vec![0, 0, 0, 0, 1, 0, 0, 0],
            quantization: TypeOptimization::None,
        },
    ];

    for test_case in test_cases {
        let (content, tq) = value_quantitization::qu64(test_case.value);
        assert_eq!(
            test_case.expected_bytes, content,
            "Output bytes should match"
        );
        assert_eq!(test_case.quantization, tq, "Quantization type should match");
    }
}

#[test]
fn can_quantize_i64() {
    struct TestCase {
        value: i64,
        expected_bytes: Vec<u8>,
        quantization: TypeOptimization,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            value: 20,
            expected_bytes: vec![20],
            quantization: TypeOptimization::QuantizedInt64AsI8,
        },
        TestCase {
            value: 32767,
            expected_bytes: vec![255, 127],
            quantization: TypeOptimization::QuantizedInt64AsI16,
        },
        TestCase {
            value: 2147483647,
            expected_bytes: vec![255, 255, 255, 127],
            quantization: TypeOptimization::QuantizedInt64AsI32,
        },
        TestCase {
            value: 6294967296,
            expected_bytes: vec![0, 148, 53, 119, 1, 0, 0, 0],
            quantization: TypeOptimization::None,
        },
    ];

    for test_case in test_cases {
        let (content, tq) = value_quantitization::qi64(test_case.value);
        assert_eq!(
            test_case.expected_bytes, content,
            "Output bytes should match"
        );
        assert_eq!(test_case.quantization, tq, "Quantization type should match");
    }
}
