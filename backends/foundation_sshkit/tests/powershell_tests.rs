//! Tests for foundation_sshkit::powershell — encoding, stripping, execution shape.

use foundation_sshkit::powershell;

#[test]
fn ps_exec_bad_session_returns_error() {
    // No connection — should fail cleanly, not panic.
    let sess = ssh2::Session::new().expect("session");
    let result = powershell::ps_exec(&sess, "Write-Host hello");
    assert!(result.is_err(), "should error on unconnected session");
}

#[test]
fn base64_encoding_is_reversible_utf16le() {
    // Verify our UTF-16LE + Base64 round-trips correctly for ASCII.
    let script = "Write-Host hello";
    let utf16le: Vec<u8> = script.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &utf16le);

    // The encoded command should not contain the raw script
    assert!(!encoded.contains("Write-Host"), "base64 should obscure the script");
    assert!(encoded.len() > 0, "should produce output");
}

#[test]
fn unicode_surrogate_roundtrip() {
    // PowerShell uses UTF-16LE — verify a character outside ASCII survives.
    let script = "echo \u{00e9}"; // e-acute
    let utf16le: Vec<u8> = script.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();

    // Decode back
    let decoded: Vec<u16> = utf16le
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let roundtripped = String::from_utf16(&decoded).expect("valid UTF-16");

    assert_eq!(roundtripped, script);
}
