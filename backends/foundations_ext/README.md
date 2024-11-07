# FoundationsExt
An extension crate that provides varying trait extensions for different libraries and utilities. The goal is to have a central place for where such extensions can live without being specifically tied down to the ewe_platform namespace.

## Crates


### serde_ext
Provides useful extensions for the serde crate, it takes inspiration from the [value-ext](https://github.com/jeremychone/rust-value-ext) crate by [Jeremy Chone](https://github.com/jeremychone/).

If you've read [Data Oriented Programming](https://blog.klipse.tech/dop/2022/06/22/principles-of-dop.html) or have used kotlin and love the ease of interacting with basic data structures like maps and list then this will be super familiar.

I have taking inspiration and code from [value-ext](https://github.com/jeremychone/rust-value-ext) crate and expanded support for toml based `Value` types as well to make it super-easy to work with such raw types.

- JSON

```rust
use serde_json::json;
use foundations_ext::serde_ext::DynamicValueExt;
use foundations_ext::serde_ext::JsonValueExt;

let mut value = json!({"tokens": 3, "hello": {"word": "hello"}});

// get a copy of the value
let actual_value: String = value.d_get("/happy/word")?;

// take the value in the path thereby removing from map
let content: String = value.d_take("/hello/word")?;

// add a value into map
value.d_insert("/happy/word", "hello")?;

value.d_walk(|tree, key| {
    // do something
    true
});

```

- TOML

```rust
use toml::toml;
use foundations_ext::serde_ext::DynamicValueExt;
use foundations_ext::serde_ext::TomlValueExt;

let mut value = toml::Value::Table(toml! {
    token=3

    [hello]
    word = "hello"

    [hello.wreckage]
    where = "londo"
});

// get a copy of the value
let actual_value: String = value.d_get("/happy/word")?;

// take the value in the path thereby removing from map
let content: String = value.d_take("/hello/word")?;

// add a value into map
value.d_insert("/happy/word", "hello")?;

value.d_walk(|tree, key| {
    // do something
    true
});

```


## Inspirations

- [rust-value-ext](https://github.com/jeremychone/rust-value-ext)
