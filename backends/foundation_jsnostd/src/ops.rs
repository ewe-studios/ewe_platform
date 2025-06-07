#![allow(dead_code)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::vec::Vec;

use crate::{
    value_quantitization, ArgumentOperations, BatchEncodable, Batchable, CompletedInstructions,
    ExternalPointer, InternalPointer, MemoryAllocation, MemoryAllocationResult, MemoryAllocations,
    MemoryId, MemorySlot, MemoryWriterError, MemoryWriterResult, ToBinary, TypeOptimization,
};

use super::{Operations, Params, StrLocation, ValueTypes};

const DEFAULT_ALLOCATION_SIZE: usize = 10;
static ARGUMENT_ENDER: &[u8] = &[ArgumentOperations::Stop as u8];
static ARGUMENT_STARTER: &[u8] = &[ArgumentOperations::Start as u8];

/// [`MemoryPointer`] holds references memory ids for both text and operations.
pub struct MemoryPointer(MemoryId, MemoryId);

impl MemoryPointer {
    pub(crate) fn new(texts: MemoryId, ops: MemoryId) -> Self {
        Self(texts, ops)
    }

    /// [`text_id`] returns the `MemoryId` for the text data.
    pub fn text_id(&self) -> MemoryId {
        self.0.clone()
    }

    /// [`ops_id`] returns the `MemoryId` for the operations data.
    pub fn ops_id(&self) -> MemoryId {
        self.1.clone()
    }
}

impl<'a> Batchable<'a> for Vec<Params<'a>> {
    fn encode<F>(&self, encoder: &'a F, optimize: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        encoder.data(ARGUMENT_STARTER)?;
        for param in self.iter() {
            param.encode(encoder, optimize)?;
        }
        encoder.data(ARGUMENT_ENDER)?;
        Ok(())
    }
}

impl<'a> Batchable<'a> for &'a [Params<'a>] {
    fn encode<F>(&self, encoder: &'a F, optimize: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        encoder.data(ARGUMENT_STARTER)?;
        for param in self.iter() {
            param.encode(encoder, optimize)?;
        }
        encoder.data(ARGUMENT_ENDER)?;
        Ok(())
    }
}

impl ToBinary for Vec<Params<'_>> {
    fn to_binary(&self) -> Vec<u8> {
        let mut encoded_params: Vec<u8> = Vec::new();
        for param in self.iter() {
            encoded_params.extend(param.to_binary())
        }
        encoded_params
    }
}

impl<'a> ToBinary for &'a [Params<'a>] {
    fn to_binary(&self) -> Vec<u8> {
        let mut encoded_params: Vec<u8> = Vec::new();
        for param in self.iter() {
            encoded_params.extend(param.to_binary())
        }
        encoded_params
    }
}

