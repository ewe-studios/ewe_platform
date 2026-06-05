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

// ── scaffold!() fallback test ──

#[test]
#[should_panic(expected = "scaffold!() was not processed")]
fn test_scaffold_fallback_panics() {
    // Without #[scaffold_impl], scaffold!() should panic
    scaffold!();
}
