use foundation_compact::crypto::chacha::{decrypt, encrypt_with_nonce};
use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use foundation_ui_wire::{finish_signed_state, read_signed_state, SignedStateT, WireLimits};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::codec::{check_body_size, check_component, verified, wall_seconds};
use super::error::malformed;
use super::mac_input::MacInput;
use super::{
    Expect, Session, StateCodec, StateConfig, StateError, StateKey, StateKeyRing, UnsealedState,
    VerifiedState,
};

const PRIVATE_FLAG: u8 = 1;
const SESSION_PURPOSE: &[u8] = b"ewe.ui.session.v1";
const DIGEST_BYTES: usize = 32;

/// Seals state with a message authentication code and optional body encryption.
pub struct HmacStateCodec {
    ring: StateKeyRing,
    config: StateConfig,
    limits: WireLimits,
}

impl HmacStateCodec {
    #[must_use]
    pub fn new(ring: StateKeyRing, config: StateConfig) -> Self {
        Self {
            ring,
            config,
            limits: config.wire_limits(),
        }
    }
}

impl StateCodec for HmacStateCodec {
    fn seal(
        &self,
        ctx: &Ctx,
        session: &Session,
        state: &UnsealedState,
    ) -> Result<Vec<u8>, StateError> {
        check_body_size(state.body.len(), self.config.body_bytes())?;
        let issued_at = wall_seconds(ctx)?;
        let expires_at = issued_at
            .checked_add(self.config.ttl_value().as_secs())
            .ok_or(StateError::Expired)?;
        let key = &self.ring.current;
        let (flags, body) = if state.private {
            let nonce = draw_nonce(ctx)?;
            let body = encrypt_with_nonce(&key.enc, nonce, &state.body)
                .map_err(|_| StateError::Decrypt)?;
            (PRIVATE_FLAG, body)
        } else {
            (0, state.body.clone())
        };

        let mut sealed = SignedStateT::default();
        sealed.format_version = state.format_version;
        sealed.flags = flags;
        sealed.key_id = key.id();
        sealed.issued_at = issued_at;
        sealed.expires_at = expires_at;
        sealed.component_type = state.component_type;
        sealed.root_node = state.root_node;
        sealed.session_binding = Some(session_binding(session).to_vec());
        sealed.body = Some(body);
        let tag = state_mac(key, &MacInput::from(&sealed))
            .finalize()
            .into_bytes();
        sealed.tag = Some(tag.to_vec());

        Ok(finish_signed_state(&sealed))
    }

    fn open(
        &self,
        ctx: &Ctx,
        session: &Session,
        bytes: &[u8],
        expect: Expect,
    ) -> Result<VerifiedState, StateError> {
        let state =
            read_signed_state(bytes, &self.limits).map_err(|error| malformed(error.to_string()))?;
        let input = MacInput::from(state);
        let tag = state.tag().map_or(&[][..], |value| value.bytes());
        if tag.len() != DIGEST_BYTES || input.session_binding.len() != DIGEST_BYTES {
            return Err(malformed(
                "The tag and session binding must each have 32 bytes.",
            ));
        }

        let lookup = self.ring.find(input.key_id);
        let key = lookup.unwrap_or(&self.ring.current);
        // The comparison also runs when the key identifier is not in the ring.
        let valid_tag = std::hint::black_box(state_mac(key, &input).verify_slice(tag).is_ok());
        let key = lookup.ok_or(StateError::UnknownKey)?;
        if !valid_tag {
            return Err(StateError::BadTag);
        }

        let now = wall_seconds(ctx)?;
        if now < input.issued_at || now >= input.expires_at {
            return Err(StateError::Expired);
        }
        if !bool::from(input.session_binding.ct_eq(&session_binding(session))) {
            return Err(StateError::SessionMismatch);
        }
        check_component(state, expect)?;

        let body = if input.flags & PRIVATE_FLAG != 0 {
            decrypt(&key.enc, input.body)
                .map_err(|_| StateError::Decrypt)?
                .to_vec()
        } else {
            input.body.to_vec()
        };
        check_body_size(body.len(), self.config.body_bytes())?;
        Ok(verified(state, body))
    }
}

fn session_binding(session: &Session) -> [u8; DIGEST_BYTES] {
    let mut hash = Sha256::new();
    hash.update(SESSION_PURPOSE);
    hash.update(session.id().as_bytes());
    hash.finalize().into()
}

fn state_mac(key: &StateKey, input: &MacInput<'_>) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.mac.as_ref())
        .expect("The message authentication code accepts a 32-byte key.");
    input.write(|bytes| mac.update(bytes));
    mac
}

fn draw_nonce(ctx: &Ctx) -> Result<[u8; 12], StateError> {
    let rng = ctx
        .try_rng()
        .ok_or(StateError::MissingEnv { what: "rng" })?;
    let mut nonce = [0; 12];
    rng.lock()
        .map_err(|_| StateError::MissingEnv { what: "rng" })?
        .fill_bytes(&mut nonce);
    Ok(nonce)
}
