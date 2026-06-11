//! Stable, unique identifiers for Rust types.
//!
//! Provides the [`TypeUuid`] trait which assigns a compile-time UUID to a type.
//! Use `#[derive(TypeUuid)]` from `foundation_macros` to generate implementations.
//!
//! # Example
//!
//! ```rust
//! use foundation_macros::TypeUuid;
//! use foundation_core::type_uuid::{TypeUuid, Bytes};
//!
//! #[derive(TypeUuid)]
//! #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
//! struct MyType;
//!
//! assert_eq!(MyType::UUID.len(), 16);
//! ```

/// A 128-bit (16 byte) buffer containing the type's UUID.
pub type Bytes = [u8; 16];

/// Provides a statically defined UUID for a Rust type.
///
/// Implement via `#[derive(TypeUuid)]` from `foundation_macros` with a
/// `#[uuid = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"]` attribute.
pub trait TypeUuid {
    const UUID: Bytes;
}

/// Allows retrieval of a type's UUID via a trait object.
///
/// Automatically implemented for all types that implement [`TypeUuid`].
pub trait TypeUuidDynamic: private::Sealed {
    fn uuid(&self) -> Bytes;
}

impl<T: TypeUuid> TypeUuidDynamic for T {
    fn uuid(&self) -> Bytes {
        Self::UUID
    }
}

mod private {
    pub trait Sealed {}
    impl<T: super::TypeUuid> Sealed for T {}
}

/// Implement [`TypeUuid`] for an external type.
///
/// # Example
///
/// ```ignore
/// external_type_uuid!(String, "7edbc10a-2147-499c-af9a-498723c7b35f");
/// ```
#[macro_export]
macro_rules! external_type_uuid {
    ($ty:ty, $uuid:expr) => {
        impl $crate::type_uuid::TypeUuid for $ty {
            const UUID: $crate::type_uuid::Bytes = $crate::type_uuid::parse_uuid_const($uuid);
        }
    };
}

/// Parse a UUID string into a `[u8; 16]` at compile time.
///
/// Accepts the standard `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` format.
pub const fn parse_uuid_const(s: &str) -> Bytes {
    let b = s.as_bytes();
    assert!(b.len() == 36, "UUID must be 36 characters");

    let mut out = [0u8; 16];
    let mut i = 0;
    let mut o = 0;
    while i < 36 {
        if b[i] == b'-' {
            i += 1;
            continue;
        }
        out[o] = (hex_val(b[i]) << 4) | hex_val(b[i + 1]);
        i += 2;
        o += 1;
    }
    assert!(o == 16, "UUID must contain exactly 32 hex digits");
    out
}

const fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("Invalid hex character in UUID"),
    }
}

// Primitive types
external_type_uuid!(bool, "abea8c1e-6910-43e4-b579-9ef1b5a95226");
external_type_uuid!(isize, "0d3b0c08-45ff-43f4-a145-b2bdef69d1d2");
external_type_uuid!(i8, "92fd5f7b-2102-46cb-9b1b-662df636625a");
external_type_uuid!(i16, "a02dfda1-8603-4d69-818a-1e1c47b154b6");
external_type_uuid!(i32, "6dd1ba7e-fa8b-4aa1-ac22-c28773798975");
external_type_uuid!(i64, "3103622f-fdfa-4ae3-8ede-67b56bd332fd");
external_type_uuid!(usize, "1d4562ce-b27d-4e99-af44-a40aca248c2e");
external_type_uuid!(u8, "b0fe47a9-fd37-41c6-b2ab-bed5d385ccde");
external_type_uuid!(u16, "3ad2a84b-c5a6-414c-8628-75613e11e67e");
external_type_uuid!(u32, "f6cc80b8-94e8-4c05-80b1-a8fbbaeb67af");
external_type_uuid!(u64, "da9a3e45-516c-4412-87d2-96ea17bebd21");
external_type_uuid!(i128, "0dbb7b33-9f27-4b3f-aebc-11426c464323");
external_type_uuid!(u128, "46eaab86-9268-4e98-ac9f-76eb71a1f0b4");
external_type_uuid!(f32, "5b1d1734-9fcc-43e7-8cc6-452ba16ff1fd");
external_type_uuid!(f64, "76b2ebf4-cd06-41de-96dc-2f402ffa46b2");
external_type_uuid!(char, "9786a9f4-1195-4dd1-875d-3e469454d9c4");
external_type_uuid!(str, "2d07a3d2-d793-44f2-bb28-08c445b164c9");

