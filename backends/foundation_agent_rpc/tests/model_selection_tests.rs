//! T13 — the model selection rules of proto.md section 5.
//!
//! One test for each row of the table. Each test checks the Connect code and,
//! where the rule is pure, the selected entry.

use foundation_agent_rpc::core::model::{device_from_wire, model_kind_from_wire};
use foundation_agent_rpc::core::model::{
    select, CatalogEntry, Device, KindSupport, ModelKind, ModelRequest, SelectionError,
};
use foundation_agent_rpc::core::registry::InstanceSpec;
use foundation_agent_rpc::{proto::v1 as pb, AgentRpcError};
use foundation_connectrpc::Code;

fn entries() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry {
            name: "api".into(),
            kind: ModelKind::Api,
            device: Device::Remote,
        },
        CatalogEntry {
            name: "tiny".into(),
            kind: ModelKind::Candle,
            device: Device::Cpu,
        },
        CatalogEntry {
            name: "second".into(),
            kind: ModelKind::Candle,
            device: Device::Cpu,
        },
    ]
}

fn supported() -> Vec<KindSupport> {
    vec![
        KindSupport {
            kind: ModelKind::Api,
            devices: vec![Device::Remote],
        },
        KindSupport {
            kind: ModelKind::Candle,
            devices: vec![Device::Cpu, Device::Cuda { ordinal: 0 }],
        },
    ]
}

fn request(name: Option<&str>, kind: Option<ModelKind>, device: Option<Device>) -> ModelRequest {
    ModelRequest {
        name: name.map(str::to_string),
        kind,
        device,
    }
}

fn code(error: SelectionError) -> Code {
    error.to_connect()
}

#[test]
fn an_unknown_kind_is_invalid_argument() {
    let wire = pb::CreateInstanceRequest {
        kind: buffa::EnumValue::Unknown(99),
        ..Default::default()
    };
    let err = ModelRequest::from_wire(&wire).expect_err("unknown kind");
    assert_eq!(err.current_context().code(), Code::InvalidArgument);
    assert!(matches!(
        err.current_context(),
        AgentRpcError::ModelSelection {
            code: SelectionError::UnknownEnum,
            ..
        }
    ));
}

#[test]
fn an_unknown_device_is_invalid_argument() {
    let wire = pb::CreateInstanceRequest {
        device: Some(pb::Device {
            kind: buffa::EnumValue::Unknown(77),
            ..Default::default()
        })
        .into(),
        ..Default::default()
    };
    let err = ModelRequest::from_wire(&wire).expect_err("unknown device");
    assert_eq!(err.current_context().code(), Code::InvalidArgument);
}

#[test]
fn video_and_audio_are_unimplemented() {
    for kind in [
        pb::ModelKind::MODEL_KIND_VIDEO,
        pb::ModelKind::MODEL_KIND_AUDIO,
    ] {
        let err = model_kind_from_wire(buffa::EnumValue::Known(kind)).expect_err("future kind");
        assert_eq!(
            err.current_context().code(),
            Code::Unimplemented,
            "{kind:?}"
        );
    }
}

#[test]
fn a_named_model_that_does_not_exist_is_not_found() {
    let err = select(
        &entries(),
        &supported(),
        &request(Some("missing"), None, None),
    )
    .expect_err("missing");
    assert_eq!(code(err), Code::NotFound);
}

#[test]
fn a_named_model_with_another_kind_is_invalid_argument() {
    let err = select(
        &entries(),
        &supported(),
        &request(Some("tiny"), Some(ModelKind::Api), None),
    )
    .expect_err("kind");
    assert_eq!(code(err), Code::InvalidArgument);
}

#[test]
fn a_named_model_with_another_device_is_failed_precondition() {
    let err = select(
        &entries(),
        &supported(),
        &request(Some("tiny"), None, Some(Device::Cuda { ordinal: 0 })),
    )
    .expect_err("device");
    assert_eq!(code(err), Code::FailedPrecondition);
}

#[test]
fn a_kind_that_the_host_never_runs_is_unimplemented() {
    let err = select(
        &entries(),
        &supported(),
        &request(None, Some(ModelKind::Gguf), None),
    )
    .expect_err("unsupported kind");
    assert_eq!(code(err), Code::Unimplemented);
}

#[test]
fn a_device_that_the_host_does_not_have_is_failed_precondition() {
    let err = select(
        &entries(),
        &supported(),
        &request(
            None,
            Some(ModelKind::Candle),
            Some(Device::Metal { ordinal: 0 }),
        ),
    )
    .expect_err("unsupported device");
    assert_eq!(code(err), Code::FailedPrecondition);
}

#[test]
fn a_supported_kind_with_no_configured_model_is_not_found() {
    // The host runs Candle on CUDA, but every configured Candle model is on
    // the CPU.
    let err = select(
        &entries(),
        &supported(),
        &request(
            None,
            Some(ModelKind::Candle),
            Some(Device::Cuda { ordinal: 0 }),
        ),
    )
    .expect_err("no model");
    assert_eq!(code(err), Code::NotFound);
}

#[test]
fn more_than_one_matching_model_is_invalid_argument() {
    let err = select(
        &entries(),
        &supported(),
        &request(None, Some(ModelKind::Candle), None),
    )
    .expect_err("ambiguous");
    assert_eq!(code(err), Code::InvalidArgument);
}

#[test]
fn a_remote_kind_with_a_local_device_is_invalid_argument() {
    assert_eq!(
        code(
            select(
                &entries(),
                &supported(),
                &request(None, Some(ModelKind::Api), Some(Device::Cpu)),
            )
            .expect_err("remote kind, local device")
        ),
        Code::InvalidArgument
    );
    assert_eq!(
        code(
            select(
                &entries(),
                &supported(),
                &request(None, Some(ModelKind::Candle), Some(Device::Remote)),
            )
            .expect_err("local kind, remote device")
        ),
        Code::InvalidArgument
    );
}

#[test]
fn an_empty_request_with_one_model_uses_it() {
    let one = [CatalogEntry {
        name: "only".into(),
        kind: ModelKind::Candle,
        device: Device::Cpu,
    }];
    let chosen = select(&one, &supported(), &request(None, None, None)).expect("one model");
    assert_eq!(chosen.name, "only");
}

#[test]
fn a_named_model_ignores_an_unspecified_kind_and_device() {
    let entries = entries();
    let supported = supported();
    let chosen = select(&entries, &supported, &request(Some("tiny"), None, None)).expect("named");
    assert_eq!(chosen.name, "tiny");
}

#[test]
fn the_wire_request_reads_kind_and_device() {
    let wire = pb::CreateInstanceRequest {
        name: "alpha".into(),
        model: "tiny".into(),
        kind: pb::ModelKind::MODEL_KIND_CANDLE.into(),
        device: Some(pb::Device {
            kind: pb::DeviceKind::DEVICE_KIND_CPU.into(),
            ordinal: 0,
            ..Default::default()
        })
        .into(),
        ..Default::default()
    };
    let spec = InstanceSpec::from_wire(&wire).expect("wire");
    assert_eq!(spec.name, "alpha");
    assert_eq!(spec.model.name.as_deref(), Some("tiny"));
    assert_eq!(spec.model.kind, Some(ModelKind::Candle));
    assert_eq!(spec.model.device, Some(Device::Cpu));
}

#[test]
fn an_explicit_device_needs_a_kind() {
    let device = pb::Device::default();
    let err = device_from_wire(&device).expect_err("no kind");
    assert_eq!(err.current_context().code(), Code::InvalidArgument);
}
