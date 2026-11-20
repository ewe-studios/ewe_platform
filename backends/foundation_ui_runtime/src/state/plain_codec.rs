use foundation_context::Ctx;
use foundation_ui_wire::{finish_signed_state, read_signed_state, SignedStateT};

use super::codec::{check_body_size, check_component, verified, wall_seconds};
use super::error::malformed;
use super::{Expect, Session, StateCodec, StateConfig, StateError, UnsealedState, VerifiedState};

/// Makes untagged state for a trusted device or test.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainStateCodec {
    config: StateConfig,
}

impl StateCodec for PlainStateCodec {
    fn seal(
        &self,
        ctx: &Ctx,
        _session: &Session,
        state: &UnsealedState,
    ) -> Result<Vec<u8>, StateError> {
        check_body_size(state.body.len(), self.config.body_bytes())?;
        let now = wall_seconds(ctx)?;
        let mut sealed = SignedStateT::default();
        sealed.format_version = state.format_version;
        sealed.issued_at = now;
        sealed.component_type = state.component_type;
        sealed.root_node = state.root_node;
        sealed.session_binding = Some(Vec::new());
        sealed.body = Some(state.body.clone());
        sealed.tag = Some(Vec::new());
        Ok(finish_signed_state(&sealed))
    }

    fn open(
        &self,
        _ctx: &Ctx,
        _session: &Session,
        bytes: &[u8],
        expect: Expect,
    ) -> Result<VerifiedState, StateError> {
        let state = read_signed_state(bytes, &self.config.wire_limits())
            .map_err(|error| malformed(error.to_string()))?;
        if state.key_id() != 0 || state.tag().is_some_and(|tag| !tag.is_empty()) {
            return Err(malformed(
                "Plain state requires an empty tag and a zero key identifier.",
            ));
        }
        check_component(state, expect)?;
        let body = state.body().map_or(&[][..], |body| body.bytes());
        check_body_size(body.len(), self.config.body_bytes())?;
        Ok(verified(state, body.to_vec()))
    }
}
