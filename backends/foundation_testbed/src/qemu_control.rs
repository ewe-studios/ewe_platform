//! QEMU Monitor control — universal agentic keyboard/mouse/screenshot (F32 Tier 1).
//!
//! All platform Docker images run QEMU VMs with a monitor Unix socket.
//! Two protocols are auto-detected:
//!   - **HMP** (Human Monitor Protocol): text shell, active on dockurr images.
//!     Commands: `sendkey a`, `screendump /tmp/x.ppm`, `mouse_move 500 500`.
//!   - **QMP** (QEMU Monitor Protocol): JSON-RPC, needs `-qmp` QEMU flag.
//!     Commands: `{"execute":"input-send-event","arguments":{...}}`.
//! `QemuConsole` auto-detects from the greeting and provides one unified API.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ── Error ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum QemuControlError {
    ConnectFailed { path: PathBuf, reason: String },
    CommandFailed { command: String, error: String },
    UnknownKey(String),
    Io(std::io::Error),
}

impl fmt::Display for QemuControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectFailed { path, reason } => {
                write!(f, "QEMU connect to {} failed: {}", path.display(), reason)
            }
            Self::CommandFailed { command, error } => {
                write!(f, "QEMU '{command}' failed: {error}")
            }
            Self::UnknownKey(key) => write!(f, "unknown key: '{key}'"),
            Self::Io(e) => write!(f, "QEMU I/O: {e}"),
        }
    }
}

impl std::error::Error for QemuControlError {}
impl From<std::io::Error> for QemuControlError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

// ── Protocol ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QemuProtocol { Hmp, Qmp }

// ── Key ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    N0, N1, N2, N3, N4, N5, N6, N7, N8, N9,
    Shift, Ctrl, Alt, Meta,
    Up, Down, Left, Right,
    Enter, Space, Tab, Escape, Backspace, Delete,
    Home, End, PageUp, PageDown,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Minus, Equal, Comma, Period, Slash, Semicolon,
}

impl Key {
    fn name(self) -> &'static str {
        match self {
            Self::A => "a", Self::B => "b", Self::C => "c", Self::D => "d",
            Self::E => "e", Self::F => "f", Self::G => "g", Self::H => "h",
            Self::I => "i", Self::J => "j", Self::K => "k", Self::L => "l",
            Self::M => "m", Self::N => "n", Self::O => "o", Self::P => "p",
            Self::Q => "q", Self::R => "r", Self::S => "s", Self::T => "t",
            Self::U => "u", Self::V => "v", Self::W => "w", Self::X => "x",
            Self::Y => "y", Self::Z => "z",
            Self::N0 => "0", Self::N1 => "1", Self::N2 => "2", Self::N3 => "3",
            Self::N4 => "4", Self::N5 => "5", Self::N6 => "6", Self::N7 => "7",
            Self::N8 => "8", Self::N9 => "9",
            Self::Shift => "shift", Self::Ctrl => "ctrl",
            Self::Alt => "alt", Self::Meta => "meta_l",
            Self::Up => "up", Self::Down => "down",
            Self::Left => "left", Self::Right => "right",
            Self::Enter => "ret", Self::Space => "spc", Self::Tab => "tab",
            Self::Escape => "esc", Self::Backspace => "backspace",
            Self::Delete => "delete",
            Self::Home => "home", Self::End => "end",
            Self::PageUp => "pgup", Self::PageDown => "pgdn",
            Self::F1 => "f1", Self::F2 => "f2", Self::F3 => "f3",
            Self::F4 => "f4", Self::F5 => "f5", Self::F6 => "f6",
            Self::F7 => "f7", Self::F8 => "f8", Self::F9 => "f9",
            Self::F10 => "f10", Self::F11 => "f11", Self::F12 => "f12",
            Self::Minus => "minus", Self::Equal => "equal",
            Self::Comma => "comma", Self::Period => "dot",
            Self::Slash => "slash", Self::Semicolon => "semicolon",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "a" => Some(Self::A), "b" => Some(Self::B), "c" => Some(Self::C),
            "d" => Some(Self::D), "e" => Some(Self::E), "f" => Some(Self::F),
            "g" => Some(Self::G), "h" => Some(Self::H), "i" => Some(Self::I),
            "j" => Some(Self::J), "k" => Some(Self::K), "l" => Some(Self::L),
            "m" => Some(Self::M), "n" => Some(Self::N), "o" => Some(Self::O),
            "p" => Some(Self::P), "q" => Some(Self::Q), "r" => Some(Self::R),
            "s" => Some(Self::S), "t" => Some(Self::T), "u" => Some(Self::U),
            "v" => Some(Self::V), "w" => Some(Self::W), "x" => Some(Self::X),
            "y" => Some(Self::Y), "z" => Some(Self::Z),
            "0" => Some(Self::N0), "1" => Some(Self::N1), "2" => Some(Self::N2),
            "3" => Some(Self::N3), "4" => Some(Self::N4), "5" => Some(Self::N5),
            "6" => Some(Self::N6), "7" => Some(Self::N7), "8" => Some(Self::N8),
            "9" => Some(Self::N9),
            "shift" => Some(Self::Shift), "ctrl" => Some(Self::Ctrl),
            "alt" => Some(Self::Alt), "meta" | "cmd" | "win" => Some(Self::Meta),
            "up" => Some(Self::Up), "down" => Some(Self::Down),
            "left" => Some(Self::Left), "right" => Some(Self::Right),
            "enter" | "return" => Some(Self::Enter), "space" => Some(Self::Space),
            "tab" => Some(Self::Tab), "esc" | "escape" => Some(Self::Escape),
            "backspace" => Some(Self::Backspace), "del" | "delete" => Some(Self::Delete),
            "home" => Some(Self::Home), "end" => Some(Self::End),
            "pgup" | "pageup" => Some(Self::PageUp),
            "pgdn" | "pagedown" => Some(Self::PageDown),
            "f1" => Some(Self::F1), "f2" => Some(Self::F2), "f3" => Some(Self::F3),
            "f4" => Some(Self::F4), "f5" => Some(Self::F5), "f6" => Some(Self::F6),
            "f7" => Some(Self::F7), "f8" => Some(Self::F8), "f9" => Some(Self::F9),
            "f10" => Some(Self::F10), "f11" => Some(Self::F11), "f12" => Some(Self::F12),
            "-"|"minus" => Some(Self::Minus), "="|"equal" => Some(Self::Equal),
            ","|"comma" => Some(Self::Comma), "."|"period" => Some(Self::Period),
            "/"|"slash" => Some(Self::Slash), ";"|"semicolon" => Some(Self::Semicolon),
            _ => None,
        }
    }
}

