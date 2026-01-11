# Rust Coding Standards

## Overview
- **Language**: Rust 1.75+ (stable)
- **Use Cases**: High-performance backend services, system tools, CLI applications, performance-critical code
- **Official Docs**: https://doc.rust-lang.org/

## Setup and Tools

### Required Tools
- **rustc**: Rust compiler (via rustup)
- **cargo**: Rust package manager and build tool
- **rustfmt**: Code formatter (ships with Rust)
- **clippy**: Linter for catching common mistakes
- **cargo-audit**: Security vulnerability scanner
- **cargo-nextest** (optional): Next-generation test runner

### Installation
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy
cargo install cargo-audit
```

### Configuration Files
- **Cargo.toml**: Package manifest and dependencies
- **Cargo.lock**: Locked dependency versions (commit this!)
- **rustfmt.toml**: Formatter configuration
- **.clippy.toml**: Clippy linter configuration
- **rust-toolchain.toml**: Specify Rust version for project

### Recommended Cargo.toml Settings
```toml
[package]
name = "project-name"
version = "0.1.0"
edition = "2021"  # Use latest edition
rust-version = "1.75"  # Minimum Rust version

[dependencies]
# Production dependencies

[dev-dependencies]
# Test/dev dependencies

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true  # Strip symbols from binary
```

### Recommended Clippy Configuration
```toml
# .clippy.toml or clippy.toml
disallowed-methods = [
    # Disallow panicking in production code
    { path = "std::panic", reason = "use Result instead" },
    { path = "std::option::Option::unwrap", reason = "use ? or handle error properly" },
    { path = "std::option::Option::expect", reason = "use ? or handle error properly" },
    { path = "std::result::Result::unwrap", reason = "use ? or handle error properly" },
    { path = "std::result::Result::expect", reason = "use ? or handle error properly" },
]
```

## Coding Standards

### Naming Conventions
- **Variables and Functions**: snake_case
  - `let user_name = "John";`
  - `fn calculate_total() -> u32 {}`
- **Types (Structs, Enums, Traits)**: PascalCase
  - `struct UserAccount {}`
  - `enum UserRole { Admin, User, Guest }`
  - `trait Validator {}`
- **Constants**: UPPER_SNAKE_CASE
  - `const MAX_RETRIES: u32 = 3;`
- **Modules**: snake_case
  - `mod user_service;`
- **Type Parameters**: Single uppercase letter or PascalCase
  - `fn process<T>(item: T) {}`
  - `struct Wrapper<TData> { data: TData }`
- **Lifetimes**: Short, lowercase, descriptive
  - `fn longest<'a>(x: &'a str, y: &'a str) -> &'a str`
- **Files**: snake_case, matching module name
  - `user_service.rs`, `api_client.rs`
- **Test Functions**: snake_case with test_ prefix
  - `fn test_user_creation()`

### Code Organization
- One module per file
- Use `mod.rs` or the new `module_name.rs` pattern for module directories
- Group related functionality into modules
- Public API exposed through `pub` keyword carefully
- Re-export commonly used items in lib.rs or mod.rs

**Module Structure Example**:
```
src/
├── lib.rs              # Public API
├── api/
│   ├── mod.rs          # api module root
│   ├── client.rs       # api::client
│   └── routes.rs       # api::routes
├── models/
│   ├── mod.rs
│   ├── user.rs
│   └── post.rs
└── utils.rs
```

**lib.rs Public API**:
```rust
// Re-export commonly used items
pub use api::client::ApiClient;
pub use models::{User, Post};

pub mod api;
pub mod models;
mod utils;  // Private module
```

### Comments and Documentation
- **Doc comments** (`///`) for all public items
- Module-level docs at top of file: `//!`
- Inline comments (`//`) for complex logic only
- Code should be self-documenting through good naming
- Examples in doc comments are encouraged (and tested!)

**Documentation Example**:
```rust
//! User service module providing user management functionality.

/// Represents a user in the system.
///
/// Users have a unique ID, name, and email address. Email addresses
/// must be unique across the system.
///
/// # Examples
///
/// ```
/// use myapp::User;
///
/// let user = User::new(1, "John", "john@example.com");
/// assert_eq!(user.name(), "John");
/// ```
pub struct User {
    id: u64,
    name: String,
    email: String,
}

