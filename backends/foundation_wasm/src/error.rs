//! Module for crate errors

use alloc::string::FromUtf16Error;
use alloc::string::FromUtf8Error;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ExternalPointer;
use crate::InternalPointer;
use crate::MemoryId;
use crate::ReturnTypeId;
use crate::ReturnValues;
use crate::ThreeState;

pub type TaskResult<T> = core::result::Result<T, TaskErrorCode>;

/// [`TaskErrorCode`] represents the converted response of an
/// [`ReturnValues::ErrorCode`] when its communicated that a async task
/// or function failed.
///
/// Usually when the only response is [`ReturnValues::ErrorCode`] when
/// the response hint provided did not match that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskErrorCode(pub u16);

impl TaskErrorCode {
    pub fn new(code: u16) -> Self {
        Self(code)
    }
}

impl From<u16> for TaskErrorCode {
    fn from(code: u16) -> Self {
        Self(code)
    }
}

impl From<ReturnValues> for TaskErrorCode {
    fn from(value: ReturnValues) -> Self {
        match &value {
            ReturnValues::ErrorCode(code) => {
                Self(*code)
            }
            _ => unreachable!("We should never attempt to convert anything but a ReturnValues::ErrorCode to a TaskErrorCode. This is a bug in the runtime code. Please report it.")
        }
    }
}

impl core::error::Error for TaskErrorCode {}

impl core::fmt::Display for TaskErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type MemoryWriterResult<T> = core::result::Result<T, MemoryWriterError>;

#[derive(Debug)]
pub enum MemoryWriterError {
    FailedWrite,
    PreviousUnclosedOperation,
    NotValidUTF8(FromUtf8Error),
    NotValidUTF16(FromUtf16Error),
    UnableToWrite,
    UnexpectedFreeState,
    UnexpectedOccupiedState,
}

impl core::error::Error for MemoryWriterError {}

impl core::fmt::Display for MemoryWriterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug)]
pub enum ReturnValueError {
    UnexpectedReturnType,
    InvalidReturnType(ReturnValues),
    ExpectedOne(Vec<ReturnValues>),
    ExpectedList(Vec<ReturnValues>),
    ExpectedMultiple(Vec<ReturnValues>),
    InvalidReturnIds(Vec<ReturnValues>),
}

impl core::error::Error for ReturnValueError {}

impl core::fmt::Display for ReturnValueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type MemoryReaderResult<T> = core::result::Result<T, MemoryReaderError>;

#[derive(Debug)]
pub enum MemoryReaderError {
    NotValidUTF8(FromUtf8Error),
    NotValidUTF16(FromUtf16Error),
    ReturnValueError(ReturnValueError),
    NotValidReplyBinary(BinaryReadError),
}

impl core::error::Error for MemoryReaderError {}

impl core::fmt::Display for MemoryReaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<ReturnValueError> for MemoryReaderError {
    fn from(value: ReturnValueError) -> Self {
        Self::ReturnValueError(value)
    }
}

impl From<BinaryReadError> for MemoryReaderError {
    fn from(value: BinaryReadError) -> Self {
        Self::NotValidReplyBinary(value)
    }
}

impl From<FromUtf16Error> for MemoryReaderError {
    fn from(value: FromUtf16Error) -> Self {
        Self::NotValidUTF16(value)
    }
}

impl From<FromUtf8Error> for MemoryReaderError {
    fn from(value: FromUtf8Error) -> Self {
        Self::NotValidUTF8(value)
    }
}

pub type MemoryAllocationResult<T> = core::result::Result<T, MemoryAllocationError>;

#[derive(Debug)]
pub enum MemoryAllocationError {
    NoMemoryAllocation,
    NoMoreAllocationSlots,
    InvalidAllocationId,
    FailedDeAllocation,
    TaskFailure(TaskErrorCode),
    FailedAllocationReading(MemoryId),
    MemoryReadError(MemoryReaderError),
    MemoryWriteError(MemoryWriterError),
}

impl From<TaskErrorCode> for MemoryAllocationError {
    fn from(value: TaskErrorCode) -> Self {
        Self::TaskFailure(value)
    }
}

impl From<ReturnValueError> for MemoryAllocationError {
    fn from(value: ReturnValueError) -> Self {
        Self::MemoryReadError(value.into())
    }
}

impl From<MemoryReaderError> for MemoryAllocationError {
    fn from(value: MemoryReaderError) -> Self {
        MemoryAllocationError::MemoryReadError(value)
    }
}

impl From<MemoryWriterError> for MemoryAllocationError {
    fn from(value: MemoryWriterError) -> Self {
        MemoryAllocationError::MemoryWriteError(value)
    }
}

impl core::error::Error for MemoryAllocationError {}

impl core::fmt::Display for MemoryAllocationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type BinaryReaderResult<T> = core::result::Result<T, BinaryReadError>;

#[derive(Debug, Clone)]
pub enum BinaryReadError {
    WrongStarterCode(u8),
    UnexpectedBinCode(u8),
    ExpectedStringInCode(u8),
    WrongEndingCode(u8),
    MemoryError(String),
    NotMatchingTypeHint(ThreeState, ReturnTypeId),
}

impl core::error::Error for BinaryReadError {}

impl core::fmt::Display for BinaryReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<MemoryAllocationError> for BinaryReadError {
    fn from(value: MemoryAllocationError) -> Self {
        let content = alloc::format!("MemoryAllocationError({value})");
        Self::MemoryError(content)
    }
}

pub type GuestOperationResult<T> = core::result::Result<T, BinaryReadError>;

#[derive(Debug, Clone)]
pub enum GuestOperationError {
    UnknownInternalPointer(InternalPointer),
    UnknownExternalPointer(ExternalPointer),
}

impl core::error::Error for GuestOperationError {}

impl core::fmt::Display for GuestOperationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type WasmRequestResult<T> = core::result::Result<T, WASMErrors>;

#[derive(Debug)]
pub enum WASMErrors {
    GuestError(GuestOperationError),
    BinaryErrors(BinaryReadError),
    MemoryErrors(MemoryAllocationError),
    WriteErrors(MemoryWriterError),
    ReadErrors(MemoryReaderError),
    ReturnError(ReturnValueError),
}

impl core::error::Error for WASMErrors {}

impl core::fmt::Display for WASMErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<ReturnValueError> for WASMErrors {
    fn from(value: ReturnValueError) -> Self {
        Self::ReturnError(value)
    }
}

impl From<GuestOperationError> for WASMErrors {
    fn from(value: GuestOperationError) -> Self {
        Self::GuestError(value)
    }
}

impl From<BinaryReadError> for WASMErrors {
    fn from(value: BinaryReadError) -> Self {
        Self::BinaryErrors(value)
    }
}

impl From<MemoryReaderError> for WASMErrors {
    fn from(value: MemoryReaderError) -> Self {
        Self::ReadErrors(value)
    }
}

impl From<MemoryWriterError> for WASMErrors {
    fn from(value: MemoryWriterError) -> Self {
        Self::WriteErrors(value)
    }
}

impl From<MemoryAllocationError> for WASMErrors {
    fn from(value: MemoryAllocationError) -> Self {
        Self::MemoryErrors(value)
    }
}