// ── Mouse ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum MouseButton { Left, Middle, Right, WheelUp, WheelDown }

// ── Console ──────────────────────────────────────────────────────────────

pub struct QemuConsole {
    stream: UnixStream,
    pub protocol: QemuProtocol,
}

impl fmt::Debug for QemuConsole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QemuConsole").field("protocol", &self.protocol).finish()
    }
}

/// Strip ANSI CSI escape sequences (HMP uses them for terminal control).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for ch in chars.by_ref() {
                if ch.is_ascii_alphabetic() || ch == '~' { break; }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out.trim().to_string()
}

impl QemuConsole {
    fn read_line_clean(&mut self) -> Result<String, QemuControlError> {
        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(strip_ansi(&line))
    }

    /// Send a command line. For HMP, reads until the `(qemu)` prompt;
    /// for QMP, reads one JSON response line.
    fn send_cmd(&mut self, line: &str) -> Result<String, QemuControlError> {
        self.stream.write_all(line.as_bytes())?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()?;

        let mut result = String::new();
        let mut attempts = 0;
        loop {
            let l = match self.read_line_clean() {
                Ok(l) => l,
                Err(_) => break, // timeout or EOF
            };
            if l.contains("(qemu)") || l.contains("\"return\"") { break; }
            if !l.is_empty() { result.push_str(&l); result.push('\n'); }
            attempts += 1;
            if attempts > 10 { break; }
        }
        Ok(result.trim().to_string())
    }
}

// ── Controller ───────────────────────────────────────────────────────────

pub struct QemuController { socket_path: PathBuf }

impl QemuController {
    /// Default HMP monitor socket (always present on dockurr images).
    pub const DEFAULT_MONITOR: &str = "/run/shm/monitor.sock";
    /// QMP socket — enabled via ARGUMENTS env var in docker-compose (F32).
    pub const QMP_SOCKET: &str = "/run/shm/qmp.sock";

    #[must_use] pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Prefer QMP if available (F32), otherwise fall back to HMP.
    #[must_use]
    pub fn auto() -> Self {
        if Path::new(Self::QMP_SOCKET).exists() {
            Self::new(Self::QMP_SOCKET)
        } else {
            Self::new(Self::DEFAULT_MONITOR)
        }
    }

