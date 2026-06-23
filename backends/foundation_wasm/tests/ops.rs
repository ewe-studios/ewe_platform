//! WHY: Verify the `Instructions` batch-encoding public API correctly encodes
//! parameters, return-type hints, function registration, and invocation into
//! the custom binary protocol.
//!
//! WHAT: `MemoryAllocations`, `Instructions`, `Params`, `ReturnTypeHints`,
//! `ExternalPointer`, `Operations`.
//!
//! HOW: Allocate instruction batches through `MemoryAllocations`, encode
//! various param types, return hints, function registrations, and invocations,
//! then assert the resulting byte sequences match expectations.

use foundation_wasm::{
    ArgumentOperations, BatchEncodable, ExternalPointer, MemoryAllocations, Operations,
    ParamTypeId, Params, ReturnHintMarker, ReturnIds, ReturnTypeHints, ReturnTypeId, ThreeState,
    ThreeStateId, TypeOptimization, TypedSlice,
};

// ---------------------------------------------------------------------------
// params_tests
// ---------------------------------------------------------------------------

#[test]
fn can_encode_type_hint_markers() {
    let mut allocator = MemoryAllocations::new();

    let batch = allocator
        .batch_for(10, 10, true)
        .expect("create new Instructions");

    batch.should_be_occupied().expect("is occupied");

    assert!(batch.encode_return_hints(ReturnTypeHints::None).is_ok());
    assert!(batch
        .encode_return_hints(ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int32)))
        .is_ok());
    assert!(batch
        .encode_return_hints(ReturnTypeHints::One(ThreeState::Two(
            ReturnTypeId::Float32,
            ReturnTypeId::Float64
        )))
        .is_ok());
    assert!(batch
        .encode_return_hints(ReturnTypeHints::One(ThreeState::Three(
            ReturnTypeId::MemorySlice,
            ReturnTypeId::ExternalReference,
            ReturnTypeId::InternalReference,
        )))
        .is_ok());
    assert!(batch
        .encode_return_hints(ReturnTypeHints::Multi(vec![
            ThreeState::One(ReturnTypeId::MemorySlice),
            ThreeState::Two(ReturnTypeId::MemorySlice, ReturnTypeId::ExternalReference),
            ThreeState::Three(
                ReturnTypeId::MemorySlice,
                ReturnTypeId::ExternalReference,
                ReturnTypeId::Bool
            ),
        ]))
        .is_ok());

    batch.end().expect("end instruction");

    let completed_data = batch.stop().expect("finish writing completion result");
    let slot = allocator.get_slot(completed_data).expect("get memory");

    let completed_strings = slot.text_ref();
    let completed_ops = slot.ops_ref();

    assert_eq!(
        vec![
            0, // Begin signal indicating start of batch
            ReturnHintMarker::Start as u8,
            ReturnIds::None as u8,
            ReturnHintMarker::Stop as u8,
            ReturnHintMarker::Start as u8,
            ReturnIds::One as u8,
            ThreeStateId::One as u8,
            ReturnTypeId::Int32 as u8,
            ReturnHintMarker::Stop as u8,
            ReturnHintMarker::Start as u8,
            ReturnIds::One as u8,
            ThreeStateId::Two as u8,
            ReturnTypeId::Float32 as u8,
            ReturnTypeId::Float64 as u8,
            ReturnHintMarker::Stop as u8,
            ReturnHintMarker::Start as u8,
            ReturnIds::One as u8,
            ThreeStateId::Three as u8,
            ReturnTypeId::MemorySlice as u8,
            ReturnTypeId::ExternalReference as u8,
            ReturnTypeId::InternalReference as u8,
            ReturnHintMarker::Stop as u8,
            ReturnHintMarker::Start as u8,
            ReturnIds::Multi as u8,
            ThreeStateId::One as u8,
            ReturnTypeId::MemorySlice as u8,
            ThreeStateId::Two as u8,
            ReturnTypeId::MemorySlice as u8,
            ReturnTypeId::ExternalReference as u8,
            ThreeStateId::Three as u8,
            ReturnTypeId::MemorySlice as u8,
            ReturnTypeId::ExternalReference as u8,
            ReturnTypeId::Bool as u8,
            ReturnHintMarker::Stop as u8,
            254, // end of the sub-block of instruction
            255  // Stop signal indicating batch is finished
        ],
        completed_ops.clone_memory().expect("clone"),
    );

    assert!(completed_strings.is_empty().expect("returns state"));
}

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
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Undefined as u8,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Null as u8,
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

    let write_result = batch.encode_params(Some(&[Params::Float32(10.2), Params::Float64(10.2)]));

    assert!(write_result.is_ok());

    batch.end().expect("end instruction");
    let completed_data = batch.stop().expect("finish writing completion result");
    let slot = allocator.get_slot(completed_data).expect("get memory");

    let completed_strings = slot.text_ref();
    let completed_ops = slot.ops_ref();

    assert_eq!(
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Float32 as u8,
            51,
            51,
            35,
            65,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Float64 as u8,
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
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Bool as u8,
            1,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Bool as u8,
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
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Uint8 as u8,
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Uint16 as u8,
            TypeOptimization::QuantizedUint16AsU8 as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Uint32 as u8,
            TypeOptimization::QuantizedUint32AsU8 as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Uint64 as u8,
            TypeOptimization::QuantizedUint64AsU8 as u8,
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Uint128 as u8,
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::TypedArraySlice as u8,
        TypedSlice::Uint8 as u8, // type of slice
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::CachedText as u8,
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
        vec![
            0,                               // Begin signal indicating start of batch
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int8 as u8,
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int16 as u8,
            TypeOptimization::QuantizedInt16AsI8 as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int32 as u8,
            TypeOptimization::QuantizedInt32AsI8 as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int64 as u8,
            TypeOptimization::QuantizedInt64AsI8 as u8,
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int128 as u8,
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Text8 as u8,
        TypeOptimization::QuantizedUint64AsU8 as u8,
        0,
        TypeOptimization::QuantizedUint64AsU8 as u8,
        4,
        ArgumentOperations::End as u8,   // end of this argument
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Text16 as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        4,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Float64ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Float32ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Int8ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Int16ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Int32ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Int64ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Uint8ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Uint16ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Uint32ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::Uint64ArrayBuffer as u8,
        TypeOptimization::None as u8,
    ];

    let encoded_end = vec![
        TypeOptimization::QuantizedUint64AsU8 as u8,
        2,
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::ExternalReference as u8,
        TypeOptimization::QuantizedUint64AsU8 as u8,
        0,
    ];

    let encoded_end = vec![
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
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

    let encoded_start = vec![
        0,                               // Begin signal indicating start of batch
        ArgumentOperations::Start as u8, // start of all arguments
        ArgumentOperations::Begin as u8, // start of this argument
        ParamTypeId::InternalReference as u8,
        TypeOptimization::QuantizedUint64AsU8 as u8,
        0,
    ];

    let encoded_end = vec![
        ArgumentOperations::End as u8,  // end of this argument
        ArgumentOperations::Stop as u8, // end of all arguments
        254,                            // end of the sub-block of instruction
        255,                            // Stop signal indicating batch is finished
    ];

    let mut encoded = Vec::new();
    encoded.extend(encoded_start);
    encoded.extend(encoded_end);

    assert_eq!(encoded, completed_ops.clone_memory().expect("clone"),);

    assert!(completed_strings.is_empty().expect("returns state"));
}

// ---------------------------------------------------------------------------
// test_instructions
// ---------------------------------------------------------------------------

#[test]
fn can_encode_no_return_function_call_with_optimizations() {
    let mut allocator = MemoryAllocations::new();

    let batch = allocator
        .batch_for(10, 10, true)
        .expect("create new Instructions");

    batch.should_be_occupied().expect("is occupied");

    let function_handle = ExternalPointer::from(1);
    let write_result = batch.invoke(
        function_handle,
        Some(&[Params::Int32(10), Params::Int64(20)]),
        ReturnTypeHints::None,
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
        vec![
            Operations::Begin as u8, // start of block
            Operations::Invoke as u8,
            ParamTypeId::ExternalReference as u8, // type of value
            TypeOptimization::QuantizedUint64AsU8 as u8,
            // address pointer to function which is a u64, so 8 bytes
            1,
            ReturnHintMarker::Start as u8,
            ReturnIds::None as u8,
            ReturnHintMarker::Stop as u8,
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int32 as u8,
            TypeOptimization::QuantizedInt32AsI8 as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int64 as u8,
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
    let write_result = batch.invoke(
        function_handle,
        Some(&[Params::Int32(10), Params::Int64(20)]),
        ReturnTypeHints::None,
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
        vec![
            Operations::Begin as u8,              // start of block
            Operations::Invoke as u8,             // sub-block: a type of instruction
            ParamTypeId::ExternalReference as u8, // type of value
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
            ReturnHintMarker::Start as u8,
            ReturnIds::None as u8,
            ReturnHintMarker::Stop as u8,
            ArgumentOperations::Start as u8, // start of all arguments
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int32 as u8,
            TypeOptimization::None as u8,
            // value of int32 in LittleIndian encoding, so 8 bytes
            10,
            0,
            0,
            0,
            ArgumentOperations::End as u8,   // end of this argument
            ArgumentOperations::Begin as u8, // start of this argument
            ParamTypeId::Int64 as u8,
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
        .invoke(
            function_handle,
            Some(&[Params::Text8("Hello from intro")]),
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::None)),
        )
        .expect("should register call");

    let completed_data = batch.stop().expect("finish writing completion result");
    let slot = allocator.get_slot(completed_data).expect("get memory");

    let completed_ops = slot.ops_ref();
    let completed_strings = slot.text_ref();

    assert!(!completed_strings.is_empty().expect("is_empty"));
    assert!(!completed_ops.is_empty().expect("is_empty"));

    let ops = completed_ops.clone_memory().expect("clone");
    assert_eq!(
        vec![
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
            Operations::Invoke as u8,
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
            ReturnHintMarker::Start as u8,
            ReturnIds::One as u8,
            ThreeStateId::One as u8,
            ReturnTypeId::None as u8,
            ReturnHintMarker::Stop as u8,
            ArgumentOperations::Start as u8, // start of all arguments
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
            ArgumentOperations::End as u8,  // end of this argument
            ArgumentOperations::Stop as u8, // end of all arguments
            Operations::End as u8,          // end of current sub-block
            Operations::Stop as u8,         // Stop signal indicating batch is finished
        ],
        ops
    );
}