impl User {
    /// Creates a new user with the given details.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the user
    /// * `name` - Display name
    /// * `email` - Email address (must be valid format)
    ///
    /// # Panics
    ///
    /// This function does not panic. Note: Only document panics in public APIs.
    pub fn new(id: u64, name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            email: email.into(),
        }
    }
}
```

## Best Practices

### Rust-Specific Idioms

#### Error Handling
- **MANDATORY**: Use `Result<T, E>` for operations that can fail
- **FORBIDDEN**: Using `unwrap()` or `expect()` in production code
- Use the `?` operator for error propagation
- Create custom error types with `thiserror` or `anyhow`
- Return errors, don't panic

**Good Example**:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UserError {
    #[error("User not found: {0}")]
    NotFound(u64),

    #[error("Invalid email format: {0}")]
    InvalidEmail(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn fetch_user(id: u64) -> Result<User, UserError> {
    let user = db::query("SELECT * FROM users WHERE id = ?", id)
        .fetch_one()  // This returns Result
        .await?;      // Propagate error with ?

    Ok(user)
}

// Usage
match fetch_user(1).await {
    Ok(user) => println!("Found user: {}", user.name),
    Err(UserError::NotFound(id)) => println!("User {} not found", id),
    Err(e) => eprintln!("Error: {}", e),
}
```

**Bad Example**:
```rust
// BAD - Don't do this!
pub fn fetch_user(id: u64) -> User {
    let user = db::query("SELECT * FROM users WHERE id = ?", id)
        .fetch_one()
        .await
        .unwrap();  // FORBIDDEN - will panic on error!

    user
}
```

#### Option Handling
- Use `Option<T>` for values that may not exist
- Avoid `unwrap()`, use pattern matching or combinators
- Use `?` operator with functions returning `Option` or `Result`

```rust
// Good - using combinators
let name = user.name.as_ref().unwrap_or("Anonymous");
let upper = user.name.map(|n| n.to_uppercase());

// Good - using if let
if let Some(email) = user.email {
    send_email(&email);
}

// Good - using match
match user.role {
    Some(Role::Admin) => grant_admin_access(),
    Some(Role::User) => grant_user_access(),
    None => deny_access(),
}
```

#### Ownership and Borrowing
- Prefer borrowing over ownership transfer when possible
- Use `&T` for read-only access
- Use `&mut T` for mutable access
- Return owned values when the caller needs ownership
- Clone only when necessary (and document why)

```rust
// Good - borrowing
fn print_name(user: &User) {
    println!("{}", user.name);
}

// Good - mutable borrowing
fn update_email(user: &mut User, email: String) {
    user.email = email;
}

// Good - taking ownership when needed
fn consume_user(user: User) -> ProcessedData {
    // Process and transform user
    ProcessedData::from(user)
}
```

#### Lifetimes
- Let the compiler infer lifetimes when possible
- Explicit lifetimes only when necessary
- Name lifetimes descriptively in complex cases

```rust
// Compiler infers lifetimes
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

// Explicit lifetime when needed
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

#### Iterators Over Loops
- Prefer iterator methods over manual loops
- Use `iter()`, `into_iter()`, `iter_mut()` appropriately
- Chain iterator methods for data transformations
- More functional style is more idiomatic in Rust

```rust
// Good - iterator methods
let doubled: Vec<i32> = numbers.iter()
    .map(|n| n * 2)
    .collect();

let sum: i32 = numbers.iter().sum();

let evens: Vec<i32> = numbers.into_iter()
    .filter(|n| n % 2 == 0)
    .collect();

// Acceptable - when iterator would be too complex
for number in &numbers {
    if complex_condition(number) {
        do_something(number);
        do_something_else(number);
    }
}
```

### Type System Usage
- Use strong types (newtype pattern) for domain concepts
- Leverage the type system to prevent invalid states
- Use enums for mutually exclusive states
- Make invalid states unrepresentable

```rust
// Good - strong types
pub struct UserId(u64);
pub struct Email(String);

