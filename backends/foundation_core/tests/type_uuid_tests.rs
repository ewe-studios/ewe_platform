use foundation_core::type_uuid::{parse_uuid_const, TypeUuid, TypeUuidDynamic};

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

    // Fixture: variants exist only to prove the derive handles unit and
    // tuple variants — the test reads the generated UUID, not the values.
    #[allow(dead_code)]
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
