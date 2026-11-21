use alloc::vec::Vec;

use flatbuffers::{EndianScalar, FlatBufferBuilder, Push, PushAlignment, Vector, WIPOffset};

/// A scalar that the builder aligns to eight bytes.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Aligned8<T>(pub T);

impl<T: EndianScalar> Push for Aligned8<T> {
    type Output = T;

    unsafe fn push(&self, dst: &mut [u8], _written_len: usize) {
        // SAFETY: The builder gives this method enough bytes to store one scalar.
        unsafe { flatbuffers::emplace_scalar(dst, self.0) }
    }

    fn alignment() -> PushAlignment {
        PushAlignment::new(8)
    }
}

/// Writes a vector whose first element starts at an eight-byte buffer offset boundary.
pub fn create_aligned_vector<'fbb, T: EndianScalar>(
    builder: &mut FlatBufferBuilder<'fbb>,
    items: &[T],
) -> WIPOffset<Vector<'fbb, T>> {
    let aligned: Vec<_> = items.iter().copied().map(Aligned8).collect();
    builder.create_vector(&aligned)
}