impl Email {
    pub fn new(email: String) -> Result<Self, ValidationError> {
        if email.contains('@') {
            Ok(Email(email))
        } else {
            Err(ValidationError::InvalidEmail)
        }
    }
}

// Good - using type system to enforce correctness
pub struct ValidatedUser {
    id: UserId,
    email: Email,  // Can't construct invalid email
}

// Good - enum for states
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { session_id: String },
    Error { reason: String },
}
```

### Testing
- Write unit tests in the same file with `#[cfg(test)]` module
- Write integration tests in `tests/` directory
- Use `cargo test` to run all tests
- Test both success and error cases
- Use `cargo nextest` for faster test execution (optional)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new(1, "John", "john@example.com");
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "John");
    }

    #[test]
    fn test_invalid_email() {
        let result = Email::new("invalid".to_string());
        assert!(result.is_err());
    }

    #[tokio::test]  // For async tests
    async fn test_fetch_user() {
        let result = fetch_user(1).await;
        assert!(result.is_ok());
    }
}
```

### Performance
- Rust is fast by default, don't premature optimize
- Use `cargo bench` for benchmarking
- Profile before optimizing (use `perf`, `flamegraph`, etc.)
- Avoid unnecessary allocations
- Use `&str` instead of `String` when possible
- Consider `Cow<str>` for sometimes-owned strings
- Use `SmallVec` for small collections
- Zero-cost abstractions mean abstractions are free

```rust
// Good - avoid unnecessary String allocations
fn process(text: &str) -> &str {
    text.trim()  // Returns &str, no allocation
}

// Good - using Cow to avoid clones when not needed
use std::borrow::Cow;

fn make_uppercase(s: &str) -> Cow<str> {
    if s.chars().all(|c| c.is_uppercase()) {
        Cow::Borrowed(s)  // No allocation
    } else {
        Cow::Owned(s.to_uppercase())  // Only allocate when needed
    }
}
```

### Security
- Never use `unsafe` without extensive documentation and justification
- Audit all `unsafe` code carefully
- Use `cargo audit` to check for vulnerable dependencies
- Validate all external input
- Use type system to enforce invariants
- Avoid SQL injection with parameterized queries
- Use prepared statements

### Dependencies
- Prefer well-maintained crates with many downloads
- Check crate quality: documentation, tests, recent updates
- Lock dependencies with Cargo.lock (commit it!)
- Run `cargo audit` regularly
- Keep dependencies up to date
- Minimize dependency count when possible

**Common Quality Crates**:
- `tokio` - async runtime
- `serde` - serialization/deserialization
- `thiserror` - error handling
- `anyhow` - flexible error handling for applications
- `sqlx` - async SQL toolkit
- `axum` / `actix-web` - web frameworks
- `reqwest` - HTTP client
- `tracing` - structured logging

## Valid Code Requirements

Code is considered valid when:
- [x] Passes `cargo fmt --check` (formatted correctly)
- [x] Passes `cargo clippy -- -D warnings` (no linter warnings)
- [x] Passes `cargo test` (all tests pass)
- [x] Passes `cargo build` (compiles without errors)
- [x] Passes `cargo audit` (no known vulnerabilities)
- [x] Has documentation for all public items
- [x] Follows all naming conventions
- [x] Uses proper error handling (no unwrap in prod code)
- [x] Has adequate test coverage

### Pre-commit Checklist
```bash
# Format code
cargo fmt

# Lint code (fail on warnings)
cargo clippy -- -D warnings

# Run tests
cargo test

# Check for security vulnerabilities
cargo audit

