# Rust Coding Standards

## Overview
- **Language**: Rust 1.75+ (stable channel, use latest stable)
- **Edition**: 2021 (latest edition with all modern features)
- **Use Cases**: Systems programming, high-performance backend services, CLI tools, embedded systems, WebAssembly, performance-critical code
- **Official Docs**: https://doc.rust-lang.org/
- **The Rust Book**: https://doc.rust-lang.org/book/
- **Rust By Example**: https://doc.rust-lang.org/rust-by-example/

## Setup and Tools

### Required Tools
- **rustup**: Rust toolchain installer and version manager
- **rustc**: Rust compiler (managed by rustup)
- **cargo**: Build system, package manager, test runner
- **rustfmt**: Official code formatter (nightly for advanced features)
- **clippy**: Lint tool for catching common mistakes and anti-patterns
- **rust-analyzer**: LSP implementation for IDE support
- **cargo-audit**: Security vulnerability scanner
- **cargo-deny**: Dependency license and advisory checker
- **cargo-expand**: Macro expansion tool (debugging)
- **cargo-nextest**: Fast test runner (recommended)
- **cargo-flamegraph**: Profiling tool

### Installation
```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add essential components
rustup component add rustfmt clippy rust-analyzer

# Install additional cargo tools
cargo install cargo-audit cargo-deny cargo-expand cargo-nextest cargo-flamegraph

# Keep toolchain updated
rustup update
```

### Configuration Files

#### **Cargo.toml** - Package Manifest
```toml
[package]
name = "project-name"
version = "0.1.0"
edition = "2021"  # MANDATORY: Use 2021 edition
rust-version = "1.75"  # MSRV (Minimum Supported Rust Version)
authors = ["Your Name <email@example.com>"]
license = "MIT OR Apache-2.0"  # Standard Rust dual license
repository = "https://github.com/user/repo"
documentation = "https://docs.rs/project-name"
readme = "README.md"
keywords = ["keyword1", "keyword2"]
categories = ["category1"]

[dependencies]
# Keep sorted alphabetically
# Use workspace dependencies for monorepos

[dev-dependencies]
# Test and development dependencies

# CRITICAL: Optimize for release builds
[profile.release]
opt-level = 3           # Maximum optimization
lto = "fat"            # Full link-time optimization (slower compile, faster runtime)
codegen-units = 1      # Better optimization, slower compile
strip = true           # Strip symbols from binary
panic = "abort"        # Smaller binary, no unwinding

# IMPORTANT: Fast compilation for debug builds
[profile.dev]
opt-level = 0          # No optimization for fast compile
debug = true           # Full debug info
split-debuginfo = "unpacked"  # Faster on macOS/Linux

# Optimize dependencies even in debug mode (faster debug experience)
[profile.dev.package."*"]
opt-level = 2

# For testing with optimizations
[profile.test]
opt-level = 1

# For benchmarking
[profile.bench]
opt-level = 3
lto = "fat"
codegen-units = 1
```

#### **rustfmt.toml** - Formatter Configuration
```toml
# Use nightly features for best formatting
edition = "2021"
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"
fn_call_width = 80
attr_fn_like_width = 80
struct_lit_width = 40
struct_variant_width = 60
array_width = 80
chain_width = 60
single_line_if_else_max_width = 50
wrap_comments = true
format_code_in_doc_comments = true
normalize_comments = true
normalize_doc_attributes = true
license_template_path = ""
format_strings = true
format_macro_matchers = true
format_macro_bodies = true
hex_literal_case = "Lower"
empty_item_single_line = true
struct_lit_single_line = true
fn_single_line = false
where_single_line = false
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
reorder_imports = true
reorder_modules = true
reorder_impl_items = false
match_block_trailing_comma = true
trailing_comma = "Vertical"
trailing_semicolon = true
use_field_init_shorthand = true
use_try_shorthand = true
```

#### **.clippy.toml** - Clippy Configuration
```toml
# Enforce strict clippy lints
msrv = "1.75"
warn-on-all-wildcard-imports = true
disallowed-methods = [
    { path = "std::option::Option::unwrap", reason = "use ? operator or proper error handling" },
    { path = "std::option::Option::expect", reason = "use ? operator or proper error handling" },
    { path = "std::result::Result::unwrap", reason = "use ? operator or proper error handling" },
    { path = "std::result::Result::expect", reason = "use ? operator or proper error handling" },
    { path = "std::result::Result::unwrap_err", reason = "use proper error handling" },
    { path = "std::panic::panic", reason = "use Result for recoverable errors" },
    { path = "std::unimplemented", reason = "implement the functionality or use todo!()" },
]
disallowed-types = []
cognitive-complexity-threshold = 30
```

#### **rust-toolchain.toml** - Toolchain Specification
```toml
[toolchain]
channel = "stable"
profile = "default"
components = ["rustfmt", "clippy", "rust-analyzer"]
```

#### **.cargo/config.toml** - Cargo Configuration
```toml
[build]
# Use mold/lld for faster linking
# rustflags = ["-C", "link-arg=-fuse-ld=lld"]  # Linux
# rustflags = ["-C", "link-arg=-fuse-ld=mold"]  # Even faster on Linux

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

# Enable unstable features for cargo
[unstable]
# sparse-registry = true  # Faster registry access
```

## Coding Standards

### Naming Conventions (RFC 430)
- **Crate names**: `snake_case` (prefer single word)
- **Modules**: `snake_case`
- **Types**: `UpperCamelCase` (PascalCase)
- **Traits**: `UpperCamelCase`
- **Enum variants**: `UpperCamelCase`
- **Functions**: `snake_case`
- **Methods**: `snake_case`
- **Local variables**: `snake_case`
- **Static variables**: `SCREAMING_SNAKE_CASE`
- **Constant variables**: `SCREAMING_SNAKE_CASE`
- **Type parameters**: Single `UpperCamelCase` letter or short `UpperCamelCase` name
- **Lifetimes**: Short lowercase names starting with `'` (e.g., `'a`, `'buf`, `'input`)

