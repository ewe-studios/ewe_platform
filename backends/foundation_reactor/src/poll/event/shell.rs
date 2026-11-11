/// Shell `Event` for platforms without a real readiness selector (for example
/// `wasm32`). The shell selector never produces events, so every accessor
/// returns a neutral default. It exists so `Events` and the shell selector
/// compile on those targets.
use crate::poll::Token;

#[derive(Clone, Copy)]
pub struct Event {
    token: Token,
}

impl Event {
    /// The token this event was registered for. Always `Token(0)`: the shell
    /// selector never delivers events.
    pub fn token(&self) -> Token {
        self.token
    }

    /// Always `false`.
    pub fn is_readable(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn is_writable(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn is_error(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn is_read_closed(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn is_write_closed(&self) -> bool {
        false
    }

    /// Always `false`.
    pub fn is_priority(&self) -> bool {
        false
    }
}
