#![allow(dead_code)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::{
    ArgumentOperations, ExternalPointer, InternalPointer, MemoryAllocation, MemoryAllocationResult,
    MemoryAllocations, MemoryId, MemoryWriterError, MemoryWriterResult,
};

use super::{Operations, Params, StrLocation, ValueTypes};

/// [`Batchable`] defines a infallible type which can be
/// encoded into a [`BatchEncodable`] implementing type
/// usually a [`Batch`].
pub trait Batchable<'a> {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable;
}

const DEFAULT_ALLOCATION_SIZE: usize = 10;
static ARGUMENT_ENDER: &[u8] = &[ArgumentOperations::Stop as u8];
static ARGUMENT_STARTER: &[u8] = &[ArgumentOperations::Start as u8];

impl<'a> Batchable<'a> for Vec<Params<'a>> {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        encoder.data(ARGUMENT_STARTER)?;
        for param in self.iter() {
            param.encode(encoder)?;
        }
        encoder.data(ARGUMENT_ENDER)?;
        Ok(())
    }
}

impl<'a> Batchable<'a> for &'a [Params<'a>] {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        encoder.data(ARGUMENT_STARTER)?;
        for param in self.iter() {
            param.encode(encoder)?;
        }
        encoder.data(ARGUMENT_ENDER)?;
        Ok(())
    }
}