**Examples**:
```rust
// Correct
const MAX_CONNECTIONS: usize = 100;
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct UserAccount { /* ... */ }
enum NetworkState { /* ... */ }
trait Serialize { /* ... */ }

fn calculate_checksum() -> u32 { /* ... */ }
fn send_message(msg: &str) { /* ... */ }

// Type parameters
struct Buffer<T> { /* ... */ }
struct Cache<Key, Value> { /* ... */ }
fn process<T: Serialize>(item: T) { /* ... */ }

// Lifetimes
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { /* ... */ }
fn parse<'input>(data: &'input [u8]) -> Result<&'input str, Error> { /* ... */ }
```

### Module Organization and Structure

#### Project Layout
```
project/
├── Cargo.toml
├── Cargo.lock              # ALWAYS commit this
├── src/
│   ├── lib.rs              # Library crate root
│   ├── main.rs             # Binary crate root (if applicable)
│   ├── bin/                # Additional binaries
│   │   └── tool.rs
│   ├── models/
│   │   ├── mod.rs          # Module root (or models.rs at src/)
│   │   ├── user.rs
│   │   └── post.rs
│   ├── services/
│   │   ├── mod.rs
│   │   └── user_service.rs
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── validation.rs
│   │   └── crypto.rs
│   └── error.rs            # Centralized error types
├── tests/                  # Integration tests
│   ├── common/
│   │   └── mod.rs          # Shared test utilities
│   └── integration_test.rs
├── benches/                # Benchmarks
│   └── benchmarks.rs
└── examples/               # Example usage
    └── basic_usage.rs
```

#### Module Best Practices
```rust
// GOOD: Explicit re-exports for public API
// src/lib.rs
#![warn(missing_docs)]
#![warn(clippy::all)]
#![forbid(unsafe_code)]  // Unless you really need unsafe

//! Top-level crate documentation.
//!
//! Provides functionality for X, Y, and Z.

// Re-export commonly used items
pub use models::{User, Post};
pub use services::UserService;
pub use error::{Error, Result};

// Public modules
pub mod models;
pub mod services;

// Private modules
mod utils;
mod error;

// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{Error, Result};
    pub use crate::models::{User, Post};
    pub use crate::services::UserService;
}
```

```rust
// GOOD: Module privacy and encapsulation
// src/models/user.rs
use crate::error::{Error, Result};

/// User account representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: UserId,
    pub name: String,      // Public field
    email: Email,          // Private, use accessor
    credentials: Credentials,  // Private, never expose
}

impl User {
    /// Creates a new user.
    ///
    /// # Errors
    ///
    /// Returns an error if email is invalid.
    pub fn new(name: String, email: String) -> Result<Self> {
        Ok(Self {
            id: UserId::new(),
            name,
            email: Email::parse(email)?,
            credentials: Credentials::default(),
        })
    }

    /// Returns the user's email address.
    pub fn email(&self) -> &str {
        self.email.as_str()
    }

    /// Returns the user's ID.
    pub fn id(&self) -> UserId {
        self.id
    }
}

// Private helper types
#[derive(Debug, Clone, PartialEq, Eq)]
struct Credentials {
    password_hash: String,
}

impl Default for Credentials {
    fn default() -> Self {
        Self {
            password_hash: String::new(),
        }
    }
}
```

### Documentation Standards

#### Module-Level Documentation
```rust
//! Authentication and authorization module.
//!
//! This module provides functionality for user authentication,
//! session management, and authorization checks.
//!
//! # Examples
//!
//! ```
//! use myapp::auth::{Authenticator, Credentials};
//!
//! let auth = Authenticator::new();
//! let credentials = Credentials::new("user", "pass");
//! let session = auth.login(credentials)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Security
//!
//! All passwords are hashed using Argon2id before storage.
//! Sessions expire after 24 hours of inactivity.
```

#### Item Documentation
```rust
/// Represents a user session.
///
/// Sessions are created upon successful authentication and contain
/// the user's identity and authorization claims.
///
/// # Examples
///
/// ```
/// use myapp::Session;
///
/// let session = Session::new(user_id, claims);
/// assert!(session.is_valid());
/// ```
///
/// # Thread Safety
///
/// `Session` is `Send + Sync` and can be safely shared across threads.
#[derive(Debug, Clone)]
pub struct Session {
    id: SessionId,
    user_id: UserId,
    claims: Vec<Claim>,
    expires_at: Instant,
}

impl Session {
    /// Creates a new session for the given user.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The unique identifier of the authenticated user
    /// * `claims` - Authorization claims for the session
    ///
    /// # Returns
    ///
    /// A new `Session` that expires in 24 hours.
    ///
    /// # Examples
    ///
    /// ```
    /// # use myapp::{Session, UserId, Claim};
    /// let session = Session::new(UserId::new(), vec![]);
    /// ```
    pub fn new(user_id: UserId, claims: Vec<Claim>) -> Self {
        Self {
            id: SessionId::generate(),
            user_id,
            claims,
            expires_at: Instant::now() + Duration::from_secs(86400),
        }
    }

    /// Checks if the session is still valid.
    ///
    /// A session is valid if it has not expired.
    ///
    /// # Returns
    ///
    /// `true` if the session is valid, `false` otherwise.
    pub fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }

    /// Returns the user ID associated with this session.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.user_id
    }
}
```

## Best Practices - Deep Dive

### Error Handling - The Rust Way

#### Use `thiserror` for Libraries, `anyhow` for Applications
```rust
// LIBRARY CODE: Use thiserror for well-typed errors
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("configuration file not found at {path}")]
    NotFound { path: String },

    #[error("invalid configuration: {0}")]
    Invalid(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

// APPLICATION CODE: Use anyhow for convenience
use anyhow::{Context, Result};

fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .context(format!("failed to read config from {}", path))?;

    let config = toml::from_str(&content)
        .context("failed to parse config file")?;

    Ok(config)
}
```

#### NEVER Use `unwrap()` or `expect()` in Production
```rust
// BAD: These will panic and crash your program
let value = option.unwrap();
let value = result.expect("this should never fail");  // Famous last words

