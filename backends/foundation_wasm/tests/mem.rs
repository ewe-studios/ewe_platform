//! WHY: Verify the `MemoryAllocations` public API for allocating, getting,
//! disposing, using, and clearing memory slots.
//!
//! WHAT: `MemoryAllocations`, `MemoryAllocation`.
//!
//! HOW: Exercise the allocator through its public interface and assert
//! index/generation values, data content, and error conditions.

use foundation_wasm::MemoryAllocations;

#[test]
fn can_allocator_memory() {
    let mut allocator = MemoryAllocations::new();

    let mem1 = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, mem1.index());
    assert_eq!(0, mem1.generation());
    assert_eq!(0, mem1.as_u64());
}

#[test]
fn can_get_allocator_id() {
    let mut allocator = MemoryAllocations::new();

    let mem1 = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, mem1.index());
    assert_eq!(0, mem1.generation());
    assert_eq!(0, mem1.as_u64());

    _ = allocator.get(mem1).expect("should find allocation");
}

#[test]
fn can_dispose_of_an_allocation() {
    let mut allocator = MemoryAllocations::new();

    let mem1 = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, mem1.index());
    assert_eq!(0, mem1.generation());
    assert_eq!(0, mem1.as_u64());

    allocator
        .deallocate(mem1)
        .expect("should dispose allocation");

    assert!(
        allocator.get(mem1).is_err(),
        "should fail to get allocation"
    );
}

#[test]
fn can_use_allocator() {
    let mut allocator = MemoryAllocations::new();

    let mem1 = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, mem1.index());

    let mem2 = allocator.allocate(30).expect("should allocate memory");
    assert_eq!(1, mem2.index());
}

#[test]
fn can_use_allocated_memory() {
    let mut allocator = MemoryAllocations::new();

    let id = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, id.index());

    let memory_slot = allocator.get(id).expect("should be able to find memory id");
    memory_slot.reset_to(0);

    memory_slot.apply(|memo| {
        memo.push(10);
        memo.push(20);
        memo.push(30);
    });

    let content = memory_slot.clone_memory().expect("should clone valid data");
    assert_eq!(vec![10, 20, 30], content);
}

#[test]
fn can_clear_allocated_memory() {
    let mut allocator = MemoryAllocations::new();

    let id = allocator.allocate(20).expect("should allocate memory");
    assert_eq!(0, id.index());

    let memory_slot = allocator.get(id).expect("should be able to find memory id");
    memory_slot.reset_to(0);
    memory_slot.apply(|memo| {
        memo.push(10);
        memo.push(20);
        memo.push(30);
    });

    let content = memory_slot.clone_memory().expect("should clone valid data");
    assert_eq!(vec![10, 20, 30], content);

    memory_slot.clear().expect("clear memory");

    assert!(memory_slot
        .is_empty()
        .expect("should return is_empty state"));
}
