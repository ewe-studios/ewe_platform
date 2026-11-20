use foundation_ui_wire::{SignedState, SignedStateT};

const PURPOSE: &[u8] = b"ewe.ui.state.v1";

pub(super) struct MacInput<'a> {
    pub(super) format_version: u16,
    pub(super) flags: u8,
    pub(super) key_id: u32,
    pub(super) issued_at: u64,
    pub(super) expires_at: u64,
    pub(super) component_type: u32,
    pub(super) root_node: u32,
    pub(super) session_binding: &'a [u8],
    pub(super) body: &'a [u8],
}

impl<'a> From<SignedState<'a>> for MacInput<'a> {
    fn from(state: SignedState<'a>) -> Self {
        Self {
            format_version: state.format_version(),
            flags: state.flags(),
            key_id: state.key_id(),
            issued_at: state.issued_at(),
            expires_at: state.expires_at(),
            component_type: state.component_type(),
            root_node: state.root_node(),
            session_binding: state.session_binding().map_or(&[], |field| field.bytes()),
            body: state.body().map_or(&[], |field| field.bytes()),
        }
    }
}

impl<'a> From<&'a SignedStateT> for MacInput<'a> {
    fn from(state: &'a SignedStateT) -> Self {
        Self {
            format_version: state.format_version,
            flags: state.flags,
            key_id: state.key_id,
            issued_at: state.issued_at,
            expires_at: state.expires_at,
            component_type: state.component_type,
            root_node: state.root_node,
            session_binding: state.session_binding.as_deref().unwrap_or_default(),
            body: state.body.as_deref().unwrap_or_default(),
        }
    }
}

impl MacInput<'_> {
    pub(super) fn write(&self, mut write: impl FnMut(&[u8])) {
        write(PURPOSE);
        write(&self.format_version.to_le_bytes());
        write(&[self.flags]);
        write(&self.key_id.to_le_bytes());
        write(&self.issued_at.to_le_bytes());
        write(&self.expires_at.to_le_bytes());
        write(&self.component_type.to_le_bytes());
        write(&self.root_node.to_le_bytes());
        for field in [self.session_binding, self.body] {
            let len =
                u32::try_from(field.len()).expect("The vector length must fit the wire format.");
            write(&len.to_le_bytes());
            write(field);
        }
    }
}

/// Builds the bytes that the state tag covers.
///
/// # Panics
///
/// Panics if a vector length does not fit in a 32-bit unsigned integer.
#[must_use]
pub fn mac_input(state: &SignedStateT) -> Vec<u8> {
    let mut bytes = Vec::new();
    MacInput::from(state).write(|field| bytes.extend_from_slice(field));
    bytes
}