// GOOD: Handle errors properly
let value = option.ok_or(Error::NotFound)?;
let value = result.map_err(|e| Error::from(e))?;
let value = match result {
    Ok(v) => v,
    Err(e) => {
        tracing::error!("operation failed: {}", e);
        return Err(Error::from(e));
    }
};

// ACCEPTABLE: In tests, examples, or truly impossible cases
#[cfg(test)]
fn test_something() {
    let value = result.unwrap();  // OK in tests
}

// Only when mathematically impossible
let idx = absolute_value.checked_sub(1).expect("value is always > 0 here");
```

#### Error Context and Chains
```rust
use anyhow::{Context, Result};

fn process_user_data(user_id: UserId) -> Result<Data> {
    let user = fetch_user(user_id)
        .context(format!("failed to fetch user {}", user_id))?;

    let permissions = load_permissions(&user)
        .with_context(|| {
            format!("failed to load permissions for user {}", user.name)
        })?;

    let data = transform_data(user, permissions)
        .context("failed to transform user data")?;

    Ok(data)
}
```

### Type System Mastery

#### Newtype Pattern for Type Safety
```rust
// EXCELLENT: Use newtypes to prevent mixing up similar types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProductId(uuid::Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

// Now this won't compile (type safety!):
fn get_user(id: UserId) -> User { /* ... */ }
let product_id = ProductId::new();
// get_user(product_id);  // Compile error! Can't mix types
```

#### Builder Pattern for Complex Construction
```rust
// GOOD: Use typed builders for complex types
pub struct ServerConfig {
    host: String,
    port: u16,
    max_connections: usize,
    timeout: Duration,
    tls_config: Option<TlsConfig>,
}

impl ServerConfig {
    pub fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ServerConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    max_connections: Option<usize>,
    timeout: Option<Duration>,
    tls_config: Option<TlsConfig>,
}

impl ServerConfigBuilder {
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn max_connections(mut self, n: usize) -> Self {
        self.max_connections = Some(n);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn tls(mut self, config: TlsConfig) -> Self {
        self.tls_config = Some(config);
        self
    }

    pub fn build(self) -> Result<ServerConfig, ConfigError> {
        Ok(ServerConfig {
            host: self.host.ok_or(ConfigError::MissingHost)?,
            port: self.port.unwrap_or(8080),
            max_connections: self.max_connections.unwrap_or(100),
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
            tls_config: self.tls_config,
        })
    }
}

// Usage
let config = ServerConfig::builder()
    .host("localhost")
    .port(3000)
    .max_connections(500)
    .build()?;
```

#### State Machines with Type States
```rust
// EXCELLENT: Use type states to make invalid states unrepresentable
pub struct Connection<State> {
    socket: TcpStream,
    state: PhantomData<State>,
}

pub struct Disconnected;
pub struct Connected;
pub struct Authenticated;

impl Connection<Disconnected> {
    pub fn new(addr: &str) -> Result<Self, Error> {
        Ok(Self {
            socket: TcpStream::connect(addr)?,
            state: PhantomData,
        })
    }

    pub fn connect(self) -> Result<Connection<Connected>, Error> {
        // Perform connection handshake
        Ok(Connection {
            socket: self.socket,
            state: PhantomData,
        })
    }
}

impl Connection<Connected> {
    pub fn authenticate(
        self,
        credentials: &Credentials,
    ) -> Result<Connection<Authenticated>, Error> {
        // Perform authentication
        Ok(Connection {
            socket: self.socket,
            state: PhantomData,
        })
    }
}

impl Connection<Authenticated> {
    pub fn send_message(&mut self, msg: &Message) -> Result<(), Error> {
        // Only authenticated connections can send messages
        Ok(())
    }
}

// Usage - compile-time state checking!
let conn = Connection::<Disconnected>::new("localhost:8080")?;
// conn.send_message(&msg)?;  // Compile error! Not authenticated
let conn = conn.connect()?;
// conn.send_message(&msg)?;  // Still compile error!
let mut conn = conn.authenticate(&creds)?;
conn.send_message(&msg)?;  // OK!
```

### Ownership, Borrowing, and Lifetimes - Advanced

#### Smart Pointer Usage
```rust
use std::rc::Rc;
use std::sync::Arc;
use std::cell::{RefCell, Cell};

// Use Arc for thread-safe shared ownership
let data = Arc::new(expensive_data);
let data_clone = Arc::clone(&data);
thread::spawn(move || {
    process(data_clone);
});

// Use Rc for single-threaded shared ownership
let config = Rc::new(Config::load()?);
let service1 = Service::new(Rc::clone(&config));
let service2 = Service::new(Rc::clone(&config));

// Use RefCell for interior mutability (single-threaded)
let cache = RefCell::new(HashMap::new());
cache.borrow_mut().insert(key, value);
let val = cache.borrow().get(&key).cloned();

// Use Cell for Copy types
let counter = Cell::new(0);
counter.set(counter.get() + 1);
```

#### Lifetime Elision Rules
```rust
// GOOD: Let the compiler infer lifetimes when possible

// No lifetime annotations needed - elision rule 1
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

// No lifetime annotations needed - elision rule 2
impl Parser {
    fn parse(&self, input: &str) -> &str {
        // Return has lifetime of &self
        &self.buffer
    }
}

// Explicit lifetimes needed - multiple input lifetimes
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

// Complex lifetime relationships
fn parse_header<'input, 'headers>(
    input: &'input [u8],
    headers: &'headers mut HeaderMap,
) -> Result<&'input [u8], Error>
where
    'input: 'headers,  // 'input outlives 'headers
{
    // Parse logic
    Ok(input)
}
```

#### Avoiding Unnecessary Clones
```rust
// BAD: Cloning everything
fn process_data(data: Vec<String>) -> Vec<String> {
    data.iter()
        .map(|s| s.clone().to_uppercase())  // Unnecessary clone!
        .collect()
}

// GOOD: Use references
fn process_data(data: &[String]) -> Vec<String> {
    data.iter()
        .map(|s| s.to_uppercase())  // No clone needed
        .collect()
}

// EXCELLENT: Use Cow for conditional cloning
use std::borrow::Cow;

fn normalize<'a>(input: &'a str) -> Cow<'a, str> {
    if input.chars().all(|c| c.is_lowercase()) {
        Cow::Borrowed(input)  // No allocation!
    } else {
        Cow::Owned(input.to_lowercase())  // Allocate only when needed
    }
}
```

### Iterator Mastery

#### Iterator Combinators
```rust
// EXCELLENT: Chain iterator methods
let result: Vec<_> = numbers
    .iter()
    .filter(|&&x| x > 0)
    .map(|&x| x * 2)
    .take(10)
    .collect();

