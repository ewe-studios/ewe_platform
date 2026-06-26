//! Tests extracted from shared/ipc/util.rs
mod tests {
    use foundation_nativeapis::shared::ipc::*;
    use foundation_nativeapis::shared::ipc::util::*;

    #[test]
    fn align4_no_padding() {
        assert_eq!(4usize.align4(), 4);
        assert_eq!(8usize.align4(), 8);
        assert_eq!(0usize.align4(), 0);
    }

    #[test]
    fn align4_padded() {
        assert_eq!(1usize.align4(), 4);
        assert_eq!(2usize.align4(), 4);
        assert_eq!(3usize.align4(), 4);
        assert_eq!(5usize.align4(), 8);
        assert_eq!(7usize.align4(), 8);
    }

    #[test]
    fn align4_u32() {
        assert_eq!(1u32.align4(), 4);
        assert_eq!(4u32.align4(), 4);
    }

    #[test]
    fn endpoint_id_unique() {
        let id1 = EndpointID::new();
        let id2 = EndpointID::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn range_to_offset_size_unbounded() {
        let (offset, size) = range_to_offset_size(..);
        assert_eq!(offset, 0);
        assert_eq!(size, None);
    }

    #[test]
    fn range_to_offset_size_exclusive() {
        let (offset, size) = range_to_offset_size(2..8);
        assert_eq!(offset, 2);
        assert_eq!(size, Some(6));
    }

    #[test]
    fn range_to_offset_size_inclusive() {
        let (offset, size) = range_to_offset_size(2..=7);
        assert_eq!(offset, 2);
        assert_eq!(size, Some(6));
    }
}
