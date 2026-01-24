# llama-cpp

A wrapper around the [llama-cpp](https://github.com/ggerganov/llama.cpp/) library for rust.


# Dependencies

This uses bindgen to build the bindings to llama.cpp. This means that you need to have clang installed on your system.

If this is a problem for you, open an issue, and we can look into including the bindings. 

See [bindgen](https://rust-lang.github.io/rust-bindgen/requirements.html) for more information.

# Disclaimer

This crate is *not safe*. There is absolutly ways to misuse the llama.cpp API provided to create UB, please create an issue if you spot one. Do not use this code for tasks where UB is not acceptable.

This is not a simple library to use. In an ideal world a nice abstraction would be written on top of this crate to
provide an ergonomic API - the benefits of this crate over raw bindings is safety (and not much of it as that) and not much else.

## Context

Originally created and built by [utilityai/llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs) released with MIT/Apache licenses.

But was derived and broken down into internal crates to maintain ownership and usage matching desired configuration.