// Use for_each for side effects
numbers
    .iter()
    .filter(|&&x| x > 100)
    .for_each(|x| println!("Large number: {}", x));

// Use fold for accumulation
let sum = numbers
    .iter()
    .fold(0, |acc, x| acc + x);

// Use partition for splitting
let (evens, odds): (Vec<_>, Vec<_>) = numbers
    .iter()
    .partition(|&&x| x % 2 == 0);
```

#### Custom Iterators
```rust
// GOOD: Implement Iterator for your types
pub struct Fibonacci {
    curr: u64,
    next: u64,
}

impl Fibonacci {
    pub fn new() -> Self {
        Self { curr: 0, next: 1 }
    }
}

impl Iterator for Fibonacci {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.curr;
        self.curr = self.next;
        self.next = curr + self.next;
        Some(curr)
    }
}

// Usage
for num in Fibonacci::new().take(10) {
    println!("{}", num);
}
```

#### Avoid Collect When Possible
```rust
// BAD: Unnecessary allocation
let has_even = numbers
    .iter()
    .filter(|&&x| x % 2 == 0)
    .collect::<Vec<_>>()  // Allocates!
    .len() > 0;

// GOOD: Use iterator methods
let has_even = numbers
    .iter()
    .any(|&x| x % 2 == 0);  // Short-circuits!

// BAD: Collect just to iterate again
let uppercase: Vec<String> = names
    .iter()
    .map(|s| s.to_uppercase())
    .collect();  // Unnecessary

for name in uppercase {
    println!("{}", name);
}

// GOOD: Iterate directly
for name in names.iter().map(|s| s.to_uppercase()) {
    println!("{}", name);
}
```

### Trait Implementation Best Practices

#### Implement Standard Traits
```rust
// ALWAYS implement these traits when applicable
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct User {
    pub id: UserId,
    pub name: String,
}

// Implement Display for user-facing output
impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "User(id={}, name={})", self.id, self.name)
    }
}

// Implement From for convenient conversions
impl From<UserDto> for User {
    fn from(dto: UserDto) -> Self {
        Self {
            id: UserId::from(dto.id),
            name: dto.name,
        }
    }
}

// Implement TryFrom for fallible conversions
impl TryFrom<&str> for UserId {
    type Error = ParseError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        uuid::Uuid::parse_str(s)
            .map(UserId)
            .map_err(|_| ParseError::InvalidUuid)
    }
}

// Implement Default when there's a sensible default
impl Default for Config {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8080,
            max_connections: 100,
        }
    }
}
```

#### Trait Bounds and Where Clauses
```rust
// GOOD: Use where clauses for complex bounds
fn process<T>(item: T) -> Result<Output, Error>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
    T::Error: Into<Error>,
{
    // Implementation
    Ok(Output::default())
}

// EXCELLENT: Use trait aliases (requires nightly or wait for stabilization)
// trait Processable = Serialize + DeserializeOwned + Send + Sync + 'static;
// fn process<T: Processable>(item: T) -> Result<Output, Error> { }
```

#### Avoid Over-Generic Code
```rust
// BAD: Too generic, hard to understand
fn do_thing<T, U, V, F, G>(t: T, f: F, g: G) -> Result<V, Error>
where
    T: Into<U>,
    U: SomeTrait,
    F: Fn(U) -> V,
    G: Fn(Error) -> Error,
{
    // What does this even do?
    unimplemented!()
}

// GOOD: Concrete types with clear purpose
fn transform_user(
    user_dto: UserDto,
    transformer: impl Fn(UserDto) -> User,
) -> Result<User, ValidationError> {
    // Clear and understandable
    let user = transformer(user_dto);
    validate_user(&user)?;
    Ok(user)
}
```

### Async/Await Best Practices

#### Task Management
```rust
use tokio::task;

// GOOD: Spawn tasks for concurrent work
async fn process_all(items: Vec<Item>) -> Vec<Result<Output, Error>> {
    let tasks: Vec<_> = items
        .into_iter()
        .map(|item| {
            task::spawn(async move {
                process_item(item).await
            })
        })
        .collect();

    // Await all tasks
    let results = futures::future::join_all(tasks).await;

    results
        .into_iter()
        .map(|r| r.expect("task panicked"))
        .collect()
}

// EXCELLENT: Use select for racing tasks
use tokio::select;

async fn fetch_with_timeout(url: &str) -> Result<Response, Error> {
    select! {
        result = fetch(url) => result,
        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            Err(Error::Timeout)
        }
    }
}
```

#### Avoiding Blocking in Async
```rust
// BAD: Blocking in async context
async fn bad_example() {
    let data = std::fs::read_to_string("file.txt");  // Blocks executor!
    process(data).await;
}

// GOOD: Use async I/O
async fn good_example() -> Result<(), Error> {
    let data = tokio::fs::read_to_string("file.txt").await?;
    process(&data).await?;
    Ok(())
}

