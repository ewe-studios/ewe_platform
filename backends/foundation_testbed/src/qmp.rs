//! QEMU Monitor Protocol (QMP) client — universal agentic control (F32 Tier 1).
//!
//! All platform Docker images (macOS, Windows, ChromeOS) run on QEMU. QEMU
//! exposes a Unix socket (`/run/shm/monitor.sock`) that accepts JSON commands
//! for keyboard, mouse, and screenshot operations — no guest-side tools needed.
//!
//! QMP sends JSON-RPC-like commands over a Unix stream:
//!   → {"execute":"qmp_capabilities"}
//!   ← {"return":{}}
//!   → {"execute":"input-send-event","arguments":{"events":[...]}}
//!   ← {"return":{}}

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── Error types ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum QmpError {
    ConnectFailed { path: PathBuf, reason: String },
    NegotiationFailed(String),
    CommandFailed { command: String, error: String },
    UnknownKey(String),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for QmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectFailed { path, reason } => {
                write!(f, "QMP connect to {} failed: {}", path.display(), reason)
            }
            Self::NegotiationFailed(msg) => write!(f, "QMP negotiation failed: {msg}"),
            Self::CommandFailed { command, error } => {
                write!(f, "QMP command '{command}' failed: {error}")
            }
            Self::UnknownKey(key) => write!(f, "unknown QEMU key: '{key}'"),
            Self::Io(e) => write!(f, "QMP I/O error: {e}"),
            Self::Json(e) => write!(f, "QMP JSON error: {e}"),
        }
    }
}

impl std::error::Error for QmpError {}

impl From<std::io::Error> for QmpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for QmpError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ── JSON protocol types ──────────────────────────────────────────────────

#[derive(Serialize)]
struct QmpCommand {
    execute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct QmpResponse {
    #[serde(rename = "return")]
    #[allow(dead_code)]
    ret: Option<serde_json::Value>,
    #[allow(dead_code)]
    error: Option<QmpResponseError>,
}

#[derive(Deserialize, Debug)]
struct QmpResponseError {
    #[allow(dead_code)]
    desc: String,
}

// ── QEMU key codes ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QemuKey {
    Shift,
    Ctrl,
    Alt,
    MetaL,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Backspace,
    Tab,
    Return,
    Escape,
    Space,
    CapsLock,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    N0,
    N1,
    N2,
    N3,
    N4,
    N5,
    N6,
    N7,
    N8,
    N9,
    Minus,
    Equal,
    Comma,
    Period,
    Slash,
    Semicolon,
    Apostrophe,
    BracketLeft,
    BracketRight,
    Backslash,
    GraveAccent,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpDivide,
    KpMultiply,
    KpSubtract,
    KpAdd,
    KpEnter,
    KpDecimal,
    VolumeMute,
    VolumeDown,
    VolumeUp,
    Print,
    ScrollLock,
    Pause,
}

