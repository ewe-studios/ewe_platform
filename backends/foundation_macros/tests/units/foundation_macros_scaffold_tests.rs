// Scaffold-macro fixtures: methods exist to be forwarded by the generated
// scaffolding (some are intentionally unused or self-less in spirit) — the
// tests assert the GENERATED delegations compile and dispatch correctly.
#![allow(dead_code, clippy::unused_self)]

// Tests for scaffold macros: #[scaffoldable], #[derive(Scaffold)], #[scaffold_impl]
//
// Since proc macros run at compile time, we test them by defining types
// in the test file and verifying the generated methods compile and work.

use foundation_macros::{scaffold_impl, scaffoldable, Scaffold};
use foundation_nostd::macros::scaffold;

// ── Test types ──

struct TableInner {
    name: String,
    age: u32,
}

#[scaffoldable]
impl TableInner {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_age(&self) -> u32 {
        self.age
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn private_helper(&self) -> usize {
        42
    }
}

// ── Automatic pathway tests ──

#[derive(Scaffold)]
pub struct PublicTable {
    #[scaffold(field)]
    inner: TableInner,
}

#[test]
fn test_derive_scaffold_basic() {
    let table = PublicTable {
        inner: TableInner {
            name: "hello".to_string(),
            age: 42,
        },
    };

    assert_eq!(table.get_name(), "hello");
    assert_eq!(table.get_age(), 42);
}

#[test]
fn test_derive_scaffold_mut_method() {
    let mut table = PublicTable {
        inner: TableInner {
            name: "old".to_string(),
            age: 1,
        },
    };

    table.set_name("new".to_string());
    assert_eq!(table.get_name(), "new");
}

#[test]
fn test_scaffoldable_skips_private() {
    let table = PublicTable {
        inner: TableInner {
            name: "test".to_string(),
            age: 10,
        },
    };

    // private_helper should NOT be forwarded — this would not compile if it were:
    // table.private_helper(); // <- does not exist
    // Verify only the expected public methods exist
    let _ = table.get_name();
    let _ = table.get_age();
}

// ── Mutex preset test ──

use std::sync::Mutex;

struct Counter {
    value: u64,
}

#[scaffoldable]
impl Counter {
    pub fn get(&self) -> u64 {
        self.value
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }
}

#[derive(Scaffold)]
pub struct ThreadSafeCounter {
    #[scaffold(field)]
    inner: Mutex<Counter>,
}

#[test]
fn test_derive_scaffold_mutex() {
    let counter = ThreadSafeCounter {
        inner: Mutex::new(Counter { value: 10 }),
    };

    assert_eq!(counter.get(), 10);
}

// ── Multiple fields test ──

struct AuthService;

#[scaffoldable]
impl AuthService {
    pub fn authenticate(&self, _token: &str) -> bool {
        true
    }
}

struct DbService;

#[scaffoldable]
impl DbService {
    pub fn query(&self, _sql: &str) -> Vec<String> {
        vec![]
    }
}

#[derive(Scaffold)]
pub struct Gateway {
    #[scaffold(field)]
    auth: AuthService,
    #[scaffold(field)]
    db: DbService,
}

#[test]
fn test_derive_scaffold_multiple_fields() {
    let gateway = Gateway {
        auth: AuthService,
        db: DbService,
    };

    assert!(gateway.authenticate("token"));
    assert!(gateway.query("SELECT 1").is_empty());
}

// ── Manual pathway tests ──

trait SimpleService {
    fn get(&self) -> u32;
    fn set(&mut self, val: u32);
    fn status(&self) -> String;
}

struct ServiceWrapper {
    inner: ServiceInner,
}

struct ServiceInner {
    value: u32,
}

impl ServiceInner {
    pub fn get(&self) -> u32 {
        self.value
    }

    pub fn set(&mut self, val: u32) {
        self.value = val;
    }
}

#[scaffold_impl(via = "self.inner")]
impl SimpleService for ServiceWrapper {
    fn get(&self) -> u32 {
        scaffold!()
    }

    fn set(&mut self, val: u32) {
        scaffold!()
    }

    fn status(&self) -> String {
        format!("value={}", self.inner.value)
    }
}

#[test]
fn test_scaffold_impl_basic() {
    let mut svc = ServiceWrapper {
        inner: ServiceInner { value: 0 },
    };
    assert_eq!(svc.get(), 0);
    svc.set(42);
    assert_eq!(svc.get(), 42);
}

#[test]
fn test_scaffold_impl_override() {
    let svc = ServiceWrapper {
        inner: ServiceInner { value: 99 },
    };
    assert_eq!(svc.status(), "value=99");
}

// ── Arc<T> preset test ──

use std::sync::Arc;

struct ReadOnlyData {
    label: String,
    count: u32,
}

#[scaffoldable]
impl ReadOnlyData {
    pub fn label(&self) -> String {
        self.label.clone()
    }