    /// Connect, auto-detecting HMP vs QMP from the greeting.
    pub fn connect(&self) -> Result<QemuConsole, QemuControlError> {
        let path = &self.socket_path;
        if !path.exists() {
            return Err(QemuControlError::ConnectFailed { path: path.clone(), reason: "no socket".into() });
        }
        let stream = UnixStream::connect(path).map_err(|e| {
            QemuControlError::ConnectFailed { path: path.clone(), reason: e.to_string() }
        })?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        let mut console = QemuConsole { stream, protocol: QemuProtocol::Hmp };
        let greeting = console.read_line_clean().unwrap_or_default();
        console.protocol = if greeting.contains("QMP") { QemuProtocol::Qmp } else { QemuProtocol::Hmp };
        if console.protocol == QemuProtocol::Qmp {
            console.send_cmd("{\"execute\":\"qmp_capabilities\"}")?;
        }
        Ok(console)
    }

    // ── Keyboard ──────────────────────────────────────────────────────

    pub fn send_key(&self, conn: &mut QemuConsole, key: Key) -> Result<(), QemuControlError> {
        match conn.protocol {
            QemuProtocol::Hmp => { conn.send_cmd(&format!("sendkey {}", key.name()))?; }
            QemuProtocol::Qmp => send_qmp_events(conn, &qmp_key_events(key.name()))?,
        }
        Ok(())
    }

