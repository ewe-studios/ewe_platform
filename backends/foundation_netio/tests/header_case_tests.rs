//! A header name is case-insensitive (issue #965).
//!
//! A JavaScript `Headers` object gives lowercase names; a forwarded identity
//! header is written with a constant that may use another case. Both must name
//! one header.

use foundation_netio::shared::http::SimpleHeader;

#[test]
fn a_custom_header_has_one_canonical_name() {
    let from_lowercase = SimpleHeader::from("ewe-forwarded-principal".to_string());
    let from_uppercase = SimpleHeader::from("EWE-FORWARDED-PRINCIPAL".to_string());
    let written = SimpleHeader::custom("EWE-Forwarded-Principal");
    assert_eq!(from_lowercase, from_uppercase);
    assert_eq!(from_lowercase, written);
}

#[test]
fn a_named_header_keeps_its_variant() {
    assert_eq!(
        SimpleHeader::from("Content-Type".to_string()),
        SimpleHeader::CONTENT_TYPE
    );
    assert_eq!(
        SimpleHeader::from("authorization".to_string()),
        SimpleHeader::AUTHORIZATION
    );
}
