//! Model kinds, devices and the selection rules.
//!
//! WHY: a client asks for a model by name, by kind, by device or by any
//! combination. Every host must choose with the same rules and report the same
//! status code when it cannot.
//!
//! WHAT: the Rust [`ModelKind`] and [`Device`] (one variant set that maps
//! one-to-one onto the wire), [`ModelRequest`] and its wire conversion, the
//! catalog types, [`SelectionError`] and the pure function [`select`].
//!
//! HOW: the handlers read the wire request into a [`ModelRequest`]. A
//! [`ModelCatalog`](catalog::ModelCatalog) gives its configured entries and
//! the kinds that the host supports, and [`select`] applies proto.md section 5
//! in the same order as its diagram.

pub mod catalog;
#[cfg(any(feature = "multi", feature = "worker", feature = "dst"))]
pub mod handle;
#[cfg(any(feature = "multi", feature = "worker"))]
pub mod remote;

pub use catalog::{
    select, AgentRpcFuture, CatalogEntry, KindSupport, LoadState, ModelCapabilities, ModelCatalog,
    ModelDescriptor, SelectionError,
};
#[cfg(any(feature = "multi", feature = "worker", feature = "dst"))]
pub use handle::ModelHandle;
#[cfg(any(feature = "multi", feature = "worker"))]
pub use remote::{ApiModelConfig, RemoteCatalog};

/// A loaded model handle. The real handle needs the agent loop, which the
/// `multi` and `dst` builds own. This stub serves a build with neither, so no
/// model can load.
#[cfg(not(any(feature = "multi", feature = "worker", feature = "dst")))]
#[derive(Debug, Clone)]
pub struct ModelHandle;

use buffa::EnumValue;
use foundation_connectrpc::Code;

use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;

/// The family of a model.
///
/// `MODEL_KIND_VIDEO` and `MODEL_KIND_AUDIO` exist on the wire, but no host
/// runs them yet, so they have no Rust variant: the conversion from the wire
/// gives `UNIMPLEMENTED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModelKind {
    /// A model at a remote provider, reached over Hypertext Transfer Protocol
    /// (HTTP).
    Api,
    /// A local safetensors model that runs on Candle.
    Candle,
    /// A local GPT-Generated Unified Format (GGUF) model that runs on
    /// llama.cpp.
    Gguf,
}

impl ModelKind {
    /// The wire value.
    #[must_use]
    pub const fn to_wire(self) -> pb::ModelKind {
        match self {
            Self::Api => pb::ModelKind::MODEL_KIND_API,
            Self::Candle => pb::ModelKind::MODEL_KIND_CANDLE,
            Self::Gguf => pb::ModelKind::MODEL_KIND_GGUF,
        }
    }

    /// The wire value as an `EnumValue`.
    #[must_use]
    pub const fn to_enum_value(self) -> EnumValue<pb::ModelKind> {
        EnumValue::Known(self.to_wire())
    }
}

/// Where a model runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Device {
    /// The central processing unit (CPU).
    Cpu,
    /// A Compute Unified Device Architecture (CUDA) graphics processing unit
    /// (GPU).
    Cuda {
        /// The GPU index.
        ordinal: u32,
    },
    /// An Apple Metal GPU.
    Metal {
        /// The GPU index.
        ordinal: u32,
    },
    /// A Vulkan GPU.
    Vulkan {
        /// The GPU index.
        ordinal: u32,
    },
    /// The remote provider runs the model.
    Remote,
}

impl Device {
    /// The wire value.
    #[must_use]
    pub fn to_wire(self) -> pb::Device {
        let (kind, ordinal) = match self {
            Self::Cpu => (pb::DeviceKind::DEVICE_KIND_CPU, 0),
            Self::Cuda { ordinal } => (pb::DeviceKind::DEVICE_KIND_CUDA, ordinal),
            Self::Metal { ordinal } => (pb::DeviceKind::DEVICE_KIND_METAL, ordinal),
            Self::Vulkan { ordinal } => (pb::DeviceKind::DEVICE_KIND_VULKAN, ordinal),
            Self::Remote => (pb::DeviceKind::DEVICE_KIND_REMOTE, 0),
        };
        pb::Device {
            kind: EnumValue::Known(kind),
            ordinal,
            ..Default::default()
        }
    }

    /// The wire device kind.
    #[must_use]
    pub const fn kind_wire(self) -> pb::DeviceKind {
        match self {
            Self::Cpu => pb::DeviceKind::DEVICE_KIND_CPU,
            Self::Cuda { .. } => pb::DeviceKind::DEVICE_KIND_CUDA,
            Self::Metal { .. } => pb::DeviceKind::DEVICE_KIND_METAL,
            Self::Vulkan { .. } => pb::DeviceKind::DEVICE_KIND_VULKAN,
            Self::Remote => pb::DeviceKind::DEVICE_KIND_REMOTE,
        }
    }

    /// True for a device of a local model.
    #[must_use]
    pub const fn is_local(self) -> bool {
        !matches!(self, Self::Remote)
    }
}

