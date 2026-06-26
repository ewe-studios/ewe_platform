extern crate alloc;

use foundation_nostd::raw_parts::*;
    use alloc::vec::Vec;

    #[test]
    fn roundtrip() {
        let mut vec = Vec::with_capacity(100); // capacity is 100
        vec.extend_from_slice(b"123456789"); // length is 9

        let raw_parts = RawParts::from_vec(vec);
        let raw_ptr = raw_parts.ptr;

        let mut roundtripped_vec = unsafe { raw_parts.into_vec() };

        assert_eq!(roundtripped_vec.capacity(), 100);
        assert_eq!(roundtripped_vec.len(), 9);
        assert_eq!(roundtripped_vec.as_mut_ptr(), raw_ptr);
    }
