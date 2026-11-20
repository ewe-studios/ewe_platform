use foundation_compact::SystemTime;
use foundation_context::Ctx;
use foundation_ui_wire::SignedState;

use super::StateError;

/// A session identifier that the host supplies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    id: String,
}

impl Session {
    /// Makes a session from a nonempty identifier, including for anonymous users.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is empty.
    pub fn new(id: impl Into<String>) -> Result<Self, StateError> {
        let id = id.into();
        if id.is_empty() {
            return Err(StateError::MissingSession);
        }
        Ok(Self { id })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// The state that an action gives to the codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsealedState {
    pub format_version: u16,
    pub component_type: u32,
    pub root_node: u32,
    pub body: Vec<u8>,
    pub private: bool,
}

/// The state after the codec opens it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedState {
    pub format_version: u16,
    pub component_type: u32,
    pub root_node: u32,
    pub body: Vec<u8>,
    pub issued_at: u64,
    pub expires_at: u64,
}

/// The component type and format version that the action expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expect {
    pub component_type: u32,
    pub format_version: u16,
}

/// Makes and reads state buffers.
pub trait StateCodec: Send + Sync {
    /// # Errors
    ///
    /// Returns a state error if the codec cannot seal the state.
    fn seal(
        &self,
        ctx: &Ctx,
        session: &Session,
        state: &UnsealedState,
    ) -> Result<Vec<u8>, StateError>;

    /// # Errors
    ///
    /// Returns the first state error found while opening the buffer.
    fn open(
        &self,
        ctx: &Ctx,
        session: &Session,
        bytes: &[u8],
        expect: Expect,
    ) -> Result<VerifiedState, StateError>;
}

pub(super) fn wall_seconds(ctx: &Ctx) -> Result<u64, StateError> {
    ctx.try_clock()
        .ok_or(StateError::MissingEnv { what: "clock" })?
        .wall_now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|_| StateError::MissingEnv { what: "clock" })
}

pub(super) fn check_body_size(actual: usize, limit: usize) -> Result<(), StateError> {
    if actual > limit {
        return Err(StateError::BodyTooLarge { limit, actual });
    }
    Ok(())
}

pub(super) fn check_component(state: SignedState<'_>, expect: Expect) -> Result<(), StateError> {
    if state.component_type() != expect.component_type
        || state.format_version() != expect.format_version
    {
        return Err(StateError::ComponentMismatch);
    }
    Ok(())
}

pub(super) fn verified(state: SignedState<'_>, body: Vec<u8>) -> VerifiedState {
    VerifiedState {
        format_version: state.format_version(),
        component_type: state.component_type(),
        root_node: state.root_node(),
        body,
        issued_at: state.issued_at(),
        expires_at: state.expires_at(),
    }
}
