# Directorate

[![Build Status](https://github.com/ewestudios/ewe_platform/workflows/Test/badge.svg)](https://github.com/ewestudios/ewe_platform/rust-embed/actions?query=workflow%3ATest)
[![crates.io](https://img.shields.io/crates/v/rust-embed.svg)](https://crates.io/crates/directorate)
[![docs.rs](https://img.shields.io/crates/v/rust-embed.svg)](https://docs.rs/directorate/0.0.1/directorate/)

Provides a small but usable wrapper for the rust_embed crate, sometimes you want to use the rust Embed implement struct
as an object you can clone and pass around but the rust_embed `Embed` interface has all it's method as non-attached (they do not use self)
so you cant really use them as an object you can pass around (I assume for performance reasons).

The directorate crate exists to attempt to provide a way to do so:

```Rust
use directorate::Directorate;

#[derive(rust_embed::Embed, Default)]
#[folder = "test_directory/"]
struct Directory;

let generator = Directorate::<Directory>::default();
let readme_file = generator.get_file("README.md");

```
