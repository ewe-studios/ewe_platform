// Not original code: copied from https://github.com/artichoke/raw-parts (see MIT license)

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::mem::ManuallyDrop;

/// A wrapper around the decomposed parts of a `Vec<T>`.
///
/// This struct contains the `Vec`'s internal pointer, length, and allocated
/// capacity.
///
/// `RawParts` makes [`Vec::from_raw_parts`] and [`Vec::into_raw_parts`] easier
/// to use by giving names to the returned values. This prevents errors from
/// mixing up the two `usize` values of length and capacity.
///
/// # Examples
///
/// ```
/// use foundation_nostd::raw_parts::RawParts;
///
/// let v: Vec<i32> = vec![-1, 0, 1];
///
/// let RawParts { ptr, length, capacity } = RawParts::from_vec(v);
///
/// let rebuilt = unsafe {
///     // We can now make changes to the components, such as
///     // transmuting the raw pointer to a compatible type.
///     let ptr = ptr as *mut u32;
///     let raw_parts = RawParts { ptr, length, capacity };
///
///     raw_parts.into_vec()
/// };
/// assert_eq!(rebuilt, [4294967295, 0, 1]);
/// ```
pub struct RawParts<T> {
    /// A non-null pointer to a buffer of `T`.
    ///
    /// This pointer is the same as the value returned by [`Vec::as_mut_ptr`] in
    /// the source vector.
    pub ptr: *mut T,
    /// The number of elements in the source vector, also referred to as its
    /// "length".
    ///
    /// This value is the same as the value returned by [`Vec::len`] in the
    /// source vector.
    pub length: u64,
    /// The number of elements the source vector can hold without reallocating.
    ///
    /// This value is the same as the value returned by [`Vec::capacity`] in the
    /// source vector.
    pub capacity: u64,
}

impl<T> From<Vec<T>> for RawParts<T> {
    /// Decompose a `Vec<T>` into its raw components.
    fn from(vec: Vec<T>) -> Self {
        Self::from_vec(vec)
    }
}

impl<T> fmt::Debug for RawParts<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RawParts")
            .field("ptr", &self.ptr)
            .field("length", &self.length)
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<T> PartialEq for RawParts<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr && self.length == other.length && self.capacity == other.capacity
    }
}

impl<T> Eq for RawParts<T> {}

impl<T> Hash for RawParts<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
        self.length.hash(state);
        self.capacity.hash(state);
    }
}

// Do not implement the `From` trait in the other direction since `crate::from`
// is an unsafe function.
//
// ```
// impl<T> From<RawParts<T>> for Vec<T> {
//     fn from(raw_parts: RawParts<T>) -> Self {
//         // ERROR: this requires `unsafe`, which we don't want to hide in a
//         // `From` impl.
//         from(raw_parts)
//     }
// }

impl<T> RawParts<T> {
    /// Construct the raw components of a `Vec<T>` by decomposing it.
    ///
    /// Returns a struct containing the raw pointer to the underlying data, the
    /// length of the vector (in elements), and the allocated capacity of the
    /// data (in elements).
    ///
    /// After calling this function, the caller is responsible for the memory
    /// previously managed by the `Vec`. The only way to do this is to convert
    /// the raw pointer, length, and capacity back into a `Vec` with the
    /// [`Vec::from_raw_parts`] function or the [`into_vec`] function, allowing
    /// the destructor to perform the cleanup.
    ///
    /// [`into_vec`]: Self::into_vec
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_nostd::raw_parts::RawParts;
    ///
    /// let v: Vec<i32> = vec![-1, 0, 1];
    ///
    /// let RawParts { ptr, length, capacity } = RawParts::from_vec(v);
    ///
    /// let rebuilt = unsafe {
    ///     // We can now make changes to the components, such as
    ///     // transmuting the raw pointer to a compatible type.
    ///     let ptr = ptr as *mut u32;
    ///     let raw_parts = RawParts { ptr, length, capacity };
    ///
    ///     raw_parts.into_vec()
    /// };
    /// assert_eq!(rebuilt, [4294967295, 0, 1]);
    /// ```
    #[must_use]
    pub fn from_vec(vec: Vec<T>) -> Self {
        // FIXME Update this when vec_into_raw_parts is stabilized
        // See: https://doc.rust-lang.org/1.69.0/src/alloc/vec/mod.rs.html#823-826
        // See: https://doc.rust-lang.org/beta/unstable-book/library-features/vec-into-raw-parts.html
        //
        // https://github.com/rust-lang/rust/issues/65816
        let mut me = ManuallyDrop::new(vec);
        let (ptr, length, capacity) = (me.as_mut_ptr(), me.len(), me.capacity());

        Self {
            ptr,
            length: length as u64,
            capacity: capacity as u64,
        }
    }