// GOOD: Use spawn_blocking for CPU-intensive work
async fn process_heavy_computation(data: Vec<u8>) -> Result<Output, Error> {
    tokio::task::spawn_blocking(move || {
        // CPU-intensive work here
        expensive_computation(&data)
    })
    .await
    .map_err(|e| Error::TaskJoin(e))?
}
```

#### Stream Processing
```rust
use tokio_stream::StreamExt;

// EXCELLENT: Process streams efficiently
async fn process_stream(stream: impl Stream<Item = Event>) {
    stream
        .filter(|event| event.is_valid())
        .map(|event| process_event(event))
        .buffer_unordered(10)  // Process 10 concurrently
        .for_each(|result| async move {
            match result {
                Ok(output) => handle_output(output).await,
                Err(e) => log_error(e),
            }
        })
        .await;
}
```

### Performance Optimization

#### Allocation Reduction
```rust
// BAD: Many small allocations
fn build_message(parts: &[&str]) -> String {
    let mut msg = String::new();
    for part in parts {
        msg = msg + part;  // Reallocates each time!
    }
    msg
}

// GOOD: Pre-allocate with capacity
fn build_message(parts: &[&str]) -> String {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    let mut msg = String::with_capacity(total_len);
    for part in parts {
        msg.push_str(part);
    }
    msg
}

// EXCELLENT: Use join
fn build_message(parts: &[&str]) -> String {
    parts.join("")
}
```

#### Stack vs Heap
```rust
// GOOD: Use stack allocation for small arrays
let buffer: [u8; 1024] = [0; 1024];  // Stack allocated

// Use Vec for dynamic or large data
let mut large_buffer = Vec::with_capacity(1024 * 1024);

// Use Box for large stack items
let large_struct = Box::new(VeryLargeStruct::default());

// Use SmallVec for small collections
use smallvec::SmallVec;
let small: SmallVec<[u32; 8]> = SmallVec::new();  // No heap if <= 8 items
```

#### Inline Hints
```rust
// Use inline hints for small, frequently called functions
#[inline]
pub fn is_valid_id(id: u64) -> bool {
    id > 0 && id < 1_000_000
}

// Use inline(always) sparingly
#[inline(always)]
fn critical_fast_path() {
    // Only for truly performance-critical code
}

// Prevent inlining for large functions
#[inline(never)]
fn large_rarely_called_function() {
    // Keep binary size down
}
```

### Testing Excellence

#### Unit Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new("Alice".to_string(), "alice@example.com".to_string())
            .expect("should create user");

        assert_eq!(user.name, "Alice");
        assert_eq!(user.email(), "alice@example.com");
    }

    #[test]
    fn test_invalid_email() {
        let result = User::new("Bob".to_string(), "invalid-email".to_string());
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::InvalidEmail(_))));
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_divide_by_zero() {
        divide(10, 0);
    }
}
```

#### Property-Based Testing
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip_serialization(user in any::<User>()) {
        let serialized = serde_json::to_string(&user)?;
        let deserialized: User = serde_json::from_str(&serialized)?;
        prop_assert_eq!(user, deserialized);
    }

    #[test]
    fn test_parse_never_panics(s in ".*") {
        // Should never panic, regardless of input
        let _ = parse_input(&s);
    }
}
```

#### Integration Testing
```rust
// tests/integration_test.rs
use myapp::prelude::*;

#[tokio::test]
async fn test_full_workflow() {
    // Setup
    let db = setup_test_database().await;
    let service = UserService::new(db.clone());

    // Execute
    let user = service.create_user("Alice", "alice@example.com").await?;
    let fetched = service.get_user(user.id()).await?;

    // Verify
    assert_eq!(user, fetched);

    // Cleanup
    cleanup_test_database(db).await;

    Ok::<(), Error>(())
}
```

#### Benchmarking
```rust
// benches/benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_parse(c: &mut Criterion) {
    let input = "sample input string";

    c.bench_function("parse_input", |b| {
        b.iter(|| parse_input(black_box(input)))
    });
}

criterion_group!(benches, benchmark_parse);
criterion_main!(benches);
```

### Security Best Practices

#### Input Validation
```rust
// ALWAYS validate untrusted input
pub fn process_user_input(input: &str) -> Result<ProcessedData, Error> {
    // Length check
    if input.len() > MAX_INPUT_LENGTH {
        return Err(Error::InputTooLong);
    }

    // Character validation
    if !input.chars().all(|c| c.is_alphanumeric() || c.is_whitespace()) {
        return Err(Error::InvalidCharacters);
    }

    // Sanitize and process
    let sanitized = input.trim();
    Ok(ProcessedData::from(sanitized))
}
```

#### Secrets Management
```rust
use secrecy::{Secret, ExposeSecret};

// DON'T: Store secrets in plain String
// let api_key = "secret_key_123";

// DO: Use secrecy crate
let api_key: Secret<String> = Secret::new(load_api_key());

// Use only when needed, never log
fn make_request(api_key: &Secret<String>) -> Result<Response, Error> {
    let key = api_key.expose_secret();  // Explicit exposure
    // Use key for request
    Ok(Response::default())
}

// Secret won't be exposed in Debug output
#[derive(Debug)]
struct Config {
    api_key: Secret<String>,  // Won't appear in debug output
}
```

#### Avoiding Common Vulnerabilities
```rust
// SQL Injection Prevention
async fn get_user_safe(pool: &PgPool, user_id: i64) -> Result<User, Error> {
    // GOOD: Use parameterized queries
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(user_id)  // Safely bound parameter
    .fetch_one(pool)
    .await
    .map_err(Error::from)
}

// Command Injection Prevention
use std::process::Command;