/// What a `CreateInstance` call asks for. `None` means any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelRequest {
    /// A configured model name. Empty means any.
    pub name: Option<String>,
    /// A model kind. `None` means any.
    pub kind: Option<ModelKind>,
    /// A device. `None` means any.
    pub device: Option<Device>,
}

impl ModelRequest {
    /// Read the wire request.
    ///
    /// An unknown enum number gives `INVALID_ARGUMENT`; `VIDEO` or `AUDIO`
    /// gives `UNIMPLEMENTED` (see proto.md section 5).
    ///
    /// # Errors
    /// [`AgentRpcError::ModelSelection`].
    pub fn from_wire(req: &pb::CreateInstanceRequest) -> AgentRpcResult<Self> {
        let kind = model_kind_from_wire(req.kind)?;
        let device = match req.device.as_option() {
            Some(device) => Some(device_from_wire(device)?),
            None => None,
        };
        let name = (!req.model.is_empty()).then(|| req.model.clone());
        Ok(Self { name, kind, device })
    }
}

/// Read one model kind from the wire.
///
/// # Errors
/// [`AgentRpcError::ModelSelection`] for an unknown number or a future kind.
pub fn model_kind_from_wire(value: EnumValue<pb::ModelKind>) -> AgentRpcResult<Option<ModelKind>> {
    match value {
        EnumValue::Known(pb::ModelKind::MODEL_KIND_UNSPECIFIED) => Ok(None),
        EnumValue::Known(pb::ModelKind::MODEL_KIND_API) => Ok(Some(ModelKind::Api)),
        EnumValue::Known(pb::ModelKind::MODEL_KIND_CANDLE) => Ok(Some(ModelKind::Candle)),
        EnumValue::Known(pb::ModelKind::MODEL_KIND_GGUF) => Ok(Some(ModelKind::Gguf)),
        EnumValue::Known(pb::ModelKind::MODEL_KIND_VIDEO) => Err(selection_error(
            SelectionError::KindUnsupported,
            "video models are not implemented",
        )),
        EnumValue::Known(pb::ModelKind::MODEL_KIND_AUDIO) => Err(selection_error(
            SelectionError::KindUnsupported,
            "audio models are not implemented",
        )),
        EnumValue::Unknown(number) => Err(selection_error(
            SelectionError::UnknownEnum,
            format!("unknown model kind {number}"),
        )),
    }
}

/// Read one device from the wire.
///
/// # Errors
/// [`AgentRpcError::ModelSelection`] for an unknown number.
pub fn device_from_wire(device: &pb::Device) -> AgentRpcResult<Device> {
    let kind = match device.kind {
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_UNSPECIFIED) => {
            return Err(selection_error(
                SelectionError::UnknownEnum,
                "an explicit device needs a device kind",
            ));
        }
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_CPU) => Device::Cpu,
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_CUDA) => Device::Cuda {
            ordinal: device.ordinal,
        },
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_METAL) => Device::Metal {
            ordinal: device.ordinal,
        },
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_VULKAN) => Device::Vulkan {
            ordinal: device.ordinal,
        },
        EnumValue::Known(pb::DeviceKind::DEVICE_KIND_REMOTE) => Device::Remote,
        EnumValue::Unknown(number) => {
            return Err(selection_error(
                SelectionError::UnknownEnum,
                format!("unknown device kind {number}"),
            ));
        }
    };
    // A device with an ordinal for CPU or REMOTE says nothing (proto.md
    // section 5: the ordinal is always 0 there).
    Ok(kind)
}

fn selection_error(
    code: SelectionError,
    detail: impl Into<String>,
) -> foundation_errstacks::ErrorTrace<AgentRpcError> {
    AgentRpcError::ModelSelection {
        code,
        detail: detail.into(),
    }
    .into()
}

/// The Connect code of a [`SelectionError`].
impl SelectionError {
    /// The status code of this selection failure.
    #[must_use]
    pub fn to_connect(self) -> Code {
        match self {
            Self::NotFound => Code::NotFound,
            Self::KindMismatch | Self::Ambiguous | Self::RemoteMismatch | Self::UnknownEnum => {
                Code::InvalidArgument
            }
            Self::DeviceMismatch | Self::DeviceUnsupported => Code::FailedPrecondition,
            Self::KindUnsupported => Code::Unimplemented,
        }
    }

    /// A short Sentence in Simplified Technical English for the detail.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::NotFound => "no configured model matches the request",
            Self::KindMismatch => "the named model has another kind",
            Self::DeviceMismatch => "the named model runs on another device",
            Self::KindUnsupported => "this host does not run this kind",
            Self::DeviceUnsupported => "this host does not run this kind on this device",
            Self::Ambiguous => "more than one model matches; name a model",
            Self::RemoteMismatch => "an API model needs the REMOTE device, and the other way round",
            Self::UnknownEnum => "the request has an unknown enum value",
        }
    }
}