    /// Creates a `Vec<T>` directly from the raw components of another vector.
    ///
    /// # Safety
    ///
    /// This function has the same safety invariants as [`Vec::from_raw_parts`],
    /// which are repeated in the following paragraphs.
    ///
    /// This is highly unsafe, due to the number of invariants that aren't
    /// checked:
    ///
    /// * `ptr` must have been allocated using the global allocator, such as via
    ///   the [`alloc::alloc`] function.
    /// * `T` needs to have the same alignment as what `ptr` was allocated with.
    ///   (`T` having a less strict alignment is not sufficient, the alignment really
    ///   needs to be equal to satisfy the [`dealloc`] requirement that memory must be
    ///   allocated and deallocated with the same layout.)
    /// * The size of `T` times the `capacity` (ie. the allocated size in bytes) needs
    ///   to be the same size as the pointer was allocated with. (Because similar to
    ///   alignment, [`dealloc`] must be called with the same layout `size`.)
    /// * `length` needs to be less than or equal to `capacity`.
    /// * The first `length` values must be properly initialized values of type `T`.
    /// * `capacity` needs to be the capacity that the pointer was allocated with.
    /// * The allocated size in bytes must be no larger than `isize::MAX`.
    ///   See the safety documentation of [`pointer::offset`].
    ///
    /// These requirements are always upheld by any `ptr` that has been allocated
    /// via `Vec<T>`. Other allocation sources are allowed if the invariants are
    /// upheld.
    ///
    /// Violating these may cause problems like corrupting the allocator's
    /// internal data structures. For example it is normally **not** safe
    /// to build a `Vec<u8>` from a pointer to a C `char` array with length
    /// `size_t`, doing so is only safe if the array was initially allocated by
    /// a `Vec` or `String`.
    /// It's also not safe to build one from a `Vec<u16>` and its length, because
    /// the allocator cares about the alignment, and these two types have different
    /// alignments. The buffer was allocated with alignment 2 (for `u16`), but after
    /// turning it into a `Vec<u8>` it'll be deallocated with alignment 1. To avoid
    /// these issues, it is often preferable to do casting/transmuting using
    /// [`slice::from_raw_parts`] instead.
    ///
    /// The ownership of `ptr` is effectively transferred to the
    /// `Vec<T>` which may then deallocate, reallocate or change the
    /// contents of memory pointed to by the pointer at will. Ensure
    /// that nothing else uses the pointer after calling this
    /// function.
    ///
    /// [`String`]: alloc::string::String
    /// [`alloc::alloc`]: alloc::alloc::alloc
    /// [`dealloc`]: alloc::alloc::GlobalAlloc::dealloc
    /// [`slice::from_raw_parts`]: core::slice::from_raw_parts
    /// [`pointer::offset`]: https://doc.rust-lang.org/stable/std/primitive.pointer.html#method.offset
    ///
    /// # Examples
    ///
    /// ```
    /// use core::ptr;
    /// use core::mem;
    ///
    /// use foundation_nostd::raw_parts::RawParts;
    ///
    /// let v = vec![1, 2, 3];
    ///
    /// // Pull out the various important pieces of information about `v`
    /// let RawParts { ptr, length, capacity } = RawParts::from_vec(v);
    ///
    /// unsafe {
    ///     // Overwrite memory with 4, 5, 6
    ///     for i in 0..length as isize {
    ///         ptr::write(ptr.offset(i), 4 + i);
    ///     }
    ///
    ///     // Put everything back together into a Vec
    ///     let raw_parts = RawParts { ptr, length, capacity };
    ///     let rebuilt = raw_parts.into_vec();
    ///     assert_eq!(rebuilt, [4, 5, 6]);
    /// }
    /// ```
    #[must_use]
    pub unsafe fn into_vec(self) -> Vec<T> {
        let Self {
            ptr,
            length,
            capacity,
        } = self;

        // Safety:
        //
        // The safety invariants that callers must uphold when calling `from` match
        // the safety invariants of `Vec::from_raw_parts`.
        unsafe { Vec::from_raw_parts(ptr, length as usize, capacity as usize) }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
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
}
