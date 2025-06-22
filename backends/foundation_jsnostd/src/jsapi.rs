#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::string::String;
use alloc::vec::Vec;
use foundation_nostd::{raw_parts::RawParts, spin::Mutex};

use crate::{
    BinaryReadError, BinaryReaderResult, CompletedInstructions, ExternalPointer, FromBinary,
    GroupReturnHintMarker, Instructions, InternalCallback, InternalPointer,
    InternalReferenceRegistry, JSEncoding, MemoryAllocation, MemoryAllocationError,
    MemoryAllocations, MemoryId, Params, ReturnTypeHints, ReturnTypeId, ReturnTypes,
    ReturnValueError, ReturnValueMarker, ReturnValues, Returns, ToBinary, MOVE_ONE_BYTE,
    MOVE_SIXTEEN_BYTES, MOVE_SIXTY_FOUR_BYTES, MOVE_THIRTY_TWO_BYTES,
};

static INTERNAL_CALLBACKS: Mutex<InternalReferenceRegistry> = InternalReferenceRegistry::create();

static ALLOCATIONS: Mutex<MemoryAllocations> = Mutex::new(MemoryAllocations::new());

struct ReturnValueParserIter<'a> {
    hint: ReturnTypeHints,
    item_index: usize,
    index: usize,
    src: &'a [u8],
}

impl<'a> ReturnValueParserIter<'a> {
    fn new(hint: ReturnTypeHints, src: &'a [u8]) -> Self {
        Self {
            src,
            hint,
            index: 0,
            item_index: 0,
        }
    }
}

// -- Parsing

