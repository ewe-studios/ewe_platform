//! Tests extracted from vms/winrm/mod.rs

use foundation_testbed::vms::winrm::{
    generate_message_id, parse_command_id, parse_receive_response, parse_shell_id, WinRM,
};

#[test]
fn test_parse_shell_id() {
    let response = r#"<SelectorSet><w:Selector Name="ShellId">ABC123-DEF456</w:Selector></SelectorSet>"#;
    assert_eq!(parse_shell_id(response).as_deref(), Some("ABC123-DEF456"));
}

#[test]
fn test_parse_command_id() {
    let response = r#"<rsp:CommandResponse><rsp:CommandId>cmd-789</rsp:CommandId></rsp:CommandResponse>"#;
    assert_eq!(parse_command_id(response).as_deref(), Some("cmd-789"));
}

#[test]
fn test_generate_message_id_starts_with_uuid() {
    let id = generate_message_id();
    assert!(id.starts_with("uuid:"));
    assert!(id.len() > 30);
}

#[test]
fn test_auth_header_value() {
    let client = WinRM::new("127.0.0.1", 5985, "vagrant", "vagrant");
    let auth = client.auth_header_value();
    // "vagrant:vagrant" base64 = "dmFncmFudDp2YWdyYW50"
    assert_eq!(auth, "dmFncmFudDp2YWdyYW50");
}

#[test]
fn test_parse_receive_response_empty() {
    let response = "<rsp:ReceiveResponse></rsp:ReceiveResponse>";
    let (stdout, stderr, done, code) = parse_receive_response(response);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert!(!done);
    assert_eq!(code, 0);
}