# Build in release mode
cargo build --release
```

## Common Pitfalls

### Pitfall 1: Using unwrap() or expect() in Production
**Problem**: `unwrap()` and `expect()` will panic if the value is `None` or `Err`, crashing the program.
**Solution**: Use proper error handling with `?` operator or pattern matching.

**Bad**:
```rust
let user = fetch_user(id).unwrap();  // FORBIDDEN
```

**Good**:
```rust
let user = fetch_user(id)?;  // Propagate error
// or
let user = match fetch_user(id) {
    Ok(u) => u,
    Err(e) => {
        error!("Failed to fetch user: {}", e);
        return Err(e.into());
    }
};
```

### Pitfall 2: Cloning Unnecessarily
**Problem**: Cloning large data structures is expensive and often unnecessary.
**Solution**: Use references and borrowing. Only clone when you truly need owned data.

**Bad**:
```rust
fn print_user(user: User) {  // Takes ownership
    println!("{}", user.name);
}  // user is dropped here

let user = User::new(1, "John", "john@example.com");
print_user(user.clone());  // Unnecessary clone
print_user(user.clone());  // Another clone!
```

**Good**:
```rust
fn print_user(user: &User) {  // Borrows
    println!("{}", user.name);
}

let user = User::new(1, "John", "john@example.com");
print_user(&user);  // Just borrow
print_user(&user);  // Can reuse!
```

### Pitfall 3: Not Handling All Enum Variants
**Problem**: Using `_` wildcard in match can miss newly added enum variants.
**Solution**: Handle all variants explicitly or use `#[non_exhaustive]` attribute.

### Pitfall 4: Blocking the Async Runtime
**Problem**: Running blocking operations in async context blocks the executor.
**Solution**: Use `tokio::task::spawn_blocking` for blocking operations.

```rust
// Bad - blocks async runtime
async fn process_file() {
    let content = std::fs::read_to_string("file.txt");  // Blocking!
}

// Good - spawn blocking task
async fn process_file() {
    let content = tokio::task::spawn_blocking(|| {
        std::fs::read_to_string("file.txt")
    }).await??;
}
```

### Pitfall 5: Not Using References in Loops
**Problem**: Iterating with `for item in collection` moves the collection.
**Solution**: Use `&collection` or `&mut collection` to iterate by reference.

```rust
// Bad - moves vector
for item in vec {  // vec is moved, can't use after loop
    println!("{}", item);
}

// Good - borrows vector
for item in &vec {  // vec can be used after loop
    println!("{}", item);
}
```

### Pitfall 6: Ignoring Compiler Warnings
**Problem**: Warnings often indicate real issues that will become errors later.
**Solution**: Fix all warnings. Use `#[deny(warnings)]` in CI.

## Examples

### Good Example: Type-Safe Database Model
```rust
use sqlx::{FromRow, PgPool};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UserError {
    #[error("User not found")]
    NotFound,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
}

impl User {
    /// Fetches a user by ID from the database.
    ///
    /// # Errors
    ///
    /// Returns `UserError::NotFound` if user doesn't exist.
    /// Returns `UserError::Database` on database errors.
    pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Self, UserError> {
        sqlx::query_as::<_, User>(
            "SELECT id, name, email FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(UserError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_user_not_found() {
        let pool = create_test_pool().await;
        let result = User::find_by_id(&pool, 999).await;
        assert!(matches!(result, Err(UserError::NotFound)));
    }
}
```

**Why This is Good**:
- Proper error handling with custom error type
- Uses `Result` return type
- No `unwrap()` or `expect()`
- Good documentation
- Includes tests
- Type-safe database queries

### Bad Example: Unsafe Database Code
```rust
// BAD - Don't do this!
pub fn get_user(id: i64) -> User {
    let result = sqlx::query("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_one()
        .await
        .unwrap();  // FORBIDDEN - will panic on error!

    User {
        id: result.get("id").unwrap(),  // More unwraps!
        name: result.get("name").unwrap(),
        email: result.get("email").unwrap(),
    }
}
```

**Why This is Bad**:
- Multiple `unwrap()` calls
- Will panic on any error
- Not using sqlx's `FromRow` derive
- No error handling
- No documentation

**How to Fix**: Use the good example above with proper error handling.

## Learning Log

### 2026-01-11: Initial Rust Standards
**Issue**: Creating initial standards document.
**Learning**: Established baseline standards for Rust development in this project.
**Corrective Action**: None (initial creation).
**New Standard**: All Rust code must follow these standards starting from this date.

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