impl ReturnValueParserIter<'_> {
    fn parse_next(&mut self) -> Option<BinaryReaderResult<ReturnValues>> {
        let bin = &self.src;
        let mut index = self.index;

        if self.index >= self.src.len() {
            return None;
        }

        let return_id: ReturnTypeId = bin[index].into();

        // move by 1 byte
        index += MOVE_ONE_BYTE;

        let result = match return_id {
            ReturnTypeId::None => {
                self.index = index;
                Ok(ReturnValues::None)
            }
            ReturnTypeId::ErrorCode => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = u16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::ErrorCode(item))
            }
            ReturnTypeId::Bool => {
                let value = if bin[index] == 1 {
                    ReturnValues::Bool(true)
                } else {
                    ReturnValues::Bool(false)
                };

                index += MOVE_ONE_BYTE;

                self.index = index;

                Ok(value)
            }
            ReturnTypeId::Uint8 => {
                let item = u8::from_le(bin[index]);
                index += MOVE_ONE_BYTE;

                self.index = index;
                Ok(ReturnValues::Uint8(item))
            }
            ReturnTypeId::Uint16 => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = u16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint16(item))
            }
            ReturnTypeId::Uint32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = u32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint32(item))
            }
            ReturnTypeId::Uint64 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Uint64(item))
            }
            ReturnTypeId::Uint128 => {
                let msb_end = index + MOVE_SIXTY_FOUR_BYTES;
                let msb_portion = &bin[index..msb_end];
                let mut msb_section: [u8; 8] = Default::default();
                msb_section.copy_from_slice(msb_portion);

                let lsb_end = msb_end + MOVE_SIXTY_FOUR_BYTES;
                let lsb_portion = &bin[index..lsb_end];
                let mut lsb_section: [u8; 8] = Default::default();
                lsb_section.copy_from_slice(lsb_portion);

                let value_msb = u64::from_le_bytes(msb_section);
                let value_lsb = u64::from_le_bytes(lsb_section);

                let mut value: u128 = (value_msb as u128) << 64;
                value |= value_lsb as u128;

                self.index = lsb_end;

                Ok(ReturnValues::Uint128(value))
            }
            ReturnTypeId::Int8 => {
                let item = i8::from_le(bin[index] as i8);
                index += MOVE_ONE_BYTE;

                self.index = index;

                Ok(ReturnValues::Int8(item))
            }
            ReturnTypeId::Int16 => {
                let end = index + MOVE_SIXTEEN_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 2] = Default::default();
                section.copy_from_slice(portion);

                let item = i16::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int16(item))
            }
            ReturnTypeId::Int32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = i32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int32(item))
            }
            ReturnTypeId::Int64 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = i64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Int64(item))
            }
            ReturnTypeId::Int128 => {
                let msb_end = index + MOVE_SIXTY_FOUR_BYTES;
                let msb_portion = &bin[index..msb_end];
                let mut msb_section: [u8; 8] = Default::default();
                msb_section.copy_from_slice(msb_portion);

                let lsb_end = msb_end + MOVE_SIXTY_FOUR_BYTES;
                let lsb_portion = &bin[index..lsb_end];
                let mut lsb_section: [u8; 8] = Default::default();
                lsb_section.copy_from_slice(lsb_portion);

                let value_msb = i64::from_le_bytes(msb_section);
                let value_lsb = i64::from_le_bytes(lsb_section);

                let mut value: i128 = (value_msb as i128) << 64;
                value |= value_lsb as i128;

                index = lsb_end;

                self.index = index;

                Ok(ReturnValues::Int128(value))
            }
            ReturnTypeId::Float32 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 4] = Default::default();
                section.copy_from_slice(portion);

                let item = f32::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Float32(item))
            }
            ReturnTypeId::Float64 => {
                let end = index + MOVE_THIRTY_TWO_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = f64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Float64(item))
            }
            ReturnTypeId::MemorySlice => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(item);

                self.index = end;

                Ok(ReturnValues::MemorySlice(mem_id))
            }
            ReturnTypeId::Object => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::Object(item.into()))
            }
            ReturnTypeId::DOMObject => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::DOMObject(item.into()))
            }
            ReturnTypeId::ExternalReference => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::ExternalReference(item.into()))
            }
            ReturnTypeId::InternalReference => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];
                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let item = u64::from_le_bytes(section);

                self.index = end;

                Ok(ReturnValues::InternalReference(item.into()))
            }
            ReturnTypeId::Uint8ArrayBuffer => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec = memory.take();
                if memory_vec.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                self.index = end;

                Ok(ReturnValues::Uint8Array(memory_vec.unwrap()))
            }
            ReturnTypeId::Uint16ArrayBuffer => {
                const TOTAL_U8_IN_U18: usize = 2;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_U18 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is two u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U18;
                let mut arr_content: Vec<u16> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U18;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U18] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u16::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint16Array(arr_content))
            }
            ReturnTypeId::Uint32ArrayBuffer => {
                const TOTAL_U8_IN_U32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u32 array converted to u8
                if memory_size % TOTAL_U8_IN_U32 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u32 send as u8 should have even lengths, because u32 in u8 is four u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U32;
                let mut arr_content: Vec<u32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint32Array(arr_content))
            }
            ReturnTypeId::Uint64ArrayBuffer => {
                const TOTAL_U8_IN_U64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u64 array converted to u8
                if memory_size % TOTAL_U8_IN_U64 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u64 send as u8 should have even lengths, because u64 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U64;
                let mut arr_content: Vec<u64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(u64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Uint64Array(arr_content))
            }
            ReturnTypeId::Int8ArrayBuffer => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let mut arr_content: Vec<i8> = Vec::with_capacity(memory_vec.len());

                for value in memory_vec {
                    arr_content.push(i8::from_le(value as i8))
                }

                self.index = end;

                Ok(ReturnValues::Int8Array(arr_content))
            }
            ReturnTypeId::Int16ArrayBuffer => {
                const TOTAL_U8_IN_U16: usize = 2;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_U16 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is two u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U16;
                let mut arr_content: Vec<i16> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U16;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U16] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i16::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int16Array(arr_content))
            }
            ReturnTypeId::Int32ArrayBuffer => {
                const TOTAL_U8_IN_U32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_U32 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is four u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U32;
                let mut arr_content: Vec<i32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int32Array(arr_content))
            }
            ReturnTypeId::Int64ArrayBuffer => {
                const TOTAL_U8_IN_U64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_U64 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_U64;
                let mut arr_content: Vec<i64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_U64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_U64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(i64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Int64Array(arr_content))
            }
            ReturnTypeId::Float32ArrayBuffer => {
                const TOTAL_U8_IN_F32: usize = 4;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_F32 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is four's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_F32;
                let mut arr_content: Vec<f32> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_F32;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_F32] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(f32::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Float32Array(arr_content))
            }
            ReturnTypeId::Float64ArrayBuffer => {
                const TOTAL_U8_IN_F64: usize = 8;

                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();
                let memory_size = memory_vec.len();

                // if the mode of 2 (size in bytes/u8) is not zero then
                // then its an invalid u16 array converted to u8
                if memory_size % TOTAL_U8_IN_F64 != 0 {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                            "Vec<u8> of u16 send as u8 should have even lengths, because u16 in u8 is eight's u8",
                        ))));
                }

                let arr_size = memory_size / TOTAL_U8_IN_F64;
                let mut arr_content: Vec<f64> = Vec::with_capacity(arr_size);

                let mut move_index = 0;
                while move_index < arr_size {
                    let portion_end = move_index + TOTAL_U8_IN_F64;
                    let portion = &memory_vec[move_index..portion_end];
                    let mut arr: [u8; TOTAL_U8_IN_F64] = Default::default();
                    arr.copy_from_slice(portion);
                    arr_content.push(f64::from_le_bytes(arr));
                    move_index = portion_end;
                }

                self.index = end;

                Ok(ReturnValues::Float64Array(arr_content))
            }
            ReturnTypeId::Text8 => {
                let end = index + MOVE_SIXTY_FOUR_BYTES;
                let portion = &bin[index..end];

                let mut section: [u8; 8] = Default::default();
                section.copy_from_slice(portion);

                let alloc_id = u64::from_le_bytes(section);
                let mem_id = MemoryId::from_u64(alloc_id);

                let memory_result = ALLOCATIONS.lock().get(mem_id);
                if let Err(err) = memory_result {
                    return Some(Err(err.into()));
                }
                let mut memory = memory_result.unwrap();
                let memory_vec_container = memory.take();
                if memory_vec_container.is_none() {
                    return Some(Err(BinaryReadError::MemoryError(String::from(
                        "No Vec<u8> not found, big problem",
                    ))));
                }

                if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                    return Some(Err(err.into()));
                }

                let memory_vec = memory_vec_container.unwrap();

                let value = match String::from_utf8(memory_vec) {
                    Ok(content) => ReturnValues::Text8(content),
                    Err(_) => {
                        return Some(Err(BinaryReadError::ExpectedStringInCode(
                            ReturnTypeId::Text8 as u8,
                        )));
                    }
                };

                self.index = end;

                Ok(value)
            }
        };

        Some(result)
    }
}