fn run_command_safe(file_path: &str) -> Result<(), Error> {
    // Validate input
    let path = Path::new(file_path);
    if !path.exists() || !path.is_file() {
        return Err(Error::InvalidPath);
    }

    // Use argument array, not shell
    Command::new("process")
        .arg(path)  // Safe: not passed through shell
        .output()?;

    Ok(())
}
```

## Valid Code Requirements

Code is considered valid when it passes ALL of the following:

### Compilation and Linting
- [x] **Compiles**: `cargo build` succeeds with zero errors
- [x] **Warnings**: `cargo build` produces zero warnings
- [x] **Clippy**: `cargo clippy -- -D warnings` passes with zero warnings
- [x] **Format**: `cargo fmt -- --check` confirms code is formatted
- [x] **Documentation**: All public items have doc comments
- [x] **Tests**: `cargo test` all tests pass
- [x] **Audit**: `cargo audit` shows no known vulnerabilities

### Code Quality
- [x] No `unwrap()` or `expect()` in production code paths
- [x] Proper error handling with `Result<T, E>`
- [x] All public APIs documented with examples
- [x] Test coverage >= 80% for critical paths
- [x] No `unsafe` blocks without extensive justification
- [x] No compiler warnings allowed
- [x] Follows naming conventions
- [x] Uses type system effectively (newtype pattern, enums)

### Pre-commit Checklist
```bash
#!/bin/bash
# Save as .git/hooks/pre-commit and chmod +x

set -e

echo "Running pre-commit checks..."

# Format check
echo "1. Checking formatting..."
cargo fmt -- --check

# Clippy
echo "2. Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Tests
echo "3. Running tests..."
cargo test --all-features

# Build
echo "4. Building..."
cargo build --all-features

# Audit (warning only)
echo "5. Security audit..."
cargo audit || echo "Warning: Security vulnerabilities found"

echo "All checks passed!"
```

## Code Verification Workflow

### Overview

**MANDATORY**: Every code change in Rust MUST be verified by a dedicated Rust Verification Agent before being committed. This is a **HARD REQUIREMENT** with **ZERO TOLERANCE** for violations.

### Verification Agent Responsibility

There can only be **ONE Rust Verification Agent** active at any time for a given set of changes. The Main Agent is responsible for:

1. **Delegating** to the Rust Verification Agent after implementation is complete
2. **Waiting** for verification results before proceeding
3. **Not committing** any Rust code until verification passes
4. **Reporting** verification results to the user

### When Verification Must Run

Verification MUST run:
- ✅ After ANY code changes to `.rs` files
- ✅ After changes to `Cargo.toml` or `Cargo.lock`
- ✅ After adding new dependencies
- ✅ After updating dependencies
- ✅ Before ANY commit containing Rust code
- ✅ After merging or rebasing branches

### Verification Agent Workflow

#### Step 1: Agent Delegation

**Main Agent** responsibilities:
```
1. Implementation agent completes Rust code changes
2. Implementation agent reports completion to Main Agent
3. Main Agent spawns ONE Rust Verification Agent
4. Main Agent provides verification agent with:
   - List of changed files
   - Description of changes made
   - Specification reference (if applicable)
5. Main Agent WAITS for verification results
```

**Verification Agent** receives:
- Context about what was changed
- Why it was changed
- Expected behavior
- Files modified

#### Step 2: Verification Agent Execution

The **Rust Verification Agent** MUST execute ALL of the following checks in order:

##### 1. Format Verification
```bash
cargo fmt -- --check
```
- **MUST PASS**: Code must be properly formatted
- **On Failure**: Run `cargo fmt` and report formatting issues
- **Zero Tolerance**: No unformatted code allowed

##### 2. Clippy Linting
```bash
cargo clippy --all-targets --all-features -- -D warnings
```
- **MUST PASS**: Zero clippy warnings allowed
- **On Failure**: Report ALL warnings with file locations
- **Zero Tolerance**: Fix all warnings before proceeding

##### 3. Compilation Check
```bash
cargo build --all-features
cargo build --all-features --release
```
- **MUST PASS**: Code must compile in both debug and release modes
- **On Failure**: Report compilation errors with context
- **Verify**: Both debug and release profiles compile

##### 4. Test Execution
```bash
cargo test --all-features
cargo test --all-features --release
```
- **MUST PASS**: All tests must pass in both modes
- **On Failure**: Report which tests failed and why
- **Verify**: Both debug and release tests pass

##### 5. Documentation Check
```bash
cargo doc --no-deps --all-features
```
- **MUST PASS**: Documentation must build without errors
- **On Failure**: Report missing or broken documentation
- **Verify**: All public items are documented

##### 6. Security Audit
```bash
cargo audit
```
- **MUST PASS**: No known security vulnerabilities
- **On Warning**: Report vulnerabilities with severity
- **Action**: Update dependencies or document accepted risks

##### 7. Dependency Check (Optional but Recommended)
```bash
cargo deny check
```
- **Check**: License compatibility, banned dependencies
- **On Failure**: Report policy violations
- **Action**: Remove or replace problematic dependencies

#### Step 3: Standards Compliance Verification

The Verification Agent MUST also verify compliance with this stack file:

##### Code Quality Checks
- [ ] No `unwrap()` or `expect()` in production code
  ```bash
  rg "\.unwrap\(\)" --type rust src/
  rg "\.expect\(" --type rust src/
  ```
  - Exclude test files (`#[cfg(test)]` modules)
  - Report any violations with file and line number

- [ ] Proper error handling with `Result<T, E>`
  - Verify functions that can fail return `Result`
  - Check error types are well-defined (use thiserror/anyhow)

- [ ] All public items have documentation
  ```bash
  # Check for missing docs (clippy will catch this)
  cargo clippy -- -W missing_docs
  ```

- [ ] Naming conventions followed
  - snake_case for functions, variables, modules
  - PascalCase for types, traits, enums
  - SCREAMING_SNAKE_CASE for constants

- [ ] No compiler warnings
  ```bash
  cargo build --all-features 2>&1 | grep "warning:"
  ```
  - Must return empty (zero warnings)

##### Documentation Verification
- [ ] Public functions have doc comments with:
  - Summary description
  - `# Arguments` section (if applicable)
  - `# Returns` section
  - `# Errors` section (for Result returns)
  - `# Examples` section
  - `# Safety` section (for unsafe code)