    pub fn count(&self) -> u32 {
        self.count
    }
}

#[derive(Scaffold)]
pub struct ArcWrapper {
    #[scaffold(field)]
    inner: Arc<ReadOnlyData>,
}

#[test]
fn test_derive_scaffold_arc() {
    let wrapper = ArcWrapper {
        inner: Arc::new(ReadOnlyData {
            label: "arc_test".to_string(),
            count: 77,
        }),
    };

    assert_eq!(wrapper.label(), "arc_test");
    assert_eq!(wrapper.count(), 77);
}

// ── Explicit #[scaffold_call] override test ──

struct Ledger {
    balance: i64,
}

#[scaffoldable]
impl Ledger {
    pub fn balance(&self) -> i64 {
        self.balance
    }
}

#[derive(Scaffold)]
pub struct SafeLedger {
    #[scaffold(field)]
    #[scaffold_call(call = { self.data.lock().unwrap() })]
    data: Arc<Mutex<Ledger>>,
}

#[test]
fn test_derive_scaffold_explicit_call() {
    let ledger = SafeLedger {
        data: Arc::new(Mutex::new(Ledger { balance: 100 })),
    };

    assert_eq!(ledger.balance(), 100);
}

// ── Manual pathway: selective #[scaffold_method] delegation ──

trait MultiSource {
    fn auth_check(&self, token: &str) -> bool;
    fn db_query(&self, sql: &str) -> Vec<String>;
    fn health(&self) -> String;
}

struct SelectiveGateway {
    auth: AuthInner,
    db: DbInner,
}

struct AuthInner;
impl AuthInner {
    pub fn auth_check(&self, _token: &str) -> bool {
        true
    }
}

struct DbInner;
impl DbInner {
    pub fn db_query(&self, sql: &str) -> Vec<String> {
        vec![sql.to_string()]
    }
}

#[scaffold_impl]
impl MultiSource for SelectiveGateway {
    #[scaffold_method(via = "self.auth")]
    fn auth_check(&self, token: &str) -> bool {
        scaffold!()
    }

    #[scaffold_method(via = "self.db")]
    fn db_query(&self, sql: &str) -> Vec<String> {
        scaffold!()
    }

    fn health(&self) -> String {
        "ok".to_string()
    }
}

#[test]
fn test_scaffold_method_selective() {
    let gw = SelectiveGateway {
        auth: AuthInner,
        db: DbInner,
    };

    assert!(gw.auth_check("token123"));
    assert_eq!(gw.db_query("SELECT 1"), vec!["SELECT 1".to_string()]);
    assert_eq!(gw.health(), "ok");
}

#[test]
fn test_mixed_via_sources() {
    let gw = SelectiveGateway {
        auth: AuthInner,
        db: DbInner,
    };

    assert!(gw.auth_check("abc"));
    assert_eq!(gw.db_query("SELECT *"), vec!["SELECT *".to_string()]);
}

// ── Manual pathway: block-level #[scaffold_call] with Mutex lock ──

trait LockedOps {
    fn read_val(&self) -> u64;
    fn write_val(&mut self, v: u64);
}

struct LockedStore {
    data: Mutex<Counter>,
}

#[scaffold_impl]
#[scaffold_call(call = { self.data.lock().unwrap() })]
impl LockedOps for LockedStore {
    fn read_val(&self) -> u64 {
        scaffold!()
    }

    fn write_val(&mut self, v: u64) {
        scaffold!()
    }
}

impl Counter {
    pub fn read_val(&self) -> u64 {
        self.value
    }

    pub fn write_val(&mut self, v: u64) {
        self.value = v;
    }
}

#[test]
fn test_scaffold_call_block_mutex() {
    let mut store = LockedStore {
        data: Mutex::new(Counter { value: 42 }),
    };

    assert_eq!(store.read_val(), 42);
    store.write_val(99);
    assert_eq!(store.read_val(), 99);
}

// ── Manual pathway: multiline call block ──

trait TrackedOps {
    fn fetch_count(&self) -> u64;
}

struct TrackedStore {
    pool: Mutex<Counter>,
}

#[scaffold_impl]
impl TrackedOps for TrackedStore {
    #[scaffold_call(call = {
        let guard = self.pool.lock().unwrap();
        guard
    })]
    fn fetch_count(&self) -> u64 {
        scaffold!()
    }
}

impl Counter {
    pub fn fetch_count(&self) -> u64 {
        self.value
    }
}

#[test]
fn test_scaffold_call_block_multiline() {
    let store = TrackedStore {
        pool: Mutex::new(Counter { value: 7 }),
    };

    assert_eq!(store.fetch_count(), 7);
}

// ── Manual pathway: block-level via + per-method call override ──

trait MixedAccess {
    fn fast_read(&self) -> u64;
    fn slow_write(&self, val: u64);
}

struct MixedWrapper {
    inner: Counter,
    locked: Mutex<Counter>,
}

#[scaffold_impl]
impl MixedAccess for MixedWrapper {
    #[scaffold_method(via = "self.inner")]
    fn fast_read(&self) -> u64 {
        scaffold!()
    }

    #[scaffold_call(call = { self.locked.lock().unwrap() })]
    fn slow_write(&self, val: u64) {
        scaffold!()
    }
}

impl Counter {
    pub fn fast_read(&self) -> u64 {
        self.value
    }

    pub fn slow_write(&mut self, val: u64) {
        self.value = val;
    }
}

#[test]
fn test_scaffold_call_per_method_override() {
    let wrapper = MixedWrapper {
        inner: Counter { value: 10 },
        locked: Mutex::new(Counter { value: 20 }),
    };

    assert_eq!(wrapper.fast_read(), 10);
    wrapper.slow_write(50);
    assert_eq!(wrapper.locked.lock().unwrap().value, 50);
}

// ── scaffold!() fallback test ──

#[test]
#[should_panic(expected = "scaffold!() was not processed")]
fn test_scaffold_fallback_panics() {
    // Without #[scaffold_impl], scaffold!() should panic
    scaffold!();
}

// ── Trybuild error case tests ──

#[test]
fn trybuild_scaffold_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/fail/scaffold_no_delegation_target.rs");
    t.compile_fail("tests/trybuild/fail/scaffold_no_scaffoldable.rs");
}
