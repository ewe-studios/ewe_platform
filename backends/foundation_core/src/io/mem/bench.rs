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
fn bench_dummy_usize_area_pool_with_deallocate(b: &mut Bencher) {
    let limiter = MemoryLimiter::create_shared(calculate_size_for::<Dummy>(None) * 8024 * 8024);
    let mut pool: ArenaPool<Dummy> = ArenaPool::new(limiter, || Dummy(0));

    b.iter(|| {
        black_box({
            let data = pool.allocate().expect("received handle");
            pool.deallocate(data);
        })
    })
}

#[derive(Clone)]
struct DummyProfile {
    pub name: String,
    pub address: String,
}

impl Resetable for DummyProfile {
    fn reset(&mut self) {
        self.name.clear();
        self.address.clear();
    }
}

#[bench]
fn bench_dummy_profile_area_pool_with_deallocate(b: &mut Bencher) {
    let limiter =
        MemoryLimiter::create_shared(calculate_size_for::<DummyProfile>(Some(10)) * 8024 * 8024);
    let mut pool: ArenaPool<DummyProfile> = ArenaPool::new(limiter, || DummyProfile {
        name: String::from("alex"),
        address: String::from("New York"),
    });

    b.iter(|| {
        black_box({
            let mut data = pool.allocate().expect("received handle");
            data.name = String::from("thunder");
            pool.deallocate(data);
        })
    })
}

#[derive(Clone)]
struct DummyProfileWithWedding {
    pub name: String,
    pub address: String,
    pub weddings: Vec<String>,
}

impl Resetable for DummyProfileWithWedding {
    fn reset(&mut self) {
        self.name.clear();
        self.address.clear();
        self.weddings.clear();
    }
}

#[bench]
fn bench_dummy_profile_with_wedding_area_pool_with_vec_add(b: &mut Bencher) {
    let limiter = MemoryLimiter::create_shared(
        calculate_size_for::<DummyProfileWithWedding>(None) * 8024 * 8024,
    );
    let mut pool: ArenaPool<DummyProfileWithWedding> =
        ArenaPool::new(limiter, || DummyProfileWithWedding {
            name: String::from("alex"),
            address: String::from("New York"),
            weddings: vec![String::from("north"), String::from("south")],
        });

    b.iter(|| {
        black_box({
            let mut data = pool.allocate().expect("received handle");
            data.name = String::from("thunder");
            data.weddings.push(String::from("west"));
            pool.deallocate(data);
        })
    })
}

#[bench]
fn bench_dummy_profile_with_wedding_area_pool_with_deallocate(b: &mut Bencher) {
    let limiter = MemoryLimiter::create_shared(
        calculate_size_for::<DummyProfileWithWedding>(None) * 8024 * 8024,
    );
    let mut pool: ArenaPool<DummyProfileWithWedding> =
        ArenaPool::new(limiter, || DummyProfileWithWedding {
            name: String::from("alex"),
            address: String::from("New York"),
            weddings: vec![String::from("north"), String::from("south")],
        });

    b.iter(|| {
        black_box({
            let mut data = pool.allocate().expect("received handle");
            data.name = String::from("thunder");
            pool.deallocate(data);
        })
    })
}