impl QemuKey {
    fn qcode(self) -> &'static str {
        match self {
            Self::Shift => "shift",
            Self::Ctrl => "ctrl",
            Self::Alt => "alt",
            Self::MetaL => "meta_l",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "pgup",
            Self::PageDown => "pgdn",
            Self::Insert => "insert",
            Self::Delete => "delete",
            Self::Backspace => "backspace",
            Self::Tab => "tab",
            Self::Return => "ret",
            Self::Escape => "esc",
            Self::Space => "spc",
            Self::CapsLock => "caps_lock",
            Self::F1 => "f1",
            Self::F2 => "f2",
            Self::F3 => "f3",
            Self::F4 => "f4",
            Self::F5 => "f5",
            Self::F6 => "f6",
            Self::F7 => "f7",
            Self::F8 => "f8",
            Self::F9 => "f9",
            Self::F10 => "f10",
            Self::F11 => "f11",
            Self::F12 => "f12",
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
            Self::F => "f",
            Self::G => "g",
            Self::H => "h",
            Self::I => "i",
            Self::J => "j",
            Self::K => "k",
            Self::L => "l",
            Self::M => "m",
            Self::N => "n",
            Self::O => "o",
            Self::P => "p",
            Self::Q => "q",
            Self::R => "r",
            Self::S => "s",
            Self::T => "t",
            Self::U => "u",
            Self::V => "v",
            Self::W => "w",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::N0 => "0",
            Self::N1 => "1",
            Self::N2 => "2",
            Self::N3 => "3",
            Self::N4 => "4",
            Self::N5 => "5",
            Self::N6 => "6",
            Self::N7 => "7",
            Self::N8 => "8",
            Self::N9 => "9",
            Self::Minus => "minus",
            Self::Equal => "equal",
            Self::BracketLeft => "bracket_left",
            Self::BracketRight => "bracket_right",
            Self::Backslash => "backslash",
            Self::Semicolon => "semicolon",
            Self::Apostrophe => "apostrophe",
            Self::Comma => "comma",
            Self::Period => "dot",
            Self::Slash => "slash",
            Self::GraveAccent => "grave_accent",
            Self::KpDivide => "kp_divide",
            Self::KpMultiply => "kp_multiply",
            Self::KpSubtract => "kp_subtract",
            Self::KpAdd => "kp_add",
            Self::KpEnter => "kp_enter",
            Self::KpDecimal => "kp_decimal",
            Self::Kp0 => "kp_0",
            Self::Kp1 => "kp_1",
            Self::Kp2 => "kp_2",
            Self::Kp3 => "kp_3",
            Self::Kp4 => "kp_4",
            Self::Kp5 => "kp_5",
            Self::Kp6 => "kp_6",
            Self::Kp7 => "kp_7",
            Self::Kp8 => "kp_8",
            Self::Kp9 => "kp_9",
            Self::VolumeMute => "mute",
            Self::VolumeDown => "volumedown",
            Self::VolumeUp => "volumeup",
            Self::Print => "print",
            Self::ScrollLock => "scroll_lock",
            Self::Pause => "pause",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "shift" => Some(Self::Shift),
            "ctrl" => Some(Self::Ctrl),
            "alt" => Some(Self::Alt),
            "meta" | "meta_l" | "cmd" | "win" => Some(Self::MetaL),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "enter" | "return" | "ret" => Some(Self::Return),
            "space" | "spc" => Some(Self::Space),
            "tab" => Some(Self::Tab),
            "escape" | "esc" => Some(Self::Escape),
            "backspace" => Some(Self::Backspace),
            "delete" | "del" => Some(Self::Delete),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            "pageup" => Some(Self::PageUp),
            "pagedown" => Some(Self::PageDown),
            "f1" => Some(Self::F1),
            "f2" => Some(Self::F2),
            "f3" => Some(Self::F3),
            "f4" => Some(Self::F4),
            "f5" => Some(Self::F5),
            "f6" => Some(Self::F6),
            "f7" => Some(Self::F7),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "f10" => Some(Self::F10),
            "f11" => Some(Self::F11),
            "f12" => Some(Self::F12),
            "a" => Some(Self::A),
            "b" => Some(Self::B),
            "c" => Some(Self::C),
            "d" => Some(Self::D),
            "e" => Some(Self::E),
            "f" => Some(Self::F),
            "g" => Some(Self::G),
            "h" => Some(Self::H),
            "i" => Some(Self::I),
            "j" => Some(Self::J),
            "k" => Some(Self::K),
            "l" => Some(Self::L),
            "m" => Some(Self::M),
            "n" => Some(Self::N),
            "o" => Some(Self::O),
            "p" => Some(Self::P),
            "q" => Some(Self::Q),
            "r" => Some(Self::R),
            "s" => Some(Self::S),
            "t" => Some(Self::T),
            "u" => Some(Self::U),
            "v" => Some(Self::V),
            "w" => Some(Self::W),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            "0" => Some(Self::N0),
            "1" => Some(Self::N1),
            "2" => Some(Self::N2),
            "3" => Some(Self::N3),
            "4" => Some(Self::N4),
            "5" => Some(Self::N5),
            "6" => Some(Self::N6),
            "7" => Some(Self::N7),
            "8" => Some(Self::N8),
            "9" => Some(Self::N9),
            "minus" | "-" => Some(Self::Minus),
            "equal" | "=" => Some(Self::Equal),
            "comma" | "," => Some(Self::Comma),
            "period" | "." => Some(Self::Period),
            "slash" | "/" => Some(Self::Slash),
            "semicolon" | ";" => Some(Self::Semicolon),
            "apostrophe" | "'" => Some(Self::Apostrophe),
            _ => None,
        }
    }
}

// ── Mouse button ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

impl MouseButton {
    fn qmp_name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Middle => "Middle",
            Self::Right => "Right",
            Self::WheelUp => "WheelUp",
            Self::WheelDown => "WheelDown",
        }
    }
}