- [ ] Examples in doc comments are tested
  ```bash
  cargo test --doc
  ```

#### Step 4: Verification Report

The Verification Agent MUST generate a comprehensive report:

##### Report Format
```markdown
# Rust Verification Report

## Summary
- **Status**: PASS ✅ / FAIL ❌
- **Files Changed**: [list of files]
- **Verification Time**: [timestamp]

## Check Results

### 1. Format Check
- **Status**: PASS ✅ / FAIL ❌
- **Details**: [any issues found]

### 2. Clippy Linting
- **Status**: PASS ✅ / FAIL ❌
- **Warnings**: [count]
- **Details**: [warning messages]

### 3. Compilation
- **Debug Build**: PASS ✅ / FAIL ❌
- **Release Build**: PASS ✅ / FAIL ❌
- **Details**: [any errors]

### 4. Tests
- **Tests Run**: [count]
- **Tests Passed**: [count]
- **Tests Failed**: [count]
- **Details**: [failure details]

### 5. Documentation
- **Status**: PASS ✅ / FAIL ❌
- **Details**: [any issues]

### 6. Security Audit
- **Status**: PASS ✅ / FAIL ❌
- **Vulnerabilities**: [count]
- **Details**: [vulnerability list]

### 7. Standards Compliance
- **unwrap/expect Check**: PASS ✅ / FAIL ❌
- **Error Handling**: PASS ✅ / FAIL ❌
- **Documentation**: PASS ✅ / FAIL ❌
- **Naming Conventions**: PASS ✅ / FAIL ❌
- **Compiler Warnings**: PASS ✅ / FAIL ❌

## Overall Assessment

[Detailed explanation of verification results]

## Recommendations

[Any suggestions for improvement]

## Blockers

[Any issues that prevent commit]
```

#### Step 5: Main Agent Response

Based on Verification Agent report:

##### If Verification PASSES (✅)
```
Main Agent actions:
1. Receives PASS report from Verification Agent
2. Reviews report for any warnings or recommendations
3. Commits the changes following Rule 03 (Work Commit Rules)
4. Includes verification summary in commit message:
   "Verified by Rust Verification Agent: All checks passed"
5. Pushes to remote following Rule 05 (Git Auto-Approval)
6. Reports success to user
```

##### If Verification FAILS (❌)
```
Main Agent actions:
1. Receives FAIL report from Verification Agent
2. DOES NOT COMMIT any code
3. Reports failures to implementation agent or user
4. Lists all issues that must be fixed:
   - Formatting issues
   - Clippy warnings
   - Compilation errors
   - Test failures
   - Documentation issues
   - Standards violations
5. Implementation agent fixes issues
6. Repeats verification process
7. ONLY proceeds after PASS
```

### Verification Agent Requirements