impl ToBinary for Params<'_> {
    fn to_binary(&self) -> Vec<u8> {
        let mut encoded_params: Vec<u8> = Vec::new();
        encoded_params.push(self.to_value_type_u8());

        match self {
            Params::Undefined => {}
            Params::Null => {}
            Params::Float64(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::CachedText(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Uint64(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Text8(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::ExternalReference(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Float32(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Bool(value) => {
                if *value {
                    encoded_params.push(1);
                } else {
                    encoded_params.push(0);
                }
            }
            Params::Float64Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Uint32Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Int8(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Int16(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Int32(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Int64(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Uint8(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Uint16(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Uint32(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Int128(value) => {
                // nice trick to switch all bits to 1 for a 64bit number.
                const MASK: i128 = 0xFFFFFFFFFFFFFFFF;

                // get the MSB by shifting right 64 bits
                let value_msb = (value >> 64) as i64;

                // get the LSB by truncating to u64, this gets automatically truncated
                // but we also just use u64::MAX to mask it to the lowest 64 bits
                // or LSB.
                let value_lsb = (value & MASK) as i64;

                let msb_bytes = value_msb.to_le_bytes();
                let lsb_bytes = value_lsb.to_le_bytes();

                encoded_params.extend_from_slice(&msb_bytes);
                encoded_params.extend_from_slice(&lsb_bytes);
            }
            Params::Uint128(value) => {
                // nice trick to switch all bits to 1 for a 64bit number.
                const MASK: u128 = 0xFFFFFFFFFFFFFFFF;

                // get the MSB by shifting right 64 bits
                let value_msb = (value >> 64) as u64;

                // get the LSB by truncating to u64, this gets automatically truncated
                // but we also just use u64::MAX to mask it to the lowest 64 bits
                // or LSB.
                let value_lsb = (value & MASK) as u64;

                let msb_bytes = value_msb.to_le_bytes();
                let lsb_bytes = value_lsb.to_le_bytes();

                encoded_params.extend_from_slice(&msb_bytes);
                encoded_params.extend_from_slice(&lsb_bytes);
            }
            Params::Text16(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::InternalReference(value) => {
                encoded_params.extend_from_slice(&value.to_le_bytes());
            }
            Params::Uint64Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Int32Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Int64Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Int8Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Int16Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Uint8Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Uint16Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::Float32Array(value) => {
                let start = value.as_ptr() as u64;
                let len = value.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
            Params::TypedArraySlice(slice_type, slice_content) => {
                encoded_params.push(slice_type.into());

                let start = slice_content.as_ptr() as u64;
                let len = slice_content.len() as u64;
                encoded_params.extend_from_slice(&start.to_le_bytes());
                encoded_params.extend_from_slice(&len.to_le_bytes());
            }
        };

        encoded_params
    }
}

impl<'a> Batchable<'a> for Params<'a> {
    fn encode<F>(&self, encoder: &'a F, optimized: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        match self {
            Params::Undefined => {
                let data: Vec<u8> = alloc::vec![
                    ArgumentOperations::Begin.into(),
                    self.to_value_type().into(),
                    ArgumentOperations::End.into(),
                ];

                encoder.data(&data)?;
                Ok(())
            }
            Params::Null => {
                let data: Vec<u8> = alloc::vec![
                    ArgumentOperations::Begin.into(),
                    self.to_value_type().into(),
                    ArgumentOperations::End.into(),
                ];

                encoder.data(&data)?;
                Ok(())
            }
            Params::Bool(value) => {
                let indicator = if *value { 1 } else { 0 };
                let data: Vec<u8> = alloc::vec![
                    ArgumentOperations::Begin.into(),
                    self.to_value_type().into(),
                    indicator,
                    ArgumentOperations::End.into(),
                ];

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float64(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qf64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 7);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float32(value) => {
                let value_bytes = value.to_le_bytes();
                let total_length = value_bytes.len() + 3;

                let mut data: Vec<u8> = Vec::with_capacity(total_length);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int8(value) => {
                let value_bytes = value.to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int16(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qi16(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int32(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qi32(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int64(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qi64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint8(value) => {
                let value_bytes = value.to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint16(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu16(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint32(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu32(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint64(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int128(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qi128(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint128(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu128(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::CachedText(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Text8(value) => {
                let value_pointer = encoder.string(value)?;

                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_index_bytes, tq_index) = if optimized {
                    value_quantitization::qu64(value_pointer.index())
                } else {
                    (
                        value_pointer.index().to_le_bytes().to_vec(),
                        TypeOptimization::None,
                    )
                };

                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value_pointer.len())
                } else {
                    (
                        value_pointer.len().to_le_bytes().to_vec(),
                        TypeOptimization::None,
                    )
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_index_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq_index.into());
                data.extend_from_slice(&value_index_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Text16(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::TypedArraySlice(slice_type, value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr())
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(slice_type.into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float32Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float64Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint32Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint8Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr())
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint16Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint64Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int32Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int64Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int8Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int16Array(value) => {
                // TODO(alex): Is there a more optimized way instead of `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qpointer(value.as_ptr() as *const u8)
                } else {
                    let value_pointer = value.as_ptr() as usize;
                    (value_pointer.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let (value_length_bytes, tq_len) = if optimized {
                    value_quantitization::qu64(value.len() as u64)
                } else {
                    (value.len().to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_bytes.len() + value_length_bytes.len() + 5);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(tq_len.into());
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::ExternalReference(value) => {
                // TODO(alex): Is there a more optimized way instead of
                // `to_vec()` which does a copy.
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::InternalReference(value) => {
                let (value_bytes, tq) = if optimized {
                    value_quantitization::qu64(*value)
                } else {
                    (value.to_le_bytes().to_vec(), TypeOptimization::None)
                };

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 4);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.push(tq.into());
                data.extend_from_slice(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod params_tests {
    use crate::TypedSlice;

    use super::*;

    #[test]
    fn can_encode_undefined_and_null() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[Params::Undefined, Params::Null]));

        assert!(write_result.is_ok());

        batch.end().expect("end instruction");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Undefined as u8,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Null as u8,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_floats() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result =
            batch.encode_params(Some(&[Params::Float32(10.2), Params::Float64(10.2)]));

        assert!(write_result.is_ok());

        batch.end().expect("end instruction");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Float32 as u8,
                51,
                51,
                35,
                65,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Float64 as u8,
                TypeOptimization::QuantizedF64AsF32 as u8,
                51,
                51,
                35,
                65,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_bool() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[Params::Bool(true), Params::Bool(false)]));

        assert!(write_result.is_ok());

        batch.end().expect("end instruction");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Bool as u8,
                1,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Bool as u8,
                0,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_uints() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[
            Params::Uint8(10),
            Params::Uint16(10),
            Params::Uint32(10),
            Params::Uint64(10),
            Params::Uint128(10),
        ]));

        assert!(write_result.is_ok());

        batch.end().expect("completed");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Uint8 as u8,
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Uint16 as u8,
                TypeOptimization::QuantizedUint16AsU8 as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Uint32 as u8,
                TypeOptimization::QuantizedUint32AsU8 as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Uint64 as u8,
                TypeOptimization::QuantizedUint64AsU8 as u8,
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Uint128 as u8,
                TypeOptimization::QuantizedUint128AsU8 as u8,
                10,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_typed_array_slice() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[u8] = &[1, 1];
        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let write_result =
            batch.encode_params(Some(&[Params::TypedArraySlice(TypedSlice::Uint8, items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::TypedArraySlice as u8,
            TypedSlice::Uint8 as u8, // type of slice
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_cached_text() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[Params::CachedText(10)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::CachedText as u8,
                TypeOptimization::QuantizedUint64AsU8 as u8,
                10,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_ints() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[
            Params::Int8(10),
            Params::Int16(10),
            Params::Int32(10),
            Params::Int64(10),
            Params::Int128(10),
        ]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert_eq!(
            alloc::vec![
                0,                               // Begin signal indicating start of batch
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int8 as u8,
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int16 as u8,
                TypeOptimization::QuantizedInt16AsI8 as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int32 as u8,
                TypeOptimization::QuantizedInt32AsI8 as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int64 as u8,
                TypeOptimization::QuantizedInt64AsI8 as u8,
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int128 as u8,
                TypeOptimization::QuantizedInt128AsI8 as u8,
                10,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                254,                            // end of the sub-block of instruction
                255                             // Stop signal indicating batch is finished
            ],
            completed_ops.clone_memory().expect("clone"),
        );

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_texts() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let content = "alex";
        let content_u16: Vec<u16> = content.encode_utf16().collect();

        let write_result = batch.encode_params(Some(&[
            Params::Text8(content),
            Params::Text16(content_u16.as_slice()),
        ]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Text8 as u8,
            TypeOptimization::QuantizedUint64AsU8 as u8,
            0,
            TypeOptimization::QuantizedUint64AsU8 as u8,
            4,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Text16 as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            4,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (content_u16.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert_eq!(4, completed_strings.len().expect("returns state"));
        assert_eq!(&[97, 108, 101, 120], content.as_bytes());
    }

    #[test]
    fn can_encode_float64_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[f64] = &[1.0, 2.0];
        let write_result = batch.encode_params(Some(&[Params::Float64Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Float64ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_float32_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[f32] = &[1.0, 2.0];
        let write_result = batch.encode_params(Some(&[Params::Float32Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Float32ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_int8_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[i8] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Int8Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Int8ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_int16_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[i16] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Int16Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Int16ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_int32_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[i32] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Int32Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Int32ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_int64_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[i64] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Int64Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Int64ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_uint8_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[u8] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Uint8Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Uint8ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_uint16_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[u16] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Uint16Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Uint16ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_uint32_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[u32] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Uint32Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Uint32ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_uint64_arrays() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let items: &[u64] = &[1, 2];
        let write_result = batch.encode_params(Some(&[Params::Uint64Array(items)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");
        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::Uint64ArrayBuffer as u8,
            TypeOptimization::None as u8,
        ];

        let encoded_end = alloc::vec![
            TypeOptimization::QuantizedUint64AsU8 as u8,
            2,
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let pointer_bytes = (items.as_ptr() as u64).to_le_bytes();

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(pointer_bytes);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_external_pointer() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[Params::ExternalReference(0)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::ExternalReference as u8,
            TypeOptimization::QuantizedUint64AsU8 as u8,
            0,
        ];

        let encoded_end = alloc::vec![
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }

    #[test]
    fn can_encode_internal_pointer() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let write_result = batch.encode_params(Some(&[Params::InternalReference(0)]));

        assert!(write_result.is_ok());

        batch.end().expect("ended");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        let encoded_start = alloc::vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ValueTypes::InternalReference as u8,
            TypeOptimization::QuantizedUint64AsU8 as u8,
            0,
        ];

        let encoded_end = alloc::vec![
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            254,                            // end of the sub-block of instruction
            255                             // Stop signal indicating batch is finished
        ];

        let mut encoded = Vec::new();
        encoded.extend(encoded_start);
        encoded.extend(encoded_end);

        assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

        assert!(completed_strings.is_empty().expect("returns state"));
    }
}

/// [`Instructions`] is a one batch set writer, meaning it encodes a single
/// batch of instruction marked by a [`Operations::Begin`] and [`Operations::Stop`]
/// markers when the [`Instructions::end`] is called.
///
/// When the end is called the allocated memory slot used by the [`Instructions`]
/// instance is marked completed and both returned by the call to [`Instructions::end`]
/// and also the provided callback the [`MemoryAllocations::batch_for`] gets.
///
/// From then on the allocator can do whatever it wants with that memory register (mostly
/// send the information to the other side) to process the batch.
pub struct Instructions {
    optimized: bool,
    ops_id: MemoryId,
    text_id: MemoryId,
    mem: Option<(MemoryAllocation, MemoryAllocation)>,
}

// -- Constructors

impl Instructions {
    pub fn new(
        optimized: bool,
        ops_id: MemoryId,
        text_id: MemoryId,
        ops: MemoryAllocation,
        texts: MemoryAllocation,
    ) -> Self {
        Self {
            ops_id,
            text_id,
            optimized,
            mem: Some((ops, texts)),
        }
    }
}

// -- Instructions private

impl Instructions {
    /// [`begin`] starts a new operation to be encoded into the Instructions set
    /// if a operation was not properly closed then an error
    /// [`MemoryWriterError::PreviousUnclosedOperation`] is returned.
    pub(crate) fn begin(&self) -> MemoryWriterResult<()> {
        if self.in_free_state() {
            return Err(MemoryWriterError::PreviousUnclosedOperation);
        }

        if let Some((op, _)) = &self.mem {
            op.apply(|mem| {
                mem.push(Operations::Begin as u8);
            });
        }
        Ok(())
    }
}

// --- MemoryIds

impl Instructions {
    pub fn pointer(&self) -> MemoryPointer {
        MemoryPointer::new(self.text_id.clone(), self.ops_id.clone())
    }

    /// [`text_id`] returns the `MemoryId` for the text data.
    pub fn text_id(&self) -> MemoryId {
        self.text_id.clone()
    }

    /// [`ops_id`] returns the `MemoryId` for the operations data.
    pub fn ops_id(&self) -> MemoryId {
        self.ops_id.clone()
    }
}

// -- Operations: checker

impl Instructions {
    pub fn in_occupied_state(&self) -> bool {
        self.mem.is_some()
    }

    pub fn in_free_state(&self) -> bool {
        self.mem.is_none()
    }

    pub fn should_be_occupied(&self) -> MemoryWriterResult<()> {
        if self.in_free_state() {
            return Err(MemoryWriterError::UnexpectedFreeState);
        }
        Ok(())
    }

    pub fn should_be_free(&self) -> MemoryWriterResult<()> {
        if self.in_occupied_state() {
            let var_name = Err(MemoryWriterError::UnexpectedOccupiedState);
            return var_name;
        }
        Ok(())
    }
}

// -- complete instruction

impl Instructions {
    pub fn complete(self) -> MemoryAllocationResult<CompletedInstructions> {
        Ok(self.stop()?)
    }
}

// -- Creating Instructions from MemoryAllocations

impl MemoryAllocations {
    /// [`batch_for`] creates a new memory slot for encoding a singular instruction
    /// batch.
    pub fn batch_for(
        &mut self,
        text_capacity: u64,
        operations_capacity: u64,
        optimized: bool,
    ) -> MemoryAllocationResult<Instructions> {
        let operations_id = self.allocate(operations_capacity)?;
        let operations_buffer = self.get(operations_id.clone())?;

        let text_id = self.allocate(text_capacity)?;
        let text_buffer = self.get(text_id.clone())?;

        let instruction = Instructions::new(
            optimized,
            operations_id,
            text_id,
            operations_buffer,
            text_buffer,
        );

        // mark the instruction as started.
        instruction.begin()?;
        Ok(instruction)
    }

    /// [`batch_from`] allows you continue building new batch instructions
    /// from an already completed instruction memory slot. This added when you
    /// are sure you do not need any immediate execution of the previous instruction
    /// and will use callbacks or other means of retrieving results async or at a future
    /// time, allowing you to encode as much information as possible before deliverying
    /// to the other side, but also be aware this also means any potential error on
    /// the host side that is caused by a batch will affect finality of your whole
    /// batch.
    pub fn batch_from(
        &self,
        optimized: bool,
        completed: CompletedInstructions,
    ) -> MemoryAllocationResult<Instructions> {
        let operations = self.get(completed.ops_id.clone())?;
        let text_buffer = self.get(completed.text_id.clone())?;
        let instruction = Instructions::new(
            optimized,
            completed.ops_id.clone(),
            completed.text_id.clone(),
            operations,
            text_buffer,
        );

        // mark the instruction as started.
        instruction.begin()?;
        Ok(instruction)
    }

    /// [`get_memory`] retrieve the underlying memory allocation from the [`CompletedInstructions`] which can
    /// allow you to inspect or interact with its raw contents as a [`MemoryAllocation`].
    pub fn get_slot(&self, completed: CompletedInstructions) -> MemoryAllocationResult<MemorySlot> {
        let operation_buffer = self.get(completed.ops_id.clone())?;
        let text_buffer = self.get(completed.text_id.clone())?;
        Ok(MemorySlot::new(operation_buffer, text_buffer))
    }
}

// -- Implements BatchEncodable

impl BatchEncodable for Instructions {
    fn string(&self, data: &str) -> MemoryWriterResult<StrLocation> {
        const SPACE_BYTES: u8 = b' ';

        if self.in_occupied_state() {
            if let Some((_, text)) = &self.mem {
                let data_bytes = data.as_bytes();
                let text_location = text.len()?;
                let text_length = data_bytes.len() as u64;

                // we want to account for more space when the bytes
                // length is way more than, because we want to padd the
                // added string because we use the byte length to correctly
                // identify where we are selecting from.
                let byte_length_diff = data_bytes.len() - data.len();

                text.apply(|mem| {
                    mem.extend_from_slice(data_bytes);

                    // pad string with difference if byte length is more than string
                    // length so we do not end up picking up the next string on the host
                    // side.
                    if byte_length_diff > 0 {
                        for _ in 0..byte_length_diff {
                            mem.push(SPACE_BYTES);
                        }
                    }
                });

                return Ok(StrLocation::new(text_location, text_length));
            }
        }

        Err(MemoryWriterError::UnableToWrite)
    }

    fn data(&self, data: &[u8]) -> MemoryWriterResult<()> {
        if self.in_occupied_state() {
            if let Some((ops, _)) = &self.mem {
                ops.apply(|mem| {
                    mem.extend(data);
                });

                return Ok(());
            }
        }
        Err(MemoryWriterError::UnableToWrite)
    }

    fn end(&self) -> MemoryWriterResult<()> {
        if self.in_occupied_state() {
            if let Some((ops, _)) = &self.mem {
                ops.apply(|mem| {
                    mem.push(Operations::End as u8);
                });

                return Ok(());
            }
        }
        Err(MemoryWriterError::UnableToWrite)
    }

    fn stop(mut self) -> MemoryWriterResult<CompletedInstructions> {
        if self.in_occupied_state() {
            if let Some((ops, _)) = self.mem.take() {
                ops.apply(|mem| {
                    mem.push(Operations::Stop as u8);
                });

                let completed = CompletedInstructions {
                    ops_id: self.ops_id,
                    text_id: self.text_id,
                };

                return Ok(completed);
            }
        }
        Err(MemoryWriterError::UnableToWrite)
    }
}

// -- Operations that can be batch

impl<'a> Batchable<'a> for InternalPointer {
    fn encode<F>(&self, encoder: &'a F, optimized: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        let (value_bytes, tq) = if optimized {
            value_quantitization::qu64(self.into_inner())
        } else {
            (
                self.into_inner().to_le_bytes().to_vec(),
                TypeOptimization::None,
            )
        };

        let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 2);

        data.push(self.to_value_type().into());
        data.push(tq.into());
        data.extend_from_slice(&value_bytes);

        encoder.data(&data)?;
        Ok(())
    }
}

impl<'a> Batchable<'a> for Operations {
    fn encode<F>(&self, encoder: &'a F, _optimized: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        let self_as_number: u8 = (*self).into();
        encoder.data(&[self_as_number])?;

        Ok(())
    }
}

impl<'a> Batchable<'a> for ExternalPointer {
    fn encode<F>(&self, encoder: &'a F, optimized: bool) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        let (value_bytes, tq) = if optimized {
            value_quantitization::qu64(self.into_inner())
        } else {
            (
                self.into_inner().to_le_bytes().to_vec(),
                TypeOptimization::None,
            )
        };

        let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 2);

        data.push(self.to_value_type().into());
        data.push(tq.into());
        data.extend_from_slice(&value_bytes);

        encoder.data(&data)?;

        Ok(())
    }
}

impl Instructions {
    /// [`register_function`] allows you register a function to
    /// a specific `ExternalPointer` that is allocated for it's use.
    ///
    /// This allows you have a specific handle you can always use to
    /// call the specific function always.
    pub fn register_function(
        &self,
        allocated_handle: ExternalPointer,
        body: &str,
    ) -> MemoryWriterResult<()> {
        let value_pointer = self.string(body)?;
        let value_index = value_pointer.index().to_le_bytes();
        let value_length = value_pointer.len().to_le_bytes();

        self.data(&[Operations::MakeFunction as u8])?;
        allocated_handle.encode(self, self.optimized)?;

        let mut data: Vec<u8> = Vec::with_capacity(value_index.len() + value_length.len() + 10);
        data.push(ValueTypes::Text8.into());
        data.extend_from_slice(&value_index);
        data.extend_from_slice(&value_length);

        self.data(&data)?;
        self.end()?;

        Ok(())
    }

    pub fn invoke_no_return_function<'a>(
        &self,
        allocated_handle: ExternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[Operations::InvokeNoReturnFunction as u8])?;

        allocated_handle.encode(self, self.optimized)?;
        self.encode_params(params)?;

        self.end()?;
        Ok(())
    }

    pub fn invoke_returning_function<'a>(
        &self,
        allocated_handle: ExternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[Operations::InvokeReturningFunction as u8])?;

        allocated_handle.encode(self, self.optimized)?;
        self.encode_params(params)?;

        self.end()?;
        Ok(())
    }

    pub fn invoke_callback_function<'a>(
        &self,
        allocated_handle: ExternalPointer,
        callback_handle: InternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[Operations::InvokeCallbackFunction as u8])?;

        callback_handle.encode(self, self.optimized)?;
        allocated_handle.encode(self, self.optimized)?;
        self.encode_params(params)?;

        self.end()?;
        Ok(())
    }

    pub(crate) fn encode_params<'a>(
        &self,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        if let Some(pm) = params {
            pm.encode(self, self.optimized)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test_instructions {
    extern crate std;

    use crate::TypeOptimization;

    use super::*;

    #[test]
    fn can_encode_no_return_function_call_with_optimizations() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, true)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let function_handle = ExternalPointer::from(1);
        let write_result = batch.invoke_no_return_function(
            function_handle,
            Some(&[Params::Int32(10), Params::Int64(20)]),
        );

        assert!(write_result.is_ok());

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert!(completed_strings.is_empty().expect("is_empty"));
        assert!(!completed_ops.is_empty().expect("is_empty"));

        let ops = completed_ops.clone_memory().expect("clone");
        assert_eq!(
            alloc::vec![
                Operations::Begin as u8, // start of block
                Operations::InvokeNoReturnFunction as u8,
                ValueTypes::ExternalReference as u8, // type of value
                TypeOptimization::QuantizedUint64AsU8 as u8,
                // address pointer to function which is a u64, so 8 bytes
                1,
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int32 as u8,
                TypeOptimization::QuantizedInt32AsI8 as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int64 as u8,
                TypeOptimization::QuantizedInt64AsI8 as u8,
                20,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                Operations::End as u8,          // end of current sub-block
                Operations::Stop as u8,         // Stop signal indicating batch is finished
            ],
            ops
        );
    }

    #[test]
    fn can_encode_no_return_function_call() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, false)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let function_handle = ExternalPointer::from(1);
        let write_result = batch.invoke_no_return_function(
            function_handle,
            Some(&[Params::Int32(10), Params::Int64(20)]),
        );

        assert!(write_result.is_ok());

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_strings = slot.text_ref();
        let completed_ops = slot.ops_ref();

        assert!(completed_strings.is_empty().expect("is_empty"));
        assert!(!completed_ops.is_empty().expect("is_empty"));

        let ops = completed_ops.clone_memory().expect("clone");
        assert_eq!(
            alloc::vec![
                Operations::Begin as u8,                  // start of block
                Operations::InvokeNoReturnFunction as u8, // sub-block: a type of instruction
                ValueTypes::ExternalReference as u8,      // type of value
                TypeOptimization::None as u8,
                // address pointer to function which is a u64, so 8 bytes
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                ArgumentOperations::Start as u8, // start of all arguments
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int32 as u8,
                TypeOptimization::None as u8,
                // value of int32 in LittleIndian encoding, so 8 bytes
                10,
                0,
                0,
                0,
                ArgumentOperations::End as u8,   // end of this argument
                ArgumentOperations::Begin as u8, // start of this argument
                ValueTypes::Int64 as u8,
                TypeOptimization::None as u8,
                20,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                ArgumentOperations::End as u8,  // end of this argument
                ArgumentOperations::Stop as u8, // end of all arguments
                Operations::End as u8,          // end of current sub-block
                Operations::Stop as u8,         // Stop signal indicating batch is finished
            ],
            ops
        );
    }

    #[test]
    fn can_encode_function_registeration_and_invoke_function() {
        let mut allocator = MemoryAllocations::new();

        let batch = allocator
            .batch_for(10, 10, false)
            .expect("create new Instructions");

        batch.should_be_occupied().expect("is occupied");

        let function_handle = ExternalPointer::from(1);
        batch
            .register_function(
                function_handle,
                "
            function(message){
                console.log(message);
            }",
            )
            .expect("should encode correctly");

        batch
            .invoke_no_return_function(function_handle, Some(&[Params::Text8("Hello from intro")]))
            .expect("should register call");

        let completed_data = batch.stop().expect("finish writing completion result");
        let slot = allocator.get_slot(completed_data).expect("get memory");

        let completed_ops = slot.ops_ref();
        let completed_strings = slot.text_ref();

        assert!(!completed_strings.is_empty().expect("is_empty"));
        assert!(!completed_ops.is_empty().expect("is_empty"));

        let ops = completed_ops.clone_memory().expect("clone");
        assert_eq!(
            alloc::vec![
                Operations::Begin as u8,
                Operations::MakeFunction as u8,
                15,
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                83,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                Operations::End as u8,
                Operations::InvokeNoReturnFunction as u8,
                15,
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                1,
                2,
                3,
                0,
                83,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                16,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                3,
                4,
                Operations::End as u8,  // end of current sub-block
                Operations::Stop as u8, // Stop signal indicating batch is finished
            ],
            ops
        );
    }
}