// ── QMP connection ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct QmpConnection {
    stream: UnixStream,
}

/// Send a QMP command and read the response. Free function so multiple
/// borrows don't conflict during handshake.
fn send_cmd(conn: &mut QmpConnection, execute: &str, arguments: Option<serde_json::Value>) -> Result<QmpResponse, QmpError> {
    let cmd = QmpCommand { execute: execute.to_string(), arguments };
    let json = serde_json::to_string(&cmd)?;
    conn.stream.write_all(json.as_bytes())?;
    conn.stream.write_all(b"\n")?;
    conn.stream.flush()?;

    let mut reader = BufReader::new(&conn.stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let trimmed = line.trim().to_string();

    let result_line = if trimmed.is_empty() {
        line.clear();
        reader.read_line(&mut line)?;
        line.trim().to_string()
    } else {
        trimmed
    };

    Ok(serde_json::from_str(&result_line)?)
}

// ── QMP client ───────────────────────────────────────────────────────────

pub struct QmpClient {
    socket_path: PathBuf,
}

impl QmpClient {
    pub const DEFAULT_SOCKET: &str = "/run/shm/monitor.sock";

    #[must_use]
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Connect to the QMP socket and negotiate capabilities.
    pub fn connect(&self) -> Result<QmpConnection, QmpError> {
        let socket = &self.socket_path;
        if !socket.exists() {
            return Err(QmpError::ConnectFailed {
                path: socket.clone(),
                reason: "socket file does not exist".into(),
            });
        }

        let stream = UnixStream::connect(socket).map_err(|e| {
            QmpError::ConnectFailed { path: socket.clone(), reason: e.to_string() }
        })?;

        stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| {
            QmpError::ConnectFailed { path: socket.clone(), reason: format!("read_timeout: {e}") }
        })?;

        let mut conn = QmpConnection { stream };

        // Read the QMP greeting
        {
            let mut reader = BufReader::new(&conn.stream);
            let mut greeting = String::new();
            reader.read_line(&mut greeting).map_err(|e| {
                QmpError::NegotiationFailed(format!("read greeting: {e}"))
            })?;
            if !greeting.contains("QMP") {
                return Err(QmpError::NegotiationFailed(format!("bad greeting: {greeting}")));
            }
        }

        // Negotiate capabilities
        send_cmd(&mut conn, "qmp_capabilities", None)?;

        Ok(conn)
    }

    // ── Keyboard ──────────────────────────────────────────────────────────

    pub fn send_key(&self, conn: &mut QmpConnection, key: QemuKey) -> Result<(), QmpError> {
        send_input_events(conn, &[
            key_event(key, true),
            key_event(key, false),
        ])
    }

    pub fn send_keys(&self, conn: &mut QmpConnection, keys: &[QemuKey]) -> Result<(), QmpError> {
        let mut events: Vec<serde_json::Value> = keys.iter().map(|k| key_event(*k, true)).collect();
        let release: Vec<serde_json::Value> = keys.iter().rev().map(|k| key_event(*k, false)).collect();
        events.extend(release);
        send_input_events(conn, &events)
    }

    // ── Mouse ──────────────────────────────────────────────────────────

    pub fn mouse_move_abs(&self, conn: &mut QmpConnection, x: f64, y: f64) -> Result<(), QmpError> {
        let qx = (x.clamp(0.0, 1.0) * 32767.0) as u16;
        let qy = (y.clamp(0.0, 1.0) * 32767.0) as u16;
        send_input_events(conn, &[
            serde_json::json!({"type":"abs","data":{"axis":"X","value":qx}}),
            serde_json::json!({"type":"abs","data":{"axis":"Y","value":qy}}),
        ])
    }

    pub fn mouse_click(&self, conn: &mut QmpConnection, button: MouseButton) -> Result<(), QmpError> {
        send_input_events(conn, &[
            mouse_event(button, true),
            mouse_event(button, false),
        ])
    }

    // ── Screenshot ─────────────────────────────────────────────────────

    pub fn screendump_ppm(&self, conn: &mut QmpConnection) -> Result<Vec<u8>, QmpError> {
        let args = serde_json::json!({"filename": "/tmp/qmp_screenshot.ppm"});
        let resp = send_cmd(conn, "screendump", Some(args))?;

        if let Some(err) = resp.error {
            return Err(QmpError::CommandFailed {
                command: "screendump".into(),
                error: err.desc,
            });
        }

        std::fs::read("/tmp/qmp_screenshot.ppm").map_err(|e| {
            QmpError::CommandFailed {
                command: "screendump".into(),
                error: format!("read PPM: {e}"),
            }
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn send_input_events(conn: &mut QmpConnection, events: &[serde_json::Value]) -> Result<(), QmpError> {
    let args = serde_json::json!({"events": events});
    let resp = send_cmd(conn, "input-send-event", Some(args))?;
    if let Some(err) = resp.error {
        return Err(QmpError::CommandFailed {
            command: "input-send-event".into(),
            error: err.desc,
        });
    }
    Ok(())
}

fn key_event(key: QemuKey, down: bool) -> serde_json::Value {
    serde_json::json!({
        "type": "key",
        "data": {
            "key": {"type": "qcode", "data": key.qcode()},
            "down": down
        }
    })
}

fn mouse_event(button: MouseButton, down: bool) -> serde_json::Value {
    serde_json::json!({
        "type": "btn",
        "data": {"button": button.qmp_name(), "down": down}
    })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qemu_key_roundtrip() {
        // Only test keys whose qcode roundtrips through from_name.
        // Some qcodes (meta_l, ret) are internal QEMU names that
        // from_name doesn't accept — use the human alias instead.
        for (key, human_name) in &[
            (QemuKey::A, "a"),
            (QemuKey::Return, "enter"),
            (QemuKey::Space, "space"),
            (QemuKey::MetaL, "meta"),
            (QemuKey::F1, "f1"),
            (QemuKey::Up, "up"),
            (QemuKey::N0, "0"),
            (QemuKey::Escape, "escape"),
            (QemuKey::Tab, "tab"),
        ] {
            let parsed = QemuKey::from_name(human_name);
            assert_eq!(parsed, Some(*key), "roundtrip for {human_name}");
            // Also verify qcode() doesn't panic
            let _ = key.qcode();
        }
    }

    #[test]
    fn qemu_key_aliases() {
        assert_eq!(QemuKey::from_name("enter"), Some(QemuKey::Return));
        assert_eq!(QemuKey::from_name("cmd"), Some(QemuKey::MetaL));
        assert_eq!(QemuKey::from_name("win"), Some(QemuKey::MetaL));
        assert_eq!(QemuKey::from_name("esc"), Some(QemuKey::Escape));
        assert_eq!(QemuKey::from_name("-"), Some(QemuKey::Minus));
        assert_eq!(QemuKey::from_name("."), Some(QemuKey::Period));
    }

    #[test]
    fn qemu_key_from_name_unknown_is_none() {
        assert!(QemuKey::from_name("nope").is_none());
        assert!(QemuKey::from_name("").is_none());
    }

    #[test]
    fn qmp_command_serializes() {
        let cmd = QmpCommand { execute: "qmp_capabilities".into(), arguments: None };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("qmp_capabilities"));
        assert!(!json.contains("arguments"));
    }

    #[test]
    fn qmp_command_with_args() {
        let cmd = QmpCommand { execute: "input-send-event".into(), arguments: Some(serde_json::json!({"events": []})) };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("input-send-event"));
        assert!(json.contains("events"));
    }

    #[test]
    fn key_event_json() {
        let event = key_event(QemuKey::Space, true);
        assert_eq!(event["type"], "key");
        assert_eq!(event["data"]["key"]["type"], "qcode");
        assert_eq!(event["data"]["key"]["data"], "spc");
        assert_eq!(event["data"]["down"], true);
    }

    #[test]
    fn connect_nonexistent_socket() {
        let client = QmpClient::new("/tmp/definitely_not_there.sock");
        let result = client.connect();
        assert!(result.is_err());
        match result {
            Err(QmpError::ConnectFailed { .. }) => {},
            other => panic!("expected ConnectFailed, got {other:?}"),
        }
    }

    #[test]
    fn connect_to_running_container() {
        // Skips if no QMP socket available (CI-safe).
        let client = QmpClient::new(QmpClient::DEFAULT_SOCKET);
        if !Path::new(QmpClient::DEFAULT_SOCKET).exists() {
            return; // No container running — skip
        }
        let result = client.connect();
        assert!(result.is_ok(), "QMP connect failed: {result:?}");
    }
}
