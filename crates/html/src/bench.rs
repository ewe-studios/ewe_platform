#![feature(test)]

extern crate syncbox;
extern crate test;

use crate::memory::*;

use self::test::{black_box, Bencher};

#[derive(Clone)]
struct Dummy(usize);

impl Resetable for Dummy {
    fn reset(&mut self) {
        // do nothing
    }
}

#[bench]
fn bench_dummy_usize_area_pool_with_drop(b: &mut Bencher) {
    let limiter = MemoryLimiter::create_shared(calculate_size_for::<Dummy>(None) * 1024 * 1024);
    let mut pool: ArenaPool<Dummy> = ArenaPool::new(limiter, || Dummy(0));

    b.iter(|| {
        black_box({
            let data = pool.allocate().expect("received handle");
            drop(data);
        })
    })
}

#[bench]
fn bench_dummy_usize_area_pool_with_deallocate(b: &mut Bencher) {
    let limiter = MemoryLimiter::create_shared(calculate_size_for::<Dummy>(None) * 1024 * 1024);
    let mut pool: ArenaPool<Dummy> = ArenaPool::new(limiter, || Dummy(0));

    b.iter(|| {
        black_box({
            let data = pool.allocate().expect("received handle");
            data.deallocate();
        })
    })
}

#[derive(Clone)]
struct DummyProfile {
    pub name: String,
    pub address: String,
    pub weddings: Vec<String>,
}

impl Resetable for DummyProfile {
    fn reset(&mut self) {
        self.name.clear();
        self.address.clear();
        self.weddings.clear();
    }
}

#[bench]
fn bench_dummy_profile_area_pool_with_drop(b: &mut Bencher) {
    let limiter =
        MemoryLimiter::create_shared(calculate_size_for::<DummyProfile>(None) * 1024 * 1024);
    let mut pool: ArenaPool<DummyProfile> = ArenaPool::new(limiter, || DummyProfile {
        name: String::from("alex"),
        address: String::from("New York"),
        weddings: vec![String::from("north"), String::from("south")],
    });

    b.iter(|| {
        black_box({
            let data = pool.allocate().expect("received handle");
            data.element().borrow_mut().clone().unwrap().name = String::from("thunder");
            drop(data);
        })
    })
}

#[bench]
fn bench_dummy_profile_area_pool_with_deallocate(b: &mut Bencher) {
    let limiter =
        MemoryLimiter::create_shared(calculate_size_for::<DummyProfile>(None) * 1024 * 1024);
    let mut pool: ArenaPool<DummyProfile> = ArenaPool::new(limiter, || DummyProfile {
        name: String::from("alex"),
        address: String::from("New York"),
        weddings: vec![String::from("north"), String::from("south")],
    });

    b.iter(|| {
        black_box({
            let data = pool.allocate().expect("received handle");
            data.element().borrow_mut().clone().unwrap().name = String::from("thunder");
            data.deallocate();
        })
    })
}