The Verification Agent MUST:
- ✅ Be spawned by Main Agent ONLY
- ✅ Run ALL checks in order
- ✅ Generate comprehensive report
- ✅ Report results to Main Agent
- ✅ NOT commit any code (Main Agent's responsibility)
- ✅ NOT proceed with partial passes (all checks must pass)

The Verification Agent MUST NOT:
- ❌ Skip any verification checks
- ❌ Ignore failures ("we'll fix it later")
- ❌ Commit code directly
- ❌ Proceed when checks fail
- ❌ Run concurrently (only one per language stack)

### Example Workflow

#### Good Example ✅
```
1. User: "Implement user authentication in Rust"
2. Main Agent: Creates specification
3. Main Agent: Spawns Rust Implementation Agent
4. Implementation Agent: Writes authentication code
5. Implementation Agent: Reports completion to Main Agent
6. Main Agent: Spawns Rust Verification Agent
7. Verification Agent: Runs all checks
8. Verification Agent: All checks PASS ✅
9. Verification Agent: Generates report
10. Verification Agent: Returns report to Main Agent
11. Main Agent: Reviews report
12. Main Agent: Commits code with verification note
13. Main Agent: Reports success to user
```

#### Bad Example ❌
```
1. User: "Implement user authentication in Rust"
2. Main Agent: Creates specification
3. Main Agent: Spawns Rust Implementation Agent
4. Implementation Agent: Writes authentication code
5. Implementation Agent: Commits code directly ❌ VIOLATION!
   (Should have reported to Main Agent first)
6. Code contains `unwrap()` calls ❌ VIOLATION!
7. Tests are failing ❌ VIOLATION!
8. No verification was run ❌ CRITICAL VIOLATION!

Result: Code quality compromised, standards violated
```

### Integration with Other Rules

#### Works With Rule 03 (Work Commit Rules)
- Verification happens BEFORE commit
- Commit message includes verification status
- Only verified code is committed

#### Works With Rule 04 (Agent Orchestration)
- Main Agent orchestrates verification
- Implementation agents don't commit directly
- Verification agent is specialized for quality checks

#### Works With Rule 06 (Specifications and Requirements)
- Verification agent receives specification context
- Tests verify requirements are met
- Verification report confirms completion

#### Works With Rule 07 (Language Conventions)
- Verification enforces stack standards
- Checks compliance with this document
- Updates Learning Log when new patterns discovered

### Enforcement

#### Zero Tolerance Policy

**VIOLATIONS** are treated with **ZERO TOLERANCE**:

- ❌ **FORBIDDEN**: Committing Rust code without verification
- ❌ **FORBIDDEN**: Skipping verification checks
- ❌ **FORBIDDEN**: Ignoring verification failures
- ❌ **FORBIDDEN**: Running verification after commit
- ❌ **FORBIDDEN**: Multiple concurrent verification agents

#### Violation Consequences

Any agent that violates verification requirements will:
1. Have their changes **REVERTED**
2. Be required to run verification properly
3. Fix ALL issues before re-attempting
4. Document the violation in Learning Log
5. Report the violation to user

#### User Impact

Violations have serious consequences:
- ❌ **Broken builds** in production
- ❌ **Failed tests** discovered too late
- ❌ **Security vulnerabilities** undetected
- ❌ **Code quality degradation** over time
- ❌ **Technical debt** accumulation
- ❌ **User trust** in agent reliability lost

**THE USER WILL BE UPSET** if code is committed without proper verification!

### Verification Commands Quick Reference

```bash
# Complete verification suite (run in order)

# 1. Format
cargo fmt -- --check

# 2. Lint
cargo clippy --all-targets --all-features -- -D warnings

# 3. Build
cargo build --all-features
cargo build --all-features --release

# 4. Test
cargo test --all-features
cargo test --all-features --release

# 5. Doc
cargo doc --no-deps --all-features

# 6. Audit
cargo audit

# 7. Standards Check
rg "\.unwrap\(\)" --type rust src/
rg "\.expect\(" --type rust src/

# All checks must PASS before commit
```

### Continuous Improvement

When verification catches issues:
1. **Document the issue** in Learning Log
2. **Explain why it was wrong**
3. **Show the correct approach**
4. **Update examples** if needed
5. **Commit Learning Log** update

This creates a self-improving system where standards evolve based on real issues encountered.

## Common Pitfalls and Solutions

### Pitfall 1: String Handling Inefficiency
**Problem**: Unnecessary String allocations and conversions.
```rust
// BAD
fn process(s: String) -> String {
    s.to_uppercase()  // Takes ownership, caller must clone
}

// GOOD
fn process(s: &str) -> String {
    s.to_uppercase()  // Borrows, no clone needed
}

// EXCELLENT: Return Cow for zero-copy when possible
fn process(s: &str) -> Cow<'_, str> {
    if s.is_empty() {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.to_uppercase())
    }
}
```

### Pitfall 2: Ignoring Compiler Errors/Warnings
**Problem**: Warnings today become bugs tomorrow.
**Solution**: Enable `#![deny(warnings)]` in lib.rs and fix all warnings immediately.

### Pitfall 3: Not Using Rust 2021 Edition Features
**Problem**: Missing out on better error messages and language improvements.
**Solution**: Always use `edition = "2021"` in Cargo.toml.

### Pitfall 4: Mutex Deadlocks
**Problem**: Holding locks across await points or acquiring locks in wrong order.
```rust
// BAD: Holding lock across await
let data = mutex.lock().unwrap();
something_async().await;  // Still holding lock!
drop(data);

// GOOD: Drop lock before await
let data = {
    let guard = mutex.lock().unwrap();
    guard.clone()
};  // Lock dropped here
something_async().await;

// EXCELLENT: Use Tokio's async-aware Mutex
let data = mutex.lock().await;
something_async().await;
drop(data);
```

### Pitfall 5: Collecting Unnecessarily
**Problem**: Allocating intermediate vectors when not needed.
```rust
// BAD
let sum = numbers
    .iter()
    .map(|x| x * 2)
    .collect::<Vec<_>>()  // Unnecessary allocation!
    .iter()
    .sum();

// GOOD
let sum: i32 = numbers
    .iter()
    .map(|x| x * 2)
    .sum();  // Direct sum, no allocation
```

### Pitfall 6: Not Using rustfmt
**Problem**: Inconsistent code style across team.
**Solution**: Run `cargo fmt` before every commit. Set up CI to enforce.

### Pitfall 7: Large Stack Allocations
**Problem**: Stack overflow from large arrays.
```rust
// BAD: Can cause stack overflow
let big_array = [0u8; 1024 * 1024];  // 1MB on stack!

// GOOD: Use Vec for large data
let big_array = vec![0u8; 1024 * 1024];  // Heap allocated

// GOOD: Use Box for large structs
let big_struct = Box::new(VeryLargeStruct::default());
```

### Pitfall 8: Not Implementing Error Traits
**Problem**: Errors that can't be used with `?` operator or error chains.
```rust
// BAD
#[derive(Debug)]
pub struct MyError {
    message: String,
}

// GOOD: Implement std::error::Error
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct MyError {
    message: String,
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for MyError {}

// EXCELLENT: Use thiserror
use thiserror::Error;

#[derive(Error, Debug)]
#[error("{message}")]
pub struct MyError {
    message: String,
}
```

## Advanced Topics

### Macros
```rust
// Declarative macros for repetitive code
macro_rules! impl_id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(uuid::Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

impl_id_type!(UserId);
impl_id_type!(SessionId);
impl_id_type!(ProductId);
```

### Unsafe Code
```rust
// Only use unsafe when absolutely necessary
// MUST document invariants and safety guarantees
/// # Safety
///
/// The caller must ensure that:
/// - `ptr` is valid for reads of `len` bytes
/// - `ptr` is properly aligned
/// - The memory referenced by `ptr` is initialized
/// - No other threads are accessing this memory
pub unsafe fn read_raw_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
    // SAFETY: Caller guarantees pointer validity
    unsafe {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}
```

### FFI (Foreign Function Interface)
```rust
// When interfacing with C code
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[link(name = "mylib")]
extern "C" {
    fn external_function(input: *const c_char) -> i32;
}

pub fn safe_wrapper(input: &str) -> Result<i32, Error> {
    let c_string = CString::new(input)?;

    // SAFETY: c_string is valid C string, external_function expects C string
    let result = unsafe { external_function(c_string.as_ptr()) };

    Ok(result)
}
```

## Learning Log

### 2026-01-11: Comprehensive Rust Standards Established
**Issue**: Initial creation of comprehensive Rust coding standards.
**Learning**: Established expert-level standards covering:
- Advanced error handling patterns (thiserror, anyhow)
- Type system mastery (newtypes, type states, builder pattern)
- Ownership and lifetime best practices
- Iterator patterns and performance optimization
- Async/await best practices
- Security considerations
- Comprehensive testing strategies
- Advanced tooling configuration

**Corrective Action**: None (initial creation).
**New Standard**: All Rust code must follow these expert-level standards with zero tolerance for deviations.

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
