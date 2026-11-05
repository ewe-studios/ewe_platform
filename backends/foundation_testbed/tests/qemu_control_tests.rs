//! QEMU monitor control tests (moved from `src/qemu_control.rs`).

use std::path::Path;

use foundation_testbed::qemu_control::{Key, QemuController, QemuProtocol, strip_ansi};

#[test]
fn strip_ansi_works() {
    assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
    assert_eq!(strip_ansi("plain"), "plain");
    assert_eq!(strip_ansi("\x1b[K\x1b[Dtext"), "text");
    assert_eq!(strip_ansi("hello\r\n"), "hello");
}

#[test]
fn key_roundtrip() {
    for (key, name) in &[
        (Key::A, "a"),
        (Key::Enter, "enter"),
        (Key::Space, "space"),
        (Key::Meta, "meta"),
        (Key::F1, "f1"),
        (Key::Up, "up"),
        (Key::N0, "0"),
        (Key::Escape, "escape"),
        (Key::Tab, "tab"),
        (Key::Minus, "-"),
        (Key::Period, "."),
    ] {
        assert_eq!(Key::from_name(name), Some(*key), "{name}");
    }
}

#[test]
fn key_from_name_unknown_is_none() {
    assert!(Key::from_name("nope").is_none());
}

#[test]
fn connect_nonexistent_socket() {
    let ctrl = QemuController::new("/tmp/nope");
    assert!(ctrl.connect().is_err());
}

#[test]
fn connect_windows_container_e2e() {
    let ctrl = QemuController::new(QemuController::DEFAULT_MONITOR);
    if !Path::new(QemuController::DEFAULT_MONITOR).exists() {
        return;
    }
    let mut conn = ctrl.connect().unwrap();
    assert_eq!(conn.protocol, QemuProtocol::Hmp);

    // Send a key
    ctrl.send_key(&mut conn, Key::A).unwrap();

    // Take screenshot
    let ppm = ctrl.screendump(&mut conn).unwrap();
    assert!(
        ppm.len() > 1000,
        "screenshot too small: {} bytes",
        ppm.len()
    );
    assert!(&ppm[..2] == b"P6", "not PPM: {:02x?}", &ppm[..4]);
}
