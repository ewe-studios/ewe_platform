use std::fmt::Write;
use std::sync::{Arc, Barrier};

use foundation_context::{Ctx, MonotonicClock};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, with_state_binding, HmacStateSigner,
    StateConfig, StateError, StateSigner,
};
use foundation_ui_wire::writer::{write_messages, ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{
    finish_frame, read_frame, ActionRequestT, AttachStateT, FrameKindT, FrameRequestT, FrameT,
    MessageT, OperationT, UiOperationT, WireLimits, SCHEMA_VERSION,
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

fn context() -> Ctx {
    Ctx::with_seed(Arc::new(MonotonicClock::new()), 123)
}

#[test]
fn independent_requests_keep_their_authenticated_user_binding() {
    let base = context();
    assert!(matches!(
        with_state_binding(&base, b"A"),
        Err(StateError::MissingSigner)
    ));
    assert_eq!(sign_state(&base, b"4"), Err(StateError::MissingSigner));
    assert_eq!(verify_state(&base, b"4"), Err(StateError::MissingSigner));
    install_state_signer(
        &base,
        Arc::new(HmacStateSigner::new([3; 32], StateConfig::default()).unwrap()),
    );
    let public = sign_state(&base, b"4").unwrap();
    let meet = Barrier::new(2);
    std::thread::scope(|scope| {
        let render = |user: &[u8]| {
            let request = with_state_binding(&base, user).unwrap();
            meet.wait();
            let signed = sign_state(&request, b"5").unwrap();
            let next_request = with_state_binding(&base, user).unwrap();
            assert_eq!(verify_state(&next_request, &signed).unwrap(), b"5");
            assert!(verify_state(&request, &public).is_err());
            signed
        };
        let alice = scope.spawn(move || render(b"Alice"));
        let bob = render(b"Bob");
        let ctx = with_state_binding(&base, b"Bob").unwrap();
        assert!(verify_state(&ctx, &alice.join().unwrap()).is_err());
        assert!(verify_state(&base, &bob).is_err());
    });
    assert_eq!(verify_state(&base, &public).unwrap(), b"4");
}

// This fixture signer accepts one canonical public counter format.
struct JsonCounter;

impl StateSigner for JsonCounter {
    fn sign(&self, _: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError> {
        let json: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| StateError::BadTag)?;
        let count = json["count"].as_u64().ok_or(StateError::BadTag)?;
        if binding.is_some() || body != format!(r#"{{"count":{count}}}"#).as_bytes() {
            return Err(StateError::BadTag);
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(&[42; 32]).unwrap();
        mac.update(body);
        let mut tag = String::with_capacity(64);
        for byte in mac.finalize().into_bytes() {
            write!(tag, "{byte:02x}").unwrap();
        }
        Ok(format!(r#"{{"count":{count},"tag":"{tag}"}}"#).into_bytes())
    }

    fn verify(
        &self,
        ctx: &Ctx,
        binding: Option<&[u8]>,
        signed: &[u8],
    ) -> Result<Vec<u8>, StateError> {
        let json: serde_json::Value =
            serde_json::from_slice(signed).map_err(|_| StateError::BadTag)?;
        let count = json["count"].as_u64().ok_or(StateError::BadTag)?;
        let body = format!(r#"{{"count":{count}}}"#).into_bytes();
        let expected = self.sign(ctx, binding, &body)?;
        if bool::from(expected.ct_eq(signed)) {
            Ok(body)
        } else {
            Err(StateError::BadTag)
        }
    }
}

#[test]
fn custom_signed_bytes_round_trip_from_render_to_submit() {
    let ctx = context();
    install_state_signer(&ctx, Arc::new(JsonCounter));
    let body = br#"{"count":4}"#;
    let signed = sign_state(&ctx, body).unwrap();
    assert_eq!(
        signed,
        include_bytes!("../../foundation_ui_wire/fixtures/state_counter_custom.json")
    );
    let messages = [40, 42].map(|node| {
        let mut attachment = AttachStateT::default();
        attachment.root_node = node;
        attachment.state = Some(signed.clone());
        let mut operation = OperationT::default();
        operation.ui = UiOperationT::AttachState(Box::new(attachment));
        let mut message = MessageT::default();
        message.operation = Some(Box::new(operation));
        message
    });
    let header = FrameHeader {
        stream_id: 7,
        sequence_id: 1,
        ack: 0,
    };
    let (reply, _) = write_messages(
        messages.into(),
        &ClientCaps::web_current(),
        header,
        &WriterConfig::default(),
    )
    .unwrap();
    let limits = WireLimits::default();
    let mounts = read_frame(&reply, &limits)
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    for (message, node) in mounts.iter().zip([40, 42]) {
        let attached = message.operation().unwrap().ui_as_attach_state().unwrap();
        assert_eq!(attached.root_node(), node);
        let mut action = ActionRequestT::default();
        action.node_id = node;
        action.action = Some("server:count".into());
        action.state = Some(attached.state().unwrap().bytes().to_vec());
        let mut request = FrameRequestT::default();
        request.action = Some(Box::new(action));
        let mut frame = FrameT::default();
        frame.schema_version = SCHEMA_VERSION;
        frame.kind = FrameKindT::FrameRequest(Box::new(request));
        let bytes = finish_frame(&frame).unwrap();
        let returned = read_frame(&bytes, &limits)
            .unwrap()
            .kind_as_frame_request()
            .unwrap()
            .action()
            .unwrap();
        assert_eq!(
            verify_state(&ctx, returned.state().unwrap().bytes()).unwrap(),
            body
        );
    }
    let mut altered = signed;
    altered[9] = b'5';
    assert_eq!(verify_state(&ctx, &altered), Err(StateError::BadTag));
}