// Standard library types
#[cfg(feature = "std")]
mod std_impls {
    external_type_uuid!(String, "7edbc10a-2147-499c-af9a-498723c7b35f");
    external_type_uuid!(std::ffi::CStr, "f8ca0716-c80a-4aca-a2f1-bdef739d5688");
    external_type_uuid!(std::ffi::CString, "d26a39da-d0e2-46b1-aeab-481fe57d0f23");
    external_type_uuid!(std::ffi::OsStr, "fb7f1478-03fc-4884-b710-977c8bf9fa8b");
    external_type_uuid!(std::ffi::OsString, "38485fce-f5d0-48df-b5cb-98e510c26a8d");
    external_type_uuid!(std::num::NonZeroU8, "284b98ec-ecb5-463c-9744-23b8669c5553");
    external_type_uuid!(std::num::NonZeroU16, "38f030e4-6046-45c9-96b4-1830b1aa3f35");
    external_type_uuid!(std::num::NonZeroU32, "b32f7cc7-2841-48b3-8d8e-760414b4c4ab");
    external_type_uuid!(std::num::NonZeroU64, "b43c6dad-6608-4f02-817a-8eac8c6345cb");
    external_type_uuid!(std::time::Duration, "449a4224-4665-47ce-88a2-8d0310d20572");
    external_type_uuid!(std::time::SystemTime, "b8dfc518-faf7-4590-91ba-82acd78b1685");
    external_type_uuid!(std::net::IpAddr, "a3c248b7-94e1-4d4a-8b7e-fd1915f4c81b");
    external_type_uuid!(std::net::Ipv4Addr, "a62542a2-6a38-4980-9467-f093bb546140");
    external_type_uuid!(std::net::Ipv6Addr, "a6ba4f16-f436-4ae2-ae62-69dd08150b33");
    external_type_uuid!(std::net::SocketAddr, "fe76891f-3e0a-49f7-b32e-14fc11768844");
    external_type_uuid!(std::net::SocketAddrV4, "e951fa30-50d9-4832-8bc9-c49c06037697");
    external_type_uuid!(std::net::SocketAddrV6, "8840455b-ad6c-41ae-8694-e50873d952c4");
    external_type_uuid!(std::path::Path, "72b02282-6efe-4392-9d9c-467b23ca8c83");
    external_type_uuid!(std::path::PathBuf, "d6db3123-4c95-45de-a28f-5a48d574b9c4");
}

// Unit type
#[allow(dead_code)]
type Unit = ();
external_type_uuid!(Unit, "03748d1a-0d0c-472f-9fdd-424856157064");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uuid_const_works() {
        let bytes = parse_uuid_const("d4adfc76-f5f4-40b0-8e28-8a51a12f5e46");
        assert_eq!(bytes[0], 0xd4);
        assert_eq!(bytes[1], 0xad);
        assert_eq!(bytes[15], 0x46);
    }

    #[test]
    fn standard_type_uuids_exist() {
        assert_ne!(bool::UUID, [0u8; 16]);
        assert_ne!(u32::UUID, [0u8; 16]);
        assert_ne!(String::UUID, [0u8; 16]);
    }

    #[test]
    fn type_uuid_dynamic_trait_object() {
        let val: Box<dyn TypeUuidDynamic> = Box::new(42u32);
        assert_eq!(val.uuid(), u32::UUID);
    }

    #[test]
    fn derive_macro_works() {
        use foundation_macros::TypeUuid;

        #[derive(TypeUuid)]
        #[uuid = "d4adfc76-f5f4-40b0-8e28-8a51a12f5e46"]
        struct TestStruct;

        assert_eq!(
            TestStruct::UUID,
            parse_uuid_const("d4adfc76-f5f4-40b0-8e28-8a51a12f5e46")
        );
    }

    #[test]
    fn derive_macro_on_enum() {
        use foundation_macros::TypeUuid;

        #[derive(TypeUuid)]
        #[uuid = "aabbccdd-1122-3344-5566-778899aabbcc"]
        enum TestEnum {
            A,
            B(u32),
        }

        assert_eq!(
            TestEnum::UUID,
            parse_uuid_const("aabbccdd-1122-3344-5566-778899aabbcc")
        );
    }

    #[test]
    fn external_type_uuid_proc_macro() {
        struct ForeignType;

        foundation_macros::external_type_uuid!(ForeignType, "12345678-abcd-ef01-2345-6789abcdef01");

        assert_eq!(
            ForeignType::UUID,
            parse_uuid_const("12345678-abcd-ef01-2345-6789abcdef01")
        );
    }
}
