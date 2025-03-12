#![allow(clippy::no_mangle_with_rust_abi)]
#![allow(clippy::missing_doc_code_examples)]
#![allow(clippy::missing_panics_doc)]

use foundation_nostd::spin::Mutex;

static ALLOCATIONS: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());

#[no_mangle]
pub fn create_allocation(size: usize) -> usize {
    let mut buf = Vec::with_capacity(size as usize);
    buf.resize(size, 0);
    let mut allocations = ALLOCATIONS.lock();
    let index = allocations.len();
    allocations.push(Some(buf));
    index
}

#[no_mangle]
pub fn allocation_start_pointer(allocation_id: usize) -> *const u8 {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations
        .get(allocation_id)
        .expect("Allocation should be initialized");
    let vec = allocation.as_ref().unwrap();
    vec.as_ptr()
}

#[no_mangle]
pub fn allocation_length(allocation_id: usize) -> f64 {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations
        .get(allocation_id)
        .expect("Allocation should be initialized");
    let vec = allocation.as_ref().unwrap();
    vec.len() as f64
}

#[no_mangle]
pub fn clear_allocation(allocation_id: usize) {
    let mut allocations = ALLOCATIONS.lock();
    allocations[allocation_id] = None;
}

pub fn extract_vec_from_memory(allocation_id: usize) -> Vec<u8> {
    let mut allocations = ALLOCATIONS.lock();
    let allocation = allocations.get(allocation_id).expect("should be allocated");
    let vec = allocation.as_ref().unwrap();
    vec.clone()
}

pub fn extract_string_from_memory(allocation_id: usize) -> String {
    let mut allocations = ALLOCATIONS.lock();
    let allocation = allocations.get(allocation_id).expect("should be allocated");
    let vec = allocation.as_ref().unwrap();
    String::from_utf8(vec.clone()).unwrap()
}