// -- As an iterator

impl Iterator for ReturnValueParserIter<'_> {
    type Item = BinaryReaderResult<ReturnValues>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parse_next()? {
            Ok(item) => {
                let item_value_type_id = item.to_return_value_type();

                match self.hint.clone() {
                    ReturnTypeHints::One(return_type_id) => {
                        if item_value_type_id != return_type_id {
                            return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                return_type_id,
                                item_value_type_id,
                            )));
                        }
                    }
                    ReturnTypeHints::Multi(return_type_ids) => {
                        let next_type_id = return_type_ids[self.item_index];

                        if item_value_type_id != next_type_id {
                            return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                next_type_id,
                                item_value_type_id,
                            )));
                        }
                        self.item_index += 1;
                    }
                    ReturnTypeHints::List(return_type_id) => {
                        if item_value_type_id != return_type_id {
                            return Some(Err(BinaryReadError::NotMatchingTypeHint(
                                return_type_id,
                                item_value_type_id,
                            )));
                        }
                    }
                    ReturnTypeHints::None => unreachable!("Should never be called"),
                };

                Some(Ok(item))
            }
            Err(err) => Some(Err(err)),
        }
    }
}

impl FromBinary for ReturnTypeHints {
    type T = Vec<ReturnValues>;

    fn from_binary(self, input_bin: &[u8]) -> BinaryReaderResult<Self::T> {
        if input_bin[0] != (ReturnValueMarker::Begin as u8) {
            return Err(BinaryReadError::WrongStarterCode(input_bin[0]));
        }

        let length = input_bin.len();
        if input_bin[length - 1] != (ReturnValueMarker::End as u8) {
            return Err(BinaryReadError::WrongEndingCode(input_bin[length - 1]));
        }

        let value_start = 1;
        let value_end = length - 1;

        let bin = &input_bin[value_start..value_end];

        let mut decoded = Vec::with_capacity(1);
        let parser = ReturnValueParserIter::new(self, bin);
        for parsed_item in parser {
            match parsed_item {
                Ok(item) => {
                    decoded.push(item);
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(decoded)
    }
}

/// [`GroupReturnTypeHints`] represents conversion of
/// underlying type which is a grouping of return values
/// from the host where it represent a batch of return values
/// that should be generated/materialized.
#[derive(Default)]
pub struct GroupReturnTypeHints;

impl FromBinary for GroupReturnTypeHints {
    type T = Vec<Returns>;

    fn from_binary(self, input_bin: &[u8]) -> BinaryReaderResult<Self::T> {
        if input_bin[0] != (GroupReturnHintMarker::Start as u8) {
            return Err(BinaryReadError::WrongStarterCode(input_bin[0]));
        }

        let length = input_bin.len();
        if input_bin[length - 1] != (GroupReturnHintMarker::Stop as u8) {
            return Err(BinaryReadError::WrongEndingCode(input_bin[length - 1]));
        }

        let value_start = 1;
        let value_end = length - 1;

        let bin = &input_bin[value_start..value_end];
        // panic!("Received binary info: {:?}", bin);

        let mut decoded = Vec::with_capacity(2);

        let mut index = 0;

        while index < bin.len() {
            let reply_type: ReturnTypes = u8::from_le(bin[index]).into();
            index += MOVE_ONE_BYTE;

            let return_hint: ReturnTypeHints = match reply_type {
                ReturnTypes::One => {
                    let value_type: ReturnTypeId = u8::from_le(bin[index]).into();
                    index += MOVE_ONE_BYTE;

                    ReturnTypeHints::One(value_type)
                }
                ReturnTypes::List => {
                    let value_type: ReturnTypeId = u8::from_le(bin[index]).into();
                    index += MOVE_ONE_BYTE;

                    ReturnTypeHints::List(value_type)
                }
                ReturnTypes::Multi => {
                    let item_count_start = index;
                    let item_count_end = index + MOVE_SIXTEEN_BYTES;
                    index = item_count_end;

                    let item_count_slice = &bin[item_count_start..item_count_end];
                    let mut item_count_arr: [u8; 2] = Default::default();
                    item_count_arr.copy_from_slice(item_count_slice);

                    let item_count = u16::from_le_bytes(item_count_arr);

                    let mut value_types = Vec::with_capacity(item_count as usize);
                    for _ in 1..item_count {
                        let item_value: ReturnTypeId = u8::from_le(bin[index]).into();
                        value_types.push(item_value);
                        index += MOVE_ONE_BYTE;
                    }

                    ReturnTypeHints::Multi(value_types)
                }
                ReturnTypes::None => unreachable!("should never get type of value from host"),
            };

            let end = index + MOVE_SIXTY_FOUR_BYTES;
            let portion = &bin[index..end];

            index = end;

            let mut section: [u8; 8] = Default::default();
            section.copy_from_slice(portion);

            let alloc_id = u64::from_le_bytes(section);
            let mem_id = MemoryId::from_u64(alloc_id);

            let memory_result = ALLOCATIONS.lock().get(mem_id);
            if let Err(err) = memory_result {
                return Err(err.into());
            }
            let memory = memory_result.unwrap();

            match memory.into_with(|mem| return_hint.clone().from_binary(mem.as_ref())) {
                Some(item_result) => {
                    let mut item = item_result?;

                    let value_item = match return_hint {
                        ReturnTypeHints::One(_) => {
                            if item.len() != 1 {
                                return Err(BinaryReadError::MemoryError(String::from(
                                    "more than one item for ReturnTypes::One(_)",
                                )));
                            }
                            Returns::One(item.pop().expect("valid index"))
                        }
                        ReturnTypeHints::List(_) => Returns::List(item),
                        ReturnTypeHints::Multi(_) => Returns::Multi(item),
                        ReturnTypeHints::None => {
                            unreachable!("should never get return type from group")
                        }
                    };

                    decoded.push(value_item);
                }
                None => {
                    return Err(BinaryReadError::MemoryError(String::from(
                        "expected a valid returned value not None",
                    )));
                }
            };

            if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                return Err(err.into());
            }
        }

        Ok(decoded)
    }
}

/// [`internal_api`] are internal methods, structs, and surfaces that provide core functionalities
/// that we support or that allows making or preparing data to be sent-out or sent-across the API.
///
/// You should never place a function in here that needs to be exposed to the host or host function
/// we want to define but instead use the [`exposed_runtime`] or [`host_runtime`] modules.
pub mod internal_api {
    use super::*;

    // -- Instruction methods

    pub fn create_instructions(text_size: u64, operation_size: u64) -> Instructions {
        ALLOCATIONS
            .lock()
            .batch_for(text_size, operation_size, true)
            .expect("should create allocated memory slot")
    }

    pub fn get_memory(memory_id: MemoryId) -> MemoryAllocation {
        ALLOCATIONS
            .lock()
            .get(memory_id)
            .expect("should fetch related memory allocation")
    }

    // -- callback methods

    pub fn register_internal_callback<F>(f: F) -> InternalPointer
    where
        F: InternalCallback + 'static,
    {
        INTERNAL_CALLBACKS.lock().add(f)
    }

    pub fn unregister_internal_callback(addr: InternalPointer) {
        INTERNAL_CALLBACKS
            .lock()
            .remove(addr)
            .expect("should be registered");
    }

    pub fn run_internal_callbacks(_addr: InternalPointer, _value: MemoryId) {
        // let callback = INTERNAL_CALLBACKS
        //     .lock()
        //     .get(addr)
        //     .expect("should be registered");
        // callback.receive(value);
        todo!()
    }

    // -- extract methods

    pub fn extract_vec_from_memory(allocation_id: u64) -> Vec<u8> {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.clone_memory().expect("should clone memory")
    }

    pub fn extract_string_from_memory(allocation_id: u64) -> String {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.string_from_memory()
            .expect("should convert into String")
    }
}

/// [`exposed_runtime`] are the underlying functions we expose to the host from
/// the system. These are functions the runtime exposes to the host to be able
/// to make calls into the system or triggering processes.
pub mod exposed_runtime {
    use super::*;

    #[no_mangle]
    pub extern "C" fn create_allocation(size: u64) -> u64 {
        let mem_id = ALLOCATIONS
            .lock()
            .allocate(size)
            .expect("should create requested allocation");
        mem_id.as_u64()
    }

    #[no_mangle]
    pub extern "C" fn allocation_start_pointer(mem_id: u64) -> *const u8 {
        let allocations = ALLOCATIONS.lock();
        let memory = allocations
            .get(mem_id.into())
            .expect("Allocation should be initialized");
        memory
            .get_pointer()
            .expect("should be able to get valid pointer")
    }

    #[no_mangle]
    pub extern "C" fn allocation_length(allocation_id: u64) -> u64 {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.len().expect("should return allocation length")
    }

    #[no_mangle]
    pub extern "C" fn clear_allocation(allocation_id: u64) {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.clear().expect("should clear memory");
    }

    #[no_mangle]
    pub extern "C" fn unregister_callback(addr: u64) {
        internal_api::unregister_internal_callback(addr.into());
    }

    #[no_mangle]
    pub extern "C" fn invoke_callback(internal_pointer: u64, allocation_id: u64) {
        internal_api::run_internal_callbacks(
            InternalPointer::pointer(internal_pointer),
            MemoryId::from_u64(allocation_id),
        );
    }
}

/// [`host_runtime`] is the expected interface which the JS/Host
/// must provide for use with wrapper functions that make it simple
/// and easier to interact with.
#[allow(unused)]
pub mod host_runtime {
    use super::*;

    pub const DOM_SELF: ExternalPointer = ExternalPointer::pointer(0);
    pub const DOM_THIS: ExternalPointer = ExternalPointer::pointer(1);
    pub const DOM_WINDOW: ExternalPointer = ExternalPointer::pointer(2);
    pub const DOM_DOCUMENT: ExternalPointer = ExternalPointer::pointer(3);
    pub const DOM_BODY: ExternalPointer = ExternalPointer::pointer(4);

    // -- Functions (Invocation & Registration)
    pub mod web {
        use crate::{CachedText, MemoryAllocationResult, MemoryReaderError};

        use super::*;

        #[link(wasm_import_module = "abi")]
        extern "C" {

            /// [`host_batch_apply`] takes a location in memory that has a batch of operations
            /// which match the [`crate::Operations`] outlined in the batching API the
            /// runtime supports, allowing us amortize the cost of doing bulk processing on
            /// the wasm and host boundaries.
            ///
            /// This batch apply returns no value and no underlying result will
            /// be written to memory.
            pub fn host_batch_apply(
                operation_pointer: u64,
                operation_length: u64,
                text_pointer: u64,
                text_length: u64,
            );

            /// [`host_batch_returning_apply`] takes a location in memory that has a batch of operations
            /// which match the [`crate::Operations`] outlined in the batching API the
            /// runtime supports, allowing us amortize the cost of doing bulk processing on
            /// the wasm and host boundaries.
            pub fn host_batch_returning_apply(
                operation_pointer: u64,
                operation_length: u64,
                text_pointer: u64,
                text_length: u64,
            ) -> u64;

            /// [`function_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for a function and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn function_allocate_external_pointer() -> u64;

            /// [`object_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for an object and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn object_allocate_external_pointer() -> u64;

            /// [`dom_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for a dom node and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn dom_allocate_external_pointer() -> u64;

            /// [`host_cache_string`] provides a way to cache dynamic utf8 strings that
            /// will be interned into a map of a u64 key representing the string, this allows
            /// us to pay the cost of conversion once for these types of strings
            /// whilst limiting the overall cost since only a reference is ever passed around.
            pub fn host_cache_string(start: u64, len: u64, encoding: u8) -> u64;

            // [`host_unregister_function`] provides a means to unregister a target function
            // from the WASM - Host runtime boundary.
            pub fn host_unregister_function(handle: u64);

            // registers a function via it's provided start and length
            // indicative of where the function body can be found
            // as utf-8 or utf-18 encoded byte (based on third argument)
            // from the start pointer in memory to the specified
            // length to be registered in the shared
            // function registry.
            pub fn host_register_function(start: u64, len: u64, encoding: u8) -> u64;

            /// [`host_invoke_function_as_i64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i64`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i64;

            /// [`host_invoke_function_as_i32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i32`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i32;

            /// [`host_invoke_function_as_i16`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i16`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i16(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i16;

            /// [`host_invoke_function_as_i8`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i8(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i8;

            /// [`host_invoke_function_as_u64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u64`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u64;

            /// [`host_invoke_function_as_u32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u32`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u32;

            /// [`host_invoke_function_as_u16`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u16`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u16(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u16;

            /// [`host_invoke_function_as_u8`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u8(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u8;

            /// [`host_invoke_function_as_bool`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_bool(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u8;

            /// [`host_invoke_function_as_f64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a f32 as result. We do this to optimize any need to allocate memory
            /// explicitly for the result though wasm will do this for us since the type is basically
            /// supported.
            pub fn host_invoke_function_as_f64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> f64;

            /// [`host_invoke_function_as_f32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a f32 as result. We do this to optimize any need to allocate memory
            /// explicitly for the result though wasm will do this for us since the type is basically
            /// supported.
            pub fn host_invoke_function_as_f32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> f32;

            /// [`host_invoke_callback_function`] invokes a Host function across the WASM/RUST ABI
            /// which must respond with result via a callback registered on the WASM side and
            /// referenced by a internal [`u64`] number.
            ///
            /// The host will issue a request to pass relevant response to the callback via
            /// the [`super::exposed_runtime::invoke_callback`] with the memory location
            /// containing the result.
            pub fn host_invoke_callback_function(
                handler: u64,
                callback_handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
                returns_start: *const u8,
                returns_length: u64,
            );

            /// [`host_invoke_function`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the memory location for both outgoing
            // parameters and also return type expectation which
            // then returns the allocation_id (as f64) that can
            // be used to get the related allocation vector
            // from the global allocations.
            pub fn host_invoke_function(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
                returns_start: *const u8,
                returns_length: u64,
            ) -> u64;
        }

        /// [`allocate_dom_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for a dom node.
        pub fn allocate_dom_reference() -> ExternalPointer {
            unsafe { ExternalPointer::pointer(host_runtime::web::dom_allocate_external_pointer()) }
        }

        /// [`allocate_function_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for a function.
        pub fn allocate_function_reference() -> ExternalPointer {
            unsafe {
                ExternalPointer::pointer(host_runtime::web::function_allocate_external_pointer())
            }
        }

        /// [`allocate_object_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for an object.
        pub fn allocate_object_reference() -> ExternalPointer {
            unsafe {
                ExternalPointer::pointer(host_runtime::web::object_allocate_external_pointer())
            }
        }

        /// [`batch`] sends a [`CompletedInstructions`] batch over to the host runtime
        /// to be applied and expects no responses/returned values to be provided.
        pub fn batch(instruction: CompletedInstructions) {
            let operations_memory = internal_api::get_memory(instruction.ops_id);
            let text_memory = internal_api::get_memory(instruction.text_id);

            let (ops_pointer, ops_length) =
                operations_memory.as_address().expect("get ops address");
            let (text_pointer, text_length) = text_memory.as_address().expect("get text address");

            unsafe {
                host_runtime::web::host_batch_apply(
                    ops_pointer as u64,
                    ops_length,
                    text_pointer as u64,
                    text_length,
                );
            };
        }

        /// [`batch_response`] sends a [`CompletedInstructions`] batch over to the host runtime
        /// and expects the returned values of these execution to be returned to it.
        ///
        /// Note: Instructions that call callbacks will generally return None or
        /// [`ReturnTypeHints::None`] as their returned values.
        ///
        /// We ensure to keep the order of instructions to returned values through
        /// group returns.
        pub fn batch_response(
            instruction: CompletedInstructions,
        ) -> BinaryReaderResult<Vec<Returns>> {
            let operations_memory = internal_api::get_memory(instruction.ops_id);
            let text_memory = internal_api::get_memory(instruction.text_id);

            let (ops_pointer, ops_length) =
                operations_memory.as_address().expect("get ops address");
            let (text_pointer, text_length) = text_memory.as_address().expect("get text address");

            let return_id = unsafe {
                host_runtime::web::host_batch_returning_apply(
                    ops_pointer as u64,
                    ops_length,
                    text_pointer as u64,
                    text_length,
                )
            };

            let mem_id = MemoryId::from_u64(return_id);
            let memory_result = ALLOCATIONS.lock().get(mem_id);
            if let Err(err) = memory_result {
                return Err(err.into());
            }
            let memory = memory_result.unwrap();

            let result = match memory.into_with(|item| {
                let group: GroupReturnTypeHints = Default::default();
                group.from_binary(item.as_ref())
            }) {
                Some(value) => value,
                None => Err(BinaryReadError::MemoryError(String::from(
                    "unable to read return values",
                ))),
            };

            if let Err(err) = ALLOCATIONS.lock().deallocate(mem_id) {
                return Err(err.into());
            }

            result
        }

        /// [`cache_text`] provides a way to have the host runtime cache an expense
        /// text for you which allows you perform multiple re-use of the same text.
        ///
        /// Remember that UTF-8 top UTF-16 is an expensive operation and when you have
        /// a string you plan to re-use over and over then there is benefit in simply
        /// caching these string but also understand we are using more memory both in
        /// the rust (guest wasm) side and the host side and you really only benefit from
        /// that overhead when that string really is very much reused alot.
        ///
        /// Additionally, what if you end up sending the same UTF-16 text over to the host
        /// often, there is also benefit in just sending it once, caching it and referencing
        /// it by the cache id.
        ///
        /// Be aware this is immediate and instead of keeping the memory slot owned on the
        /// wasm side, we defer to the host runtime to always maintain a reference id for us
        /// and must guarantee that id will hold for the lifetime of the program.
        pub fn cache_text(code: &str) -> CachedText {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                CachedText::pointer(host_runtime::web::host_cache_string(
                    start as u64,
                    len as u64,
                    JSEncoding::UTF8.into(),
                ))
            }
        }

        /// [`register_function`] calls the underlying [`js_abi`] registration
        /// function to register a host code that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function(code: &str) -> HostFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                HostFunction {
                    handler: host_runtime::web::host_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF8.into(),
                    ),
                }
            }
        }

        /// [`register_function_utf16`] calls the underlying [`js_abi`] registration
        /// function to register a host code already encoded
        /// as UTF16 by the borrowed slice of u16 that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function_utf16(code: &[u16]) -> HostFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                HostFunction {
                    handler: host_runtime::web::host_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF16.into(),
                    ), // precision loss here
                }
            }
        }

        /// [`invoke_as_f64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`f64`].
        pub fn invoke_as_f64(handler: u64, params: &[Params]) -> f64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_f64(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_f32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`f32`].
        pub fn invoke_as_f32(handler: u64, params: &[Params]) -> f32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_f32(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_i64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i64`].
        pub fn invoke_as_i64(handler: u64, params: &[Params]) -> i64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_i64(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_i32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i32`].
        pub fn invoke_as_i32(handler: u64, params: &[Params]) -> i32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_i32(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_i16`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i16`].
        pub fn invoke_as_i16(handler: u64, params: &[Params]) -> i16 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_i16(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_i8`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i8`].
        pub fn invoke_as_i8(handler: u64, params: &[Params]) -> i8 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_i8(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_u64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u64`].
        pub fn invoke_as_u64(handler: u64, params: &[Params]) -> u64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_u64(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_u32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u32`].
        pub fn invoke_as_u32(handler: u64, params: &[Params]) -> u32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_u32(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_u16`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u16`].
        pub fn invoke_as_u16(handler: u64, params: &[Params]) -> u16 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_u16(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_u8`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u8`].
        pub fn invoke_as_u8(handler: u64, params: &[Params]) -> u8 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_u8(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                )
            }
        }

        /// [`invoke_as_bool`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u8`]
        /// which represents the state to be returned (1- true, 0 - false).
        pub fn invoke_as_bool(handler: u64, params: &[Params]) -> bool {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function_as_bool(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                ) == 1
            }
        }

        /// [`invoke_as_callback`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side which will when executed
        /// must have it's returned result communicated via a callback handle provided.
        ///
        /// This provides support for cases where Promises or async function or function
        /// whoes means of result communication is callback only.
        ///
        /// A [`u64`]
        ///
        /// When called we expect the return of a [`u64`] which actually points to a
        /// [`MemoryId`] registered in [`ALLOCATIONS`] which can be retrieved to get
        /// the actual result and is expected to be a binary of [`ReturnValues`]
        /// which match the return hints [`ReturnTypeHints`].
        pub fn invoke_as_callback(
            handler: u64,
            callback: InternalPointer,
            params: &[Params],
            returns: ReturnTypeHints,
        ) {
            let return_hints_bytes = returns.to_binary();
            let return_raw = RawParts::from_vec(return_hints_bytes);

            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_callback_function(
                    handler,
                    callback.into_inner(),
                    param_raw.ptr,
                    param_raw.length,
                    return_raw.ptr,
                    return_raw.length,
                );
            };
        }

        /// [`invoke_for_replies`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
        /// which represents the allocation id of the contents which are to be encoded in
        /// the new Reply binary format implemented via [`FromBinary`] for [`ReturnTypeHints`].
        pub fn invoke_for_replies(
            handler: u64,
            params: &[Params],
            returns: ReturnTypeHints,
        ) -> MemoryAllocationResult<Vec<ReturnValues>> {
            let memory_id =
                MemoryId::from_u64(host_runtime::web::invoke(handler, params, returns.clone()));

            let memory = internal_api::get_memory(memory_id);
            let result_container =
                memory.into_with(|mem| returns.clone().from_binary(mem.as_ref()));

            if result_container.is_none() {
                return Err(MemoryAllocationError::FailedAllocationReading(memory_id));
            }

            let result = result_container.unwrap();
            if let Err(err) = result {
                return Err(MemoryReaderError::NotValidReplyBinary(err).into());
            }

            let replies = result.unwrap();

            match &returns {
                ReturnTypeHints::One(_) => {
                    if replies.len() != 1 {
                        return Err(MemoryReaderError::ReturnValueError(
                            crate::ReturnValueError::ExpectedOne(replies),
                        )
                        .into());
                    }

                    Ok(replies)
                }
                ReturnTypeHints::List(_) => Ok(replies),
                ReturnTypeHints::Multi(_) => Ok(replies),
                ReturnTypeHints::None => unreachable!(
                    "a reply is always expected you cant use this in situation where None is given"
                ),
            }
        }

        /// [`invoke_for_str`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
        /// which represents the allocation id of the contents.
        pub fn invoke_for_str(handler: u64, params: &[Params]) -> MemoryAllocationResult<String> {
            match host_runtime::web::invoke_for_replies(
                handler,
                params,
                ReturnTypeHints::One(ReturnTypeId::Text8),
            ) {
                Ok(mut values) => match values.pop().unwrap() {
                    ReturnValues::Text8(content) => Ok(content),
                    _ => Err(ReturnValueError::UnexpectedReturnType.into()),
                },
                Err(err) => Err(err),
            }
        }

        /// [`invoke`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u64`] which actually points to a
        /// [`MemoryId`] registered in [`ALLOCATIONS`] which can be retrieved to get
        /// the actual result and is expected to be a binary of [`ReturnValues`]
        /// which match the return hints [`ReturnTypeHints`].
        pub fn invoke(handler: u64, params: &[Params], returns: ReturnTypeHints) -> u64 {
            let return_hints_bytes = returns.to_binary();
            let return_raw = RawParts::from_vec(return_hints_bytes);

            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                host_runtime::web::host_invoke_function(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                    return_raw.ptr,
                    return_raw.length,
                )
            }
        }

        // --- Browser / WASM ABI

        #[derive(Copy, Clone)]
        pub struct HostFunction {
            pub handler: u64,
        }

        #[allow(clippy::cast_precision_loss)]
        impl HostFunction {
            /// [`invoke`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then receives the set of parameters
            /// supplied to be invoked with.
            ///
            /// The `js_abi` will handle necessary conversion and execution of the function
            /// with the passed arguments.
            pub fn invoke(&self, params: &[Params], returns: ReturnTypeHints) -> MemoryId {
                MemoryId::from_u64(host_runtime::web::invoke(self.handler, params, returns))
            }

            /// [`invoke_for_memory`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
            /// which represents the allocation id of the contents.
            pub fn invoke_no_return(&self, params: &[Params]) {
                _ = host_runtime::web::invoke(self.handler, params, ReturnTypeHints::None);
            }

            /// [`invoke_for_bool`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a bool indicating the
            /// result.
            ///
            /// Internal true is when the returned number is >= 1 and False if 0.
            pub fn invoke_for_bool(&self, params: &[Params]) -> bool {
                host_runtime::web::invoke_as_bool(self.handler, params)
            }

            /// [`invoke_for_i8`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u8.
            pub fn invoke_for_i8(&self, params: &[Params]) -> i8 {
                unsafe { host_runtime::web::invoke_as_i8(self.handler, params) }
            }

            /// [`invoke_for_i16`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u16.
            pub fn invoke_for_i16(&self, params: &[Params]) -> i16 {
                host_runtime::web::invoke_as_i16(self.handler, params)
            }

            /// [`invoke_for_i32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u32.
            pub fn invoke_for_i32(&self, params: &[Params]) -> i32 {
                host_runtime::web::invoke_as_i32(self.handler, params)
            }

            /// [`invoke_for_i64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_i64(&self, params: &[Params]) -> i64 {
                host_runtime::web::invoke_as_i64(self.handler, params)
            }

            /// [`invoke_for_u8`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u8.
            pub fn invoke_for_u8(&self, params: &[Params]) -> u8 {
                unsafe { host_runtime::web::invoke_as_u8(self.handler, params) }
            }

            /// [`invoke_for_u16`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u16.
            pub fn invoke_for_u16(&self, params: &[Params]) -> u16 {
                host_runtime::web::invoke_as_u16(self.handler, params)
            }

            /// [`invoke_for_u32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u32.
            pub fn invoke_for_u32(&self, params: &[Params]) -> u32 {
                host_runtime::web::invoke_as_u32(self.handler, params)
            }

            /// [`invoke_for_u64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_u64(&self, params: &[Params]) -> u64 {
                host_runtime::web::invoke_as_u64(self.handler, params)
            }

            /// [`invoke_for_float64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a f64.
            pub fn invoke_for_f64(&self, params: &[Params]) -> f64 {
                host_runtime::web::invoke_as_f64(self.handler, params)
            }

            /// [`invoke_for_float32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a f32.
            pub fn invoke_for_f32(&self, params: &[Params]) -> f32 {
                host_runtime::web::invoke_as_f32(self.handler, params)
            }

            /// [`invoke_for_str`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
            /// which represents the allocation id of the contents.
            pub fn invoke_for_str(&self, params: &[Params]) -> MemoryAllocationResult<String> {
                host_runtime::web::invoke_for_str(self.handler, params)
            }

            /// [`invoke_for_dom`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer {
                ExternalPointer::pointer(host_runtime::web::invoke(
                    self.handler,
                    params,
                    ReturnTypeHints::One(ReturnTypeId::DOMObject),
                ))
            }

            /// [`invoke_for_object`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the object via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_object(&self, params: &[Params]) -> ExternalPointer {
                ExternalPointer::pointer(host_runtime::web::invoke(
                    self.handler,
                    params,
                    ReturnTypeHints::One(ReturnTypeId::Object),
                ))
            }

            /// [`unregister_function`] calls the JS ABI on the host to de-register
            /// the target function.
            pub fn unregister(self) {
                unsafe { host_runtime::web::host_unregister_function(self.handler) }
            }
        }
    }
}
