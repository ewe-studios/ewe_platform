use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_compact::{Duration, Instant, SystemTime};
use foundation_context::{Clock, Ctx};
use foundation_ui_runtime::state::{
    Expect, HmacStateCodec, Session, StateConfig, StateKey, StateKeyRing, UnsealedState,
};

pub const WALL: u64 = 1_000_000;
pub const TTL: u64 = 86_400;
pub const MAC: [u8; 32] = [1; 32];
pub const ENC: [u8; 32] = [2; 32];
pub const EXPECT: Expect = Expect {
    component_type: 7,
    format_version: 1,
};

pub struct TestClock {
    start: Instant,
    wall_secs: u64,
    elapsed: AtomicU64,
}

impl TestClock {
    pub fn set_elapsed(&self, secs: u64) {
        self.elapsed.store(secs, Ordering::Relaxed);
    }
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.start + self.elapsed()
    }

    fn elapsed(&self) -> Duration {
        Duration::from_secs(self.elapsed.load(Ordering::Relaxed))
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.wall_secs) + self.elapsed()
    }
}

pub fn test_ctx(wall_secs: u64) -> (Ctx, Arc<TestClock>) {
    let clock = Arc::new(TestClock {
        start: Instant::now(),
        wall_secs,
        elapsed: AtomicU64::new(0),
    });
    (Ctx::with_seed(clock.clone(), 1), clock)
}

pub fn session() -> Session {
    Session::new("sess-A").unwrap()
}

pub fn state() -> UnsealedState {
    UnsealedState {
        format_version: EXPECT.format_version,
        component_type: EXPECT.component_type,
        root_node: 40,
        body: vec![1, 0, 0, 0],
        private: false,
    }
}

pub fn codec() -> HmacStateCodec {
    HmacStateCodec::new(
        StateKeyRing::new(StateKey::new(1, MAC, ENC).unwrap()),
        StateConfig::default(),
    )
}
