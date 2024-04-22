//! ## TinyTemplate
//!
//! TinyTemplate is a minimal templating library originally designed for use in [Criterion.rs].
//! It deliberately does not provide all of the features of a full-power template engine, but in
//! return it provides a simple API, clear templating syntax, decent performance and very few
//! dependencies.
//!
//! ## Features
//!
//! The most important features are as follows (see the [syntax](syntax/index.html) module for full
//! details on the template syntax):
//!
//! * Rendering values - `{ myvalue }`
//! * Conditionals - `{{ if foo }}Foo is true{{ else }}Foo is false{{ endif }}`
//! * Loops - `{{ for value in row }}{value}{{ endfor }}`
//! * Customizable value formatters `{ value | my_formatter }`
//! * Macros `{{ call my_template with foo }}`
//!
//! ## Restrictions
//!
//! TinyTemplate was designed with the assumption that the templates are available as static strings,
//! either using string literals or the `include_str!` macro. Thus, it borrows `&str` slices from the
//! template text itself and uses them during the rendering process. Although it is possible to use
//! TinyTemplate with template strings loaded at runtime, this is not recommended.
//!
//! Additionally, TinyTemplate can only render templates into Strings. If you need to render a
//! template directly to a socket or file, TinyTemplate may not be right for you.
//!
//! ## Example
//!
//! ```
//! use serde::Serialize;
//! use template::tiny::TinyTemplate;
//! use std::error::Error;
//!
//! #[derive(Serialize)]
//! struct Context {
//!     name: String,
//! }
//!
//! static TEMPLATE : &'static str = "Hello {name}!";
//!
//! pub fn main() -> Result<(), Box<dyn Error>> {
//!     let mut tt = TinyTemplate::new();
//!     tt.add_template("hello", TEMPLATE)?;
//!
//!     let context = Context {
//!         name: "World".to_string(),
//!     };
//!
//!     let rendered = tt.render("hello", &context)?;
//! #   assert_eq!("Hello World!", &rendered);
//!     println!("{}", rendered);
//!
//!     Ok(())
//! }
//! ```
//!
//! [Criterion.rs]: https://github.com/bheisler/criterion.rs
//!

pub mod compiler;
pub mod error;
pub mod instruction;
pub mod syntax;
pub mod templates;
pub mod tiny;