    /// Key combo — all held together, then released. HMP: `sendkey ctrl-alt-del`.
    pub fn send_keys(&self, conn: &mut QemuConsole, keys: &[Key]) -> Result<(), QemuControlError> {
        match conn.protocol {
            QemuProtocol::Hmp => {
                let combo = keys.iter().map(|k| k.name()).collect::<Vec<_>>().join("-");
                conn.send_cmd(&format!("sendkey {combo}"))?;
            }
            QemuProtocol::Qmp => {
                let mut events: Vec<serde_json::Value> = keys.iter()
                    .map(|k| serde_json::json!({"type":"key","data":{"key":{"type":"qcode","data":k.name()},"down":true}}))
                    .collect();
                let release: Vec<serde_json::Value> = keys.iter().rev()
                    .map(|k| serde_json::json!({"type":"key","data":{"key":{"type":"qcode","data":k.name()},"down":false}}))
                    .collect();
                events.extend(release);
                let payload = serde_json::json!({"events": events});
                conn.send_cmd(&format!(r#"{{"execute":"input-send-event","arguments":{}}}}}"#, serde_json::to_string(&payload).unwrap()))?;
            }
        }
        Ok(())
    }

    // ── Mouse ──────────────────────────────────────────────────────

    /// Mouse move to absolute coords (0.0-1.0 → 0-32767 QEMU tablet range).
    pub fn mouse_move_abs(&self, conn: &mut QemuConsole, x: f64, y: f64) -> Result<(), QemuControlError> {
        let qx = (x.clamp(0.0, 1.0) * 32767.0) as u16;
        let qy = (y.clamp(0.0, 1.0) * 32767.0) as u16;
        match conn.protocol {
            QemuProtocol::Hmp => { conn.send_cmd(&format!("mouse_move {qx} {qy}"))?; }
            QemuProtocol::Qmp => send_qmp_events(conn, &[
                serde_json::json!({"type":"abs","data":{"axis":"X","value":qx}}),
                serde_json::json!({"type":"abs","data":{"axis":"Y","value":qy}}),
            ])?,
        }
        Ok(())
    }

    /// Mouse click.
    pub fn mouse_click(&self, conn: &mut QemuConsole, button: MouseButton) -> Result<(), QemuControlError> {
        match conn.protocol {
            QemuProtocol::Hmp => {
                let btn = match button {
                    MouseButton::Left => 1, MouseButton::Middle => 2,
                    MouseButton::Right => 3, MouseButton::WheelUp => 4,
                    MouseButton::WheelDown => 5,
                };
                conn.send_cmd(&format!("mouse_button {btn}"))?;
            }
            QemuProtocol::Qmp => {
                let qbtn = match button {
                    MouseButton::Left => "Left", MouseButton::Middle => "Middle",
                    MouseButton::Right => "Right", MouseButton::WheelUp => "WheelUp",
                    MouseButton::WheelDown => "WheelDown",
                };
                send_qmp_events(conn, &[
                    serde_json::json!({"type":"btn","data":{"button":qbtn,"down":true}}),
                    serde_json::json!({"type":"btn","data":{"button":qbtn,"down":false}}),
                ])?;
            }
        }
        Ok(())
    }

    // ── Convenience ────────────────────────────────────────────────────

    /// Type a string by sending individual key events.
    /// Maps ASCII chars to QEMU keys. Space → spc, Enter → ret, etc.
    pub fn type_text(&self, conn: &mut QemuConsole, text: &str) -> Result<(), QemuControlError> {
        for ch in text.chars() {
            let key = match ch {
                'a'..='z' => Key::from_name(&ch.to_string()).unwrap(),
                'A'..='Z' => Key::from_name(&ch.to_lowercase().to_string()).unwrap(),
                '0'..='9' => Key::from_name(&ch.to_string()).unwrap(),
                ' ' => Key::Space,
                '\n' | '\r' => Key::Enter,
                '\t' => Key::Tab,
                '-' => Key::Minus,
                '=' => Key::Equal,
                ',' => Key::Comma,
                '.' => Key::Period,
                '/' => Key::Slash,
                ';' => Key::Semicolon,
                _ => continue, // skip unmappable chars
            };
            self.send_key(conn, key)?;
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    /// Convenience: send a key combination by name.
    /// `ctrl("ctrl", &["alt", "delete"])` → send_keys with ctrl held.
    pub fn key_combo(&self, conn: &mut QemuConsole, modifier: &str, keys: &[&str]) -> Result<(), QemuControlError> {
        let mod_key = Key::from_name(modifier).ok_or_else(|| QemuControlError::UnknownKey(modifier.to_string()))?;
        let mut all: Vec<Key> = vec![mod_key];
        for k in keys {
            all.push(Key::from_name(k).ok_or_else(|| QemuControlError::UnknownKey(k.to_string()))?);
        }
        self.send_keys(conn, &all)
    }

    // ── Screenshot ─────────────────────────────────────────────────────

    /// Capture screenshot as PPM bytes (host-side file).
    pub fn screendump(&self, conn: &mut QemuConsole) -> Result<Vec<u8>, QemuControlError> {
        let tmp = "/tmp/qemu_screendump.ppm";
        match conn.protocol {
            QemuProtocol::Hmp => { conn.send_cmd(&format!("screendump {tmp}"))?; }
            QemuProtocol::Qmp => {
                conn.send_cmd(&format!(r#"{{"execute":"screendump","arguments":{{"filename":"{tmp}"}}}}"#))?;
            }
        }
        std::thread::sleep(Duration::from_millis(300));
        std::fs::read(tmp).map_err(|e| QemuControlError::CommandFailed {
            command: "screendump".into(), error: format!("read {tmp}: {e}"),
        })
    }

    /// Capture screenshot as PNG bytes (converts PPM via imagemagick).
    pub fn screendump_png(&self, conn: &mut QemuConsole) -> Result<Vec<u8>, QemuControlError> {
        let ppm = self.screendump(conn)?;
        // Convert PPM → PNG via imagemagick's `convert` command
        let mut child = std::process::Command::new("convert")
            .args(["ppm:-", "png:-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| QemuControlError::CommandFailed {
                command: "convert".into(),
                error: format!("imagemagick not found: {e}"),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(&ppm);
        }

        let output = child.wait_with_output().map_err(|e| QemuControlError::Io(e))?;
        Ok(output.stdout)
    }
}

fn qmp_key_events(qcode: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"type":"key","data":{"key":{"type":"qcode","data":qcode},"down":true}}),
        serde_json::json!({"type":"key","data":{"key":{"type":"qcode","data":qcode},"down":false}}),
    ]
}

fn send_qmp_events(conn: &mut QemuConsole, events: &[serde_json::Value]) -> Result<(), QemuControlError> {
    let payload = serde_json::json!({"events": events});
    conn.send_cmd(&format!(r#"{{"execute":"input-send-event","arguments":{}}}}}"#, serde_json::to_string(&payload).unwrap()))?;
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
            (Key::A, "a"), (Key::Enter, "enter"), (Key::Space, "space"),
            (Key::Meta, "meta"), (Key::F1, "f1"), (Key::Up, "up"),
            (Key::N0, "0"), (Key::Escape, "escape"), (Key::Tab, "tab"),
            (Key::Minus, "-"), (Key::Period, "."),
        ] { assert_eq!(Key::from_name(name), Some(*key), "{name}"); }
    }

    #[test]
    fn key_from_name_unknown_is_none() { assert!(Key::from_name("nope").is_none()); }

    #[test]
    fn connect_nonexistent_socket() {
        let ctrl = QemuController::new("/tmp/nope");
        assert!(ctrl.connect().is_err());
    }

    #[test]
    fn connect_windows_container_e2e() {
        let ctrl = QemuController::new(QemuController::DEFAULT_MONITOR);
        if !Path::new(QemuController::DEFAULT_MONITOR).exists() { return; }
        let mut conn = ctrl.connect().unwrap();
        assert_eq!(conn.protocol, QemuProtocol::Hmp);

        // Send a key
        ctrl.send_key(&mut conn, Key::A).unwrap();

        // Take screenshot
        let ppm = ctrl.screendump(&mut conn).unwrap();
        assert!(ppm.len() > 1000, "screenshot too small: {} bytes", ppm.len());
        assert!(&ppm[..2] == b"P6", "not PPM: {:02x?}", &ppm[..4]);
    }
}
