use foundation_nostd::raw_parts::RawParts;
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