impl<'a> Batchable<'a> for Params<'a> {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        match self {
            Params::Undefined => {
                let data: Vec<u8> = alloc::vec![
                    ArgumentOperations::Begin.into(),
                    ValueTypes::Undefined.into(),
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
            Params::Int32(value) => {
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
            Params::Int64(value) => {
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
            Params::Uint32(value) => {
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
            Params::Uint64(value) => {
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
                let value_bytes = value.to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
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
                let value_bytes = value.to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);
                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Text8(value) => {
                let value_pointer = encoder.string(value)?;
                let value_index = value_pointer.index().to_le_bytes();
                let value_length = value_pointer.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_index.len() + value_length.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_index);
                data.extend_from_slice(&value_length);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Text16(value) => {
                let value_length = value.len().to_le_bytes();

                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float32Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Float64Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint32Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint64Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int32Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int64Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int8Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Int16Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint8Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::Uint16Array(value) => {
                let value_pointer = value.as_ptr() as usize;
                let value_pointer_bytes = value_pointer.to_le_bytes(); // size of
                let value_length_bytes = value.len().to_le_bytes();

                let mut data: Vec<u8> =
                    Vec::with_capacity(value_pointer_bytes.len() + value_length_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_pointer_bytes);
                data.extend_from_slice(&value_length_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::ExternalReference(value) => {
                let value_bytes = value.into_inner().to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
            Params::InternalReference(value) => {
                let value_bytes = value.into_inner().to_le_bytes();

                let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 3);

                data.push(ArgumentOperations::Begin.into());
                data.push(self.to_value_type().into());
                data.extend_from_slice(&value_bytes);
                data.push(ArgumentOperations::End.into());

                encoder.data(&data)?;
                Ok(())
            }
        }
    }
}

/// [`BatchEncodable`] defines a trait which allows you implement
/// conversion an underlying binary representation of a Batch
/// operation.
pub trait BatchEncodable {
    /// [`string`] encodes the underlying string
    /// returning the string location information which allows
    /// whatever is calling it
    fn string(&self, data: &str) -> MemoryWriterResult<StrLocation>;

    /// [`data`] provides the underlying related data for
    /// the identified operation.
    fn data(&self, data: &[u8]) -> MemoryWriterResult<()>;

    /// [`end`] indicates the batch encoding can be considered finished and
    /// added to the batch list.
    fn end(self);
}

pub struct CompletedInstructions {
    pub ops_id: MemoryId,
    pub text_id: MemoryId,
}

pub struct Instructions<'a> {
    ops_id: MemoryId,
    text_id: MemoryId,
    occupied: Option<Operations>,
    mem: Option<(MemoryAllocation, MemoryAllocation)>,
    consumer: Box<dyn FnOnce(CompletedInstructions) + 'a>,
}

impl MemoryAllocations {
    pub fn batch_for<'a>(
        &mut self,
        text_capacity: u64,
        operations_capacity: u64,
        consumer: impl FnOnce(CompletedInstructions) + 'a,
    ) -> MemoryAllocationResult<Instructions<'a>> {
        let operations_id = self.allocate(operations_capacity)?;
        let operations_buffer = self.get(operations_id.clone())?;

        let text_id = self.allocate(text_capacity)?;
        let text_buffer = self.get(text_id.clone())?;

        Ok(Instructions::new(
            operations_id,
            text_id,
            operations_buffer,
            text_buffer,
            Box::new(consumer),
        ))
    }
}

// -- Implements BatchEncodable

impl BatchEncodable for Instructions<'_> {
    fn string(&self, data: &str) -> MemoryWriterResult<StrLocation> {
        if self.in_occupied_state() {
            if let Some((_, text)) = &self.mem {
                let data_bytes = data.as_bytes();
                let text_location = data_bytes.len() as u64;
                let text_length = data_bytes.len() as u64;

                text.apply(|mem| {
                    mem.extend_from_slice(data_bytes);
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

    fn end(mut self) {
        if let Some((ops, _)) = self.mem.take() {
            ops.apply(|mem| {
                mem.push(Operations::Stop as u8);
            });

            (self.consumer)(CompletedInstructions {
                ops_id: self.ops_id,
                text_id: self.text_id,
            });
        }
    }
}

// -- Operations: checker

impl Instructions<'_> {
    pub fn in_occupied_state(&self) -> bool {
        self.occupied.is_some()
    }

    pub fn in_free_state(&self) -> bool {
        self.occupied.is_none()
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

// -- Constructors

impl<'a> Instructions<'a> {
    pub fn new(
        ops_id: MemoryId,
        text_id: MemoryId,
        ops: MemoryAllocation,
        texts: MemoryAllocation,
        consumer: Box<dyn FnOnce(CompletedInstructions) + 'a>,
    ) -> Self {
        Self {
            ops_id,
            text_id,
            consumer,
            occupied: None,
            mem: Some((ops, texts)),
        }
    }

    /// [`begin`] starts a new operation to be encoded into the Instructions set
    /// if a operation was not properly closed then an error
    /// [`MemoryWriterError::PreviousUnclosedOperation`] is returned.
    pub fn begin(mut self) -> MemoryWriterResult<Self> {
        if self.occupied.is_some() {
            return Err(MemoryWriterError::PreviousUnclosedOperation);
        }

        self.occupied.replace(Operations::Begin);
        if let Some((op, _)) = &self.mem {
            op.apply(|mem| {
                mem.push(Operations::Begin as u8);
            });
        }
        Ok(self)
    }
}

// -- Operations that can be batch

impl<'a> Batchable<'a> for InternalPointer {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        let value_bytes = self.into_inner().to_le_bytes();

        let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 1);
        data.push(self.to_value_type().into());
        data.extend_from_slice(&value_bytes);

        encoder.data(&data)?;
        Ok(())
    }
}

impl<'a> Batchable<'a> for ExternalPointer {
    fn encode<F>(&self, encoder: &'a F) -> MemoryWriterResult<()>
    where
        F: BatchEncodable,
    {
        let value_bytes = self.into_inner().to_le_bytes();

        let mut data: Vec<u8> = Vec::with_capacity(value_bytes.len() + 1);
        data.push(self.to_value_type().into());
        data.extend_from_slice(&value_bytes);

        encoder.data(&data)?;
        Ok(())
    }
}

impl<'a> Instructions<'a> {
    /// [`register_function`] is immediate and will call end which will flush
    /// the batch into the underlying Operations.
    pub fn register_function(
        &self,
        allocated_handle: ExternalPointer,
        body: &str,
    ) -> MemoryWriterResult<()> {
        let value_pointer = self.string(body)?;
        let value_index = value_pointer.index().to_le_bytes();
        let value_length = value_pointer.len().to_le_bytes();

        let mut data: Vec<u8> = Vec::with_capacity(value_index.len() + value_length.len() + 10);

        data.push(Operations::MakeFunction.into());
        allocated_handle.encode(self)?;

        data.push(ValueTypes::Text8.into());
        data.extend_from_slice(&value_index);
        data.extend_from_slice(&value_length);
        data.push(ArgumentOperations::End.into());

        self.data(&data)?;

        Ok(())
    }

    pub fn invoke_no_return_function(
        &self,
        allocated_handle: ExternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[
            Operations::Begin as u8,
            Operations::InvokeNoReturnFunction as u8,
        ])?;

        allocated_handle.encode(self)?;
        if let Some(pm) = params {
            pm.encode(self)?;
        }

        self.data(&[Operations::Stop as u8])?;

        Ok(())
    }

    pub fn invoke_returning_function(
        &self,
        allocated_handle: ExternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[
            Operations::Begin as u8,
            Operations::InvokeReturningFunction as u8,
        ])?;

        allocated_handle.encode(self)?;
        if let Some(pm) = params {
            pm.encode(self)?;
        }

        self.data(&[Operations::Stop as u8])?;

        Ok(())
    }

    pub fn invoke_callback_function(
        &self,
        allocated_handle: ExternalPointer,
        callback_handle: InternalPointer,
        params: Option<&'a [Params<'a>]>,
    ) -> MemoryWriterResult<()> {
        self.data(&[
            Operations::Begin as u8,
            Operations::InvokeCallbackFunction as u8,
        ])?;

        callback_handle.encode(self)?;
        allocated_handle.encode(self)?;
        if let Some(pm) = params {
            pm.encode(self)?;
        }

        self.data(&[Operations::Stop as u8])?;

        Ok(())
    }
}

#[cfg(test)]
mod test_instructions {
    use alloc::rc::Rc;
    use core::cell::RefCell;

    use super::*;

    #[test]
    fn can_encode_params_with_instructions() {
        let mut allocator = MemoryAllocations::new();
        let container: Rc<RefCell<Option<CompletedInstructions>>> = Rc::new(RefCell::new(None));

        let _batch = allocator.batch_for(10, 10, |item| {
            container.borrow_mut().replace(item);
        });

        // batch.
    }
}
