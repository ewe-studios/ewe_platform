//! Check version four identifiers with caller-supplied entropy.
use foundation_compact::uuid::UuidV4;

#[test]
fn injected_entropy_sets_the_version_and_variant() {
    let id = UuidV4::from_entropy(|bytes| {
        bytes.fill(0xff);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(id.to_string(), "ffffffff-ffff-4fff-bfff-ffffffffffff");
    assert_eq!(id.to_string().parse(), Ok(id));
    assert!("ffffffff-ffff-1fff-bfff-ffffffffffff"
        .parse::<UuidV4>()
        .is_err());
    assert!("ffffffff-ffff-4fff-ffff-ffffffffffff"
        .parse::<UuidV4>()
        .is_err());
    assert_eq!(
        UuidV4::from_entropy(|_| Err("unavailable")),
        Err("unavailable")
    );
}

#[cfg(feature = "serde")]
#[test]
fn wire_format_uses_text() {
    let value = "ffffffff-ffff-4fff-bfff-ffffffffffff";
    serde_test::assert_tokens(
        &value.parse::<UuidV4>().unwrap(),
        &[serde_test::Token::Str(value)],
    );
}
