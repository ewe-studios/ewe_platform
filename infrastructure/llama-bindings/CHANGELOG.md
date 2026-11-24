# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/ewe-studios/ewe_platform/compare/infrastructure_llama_bindings-v0.0.5...infrastructure_llama_bindings-v0.0.6) - 2026-11-24

### Fixed

- *(llama)* stop esaxx from forcing the static MSVC runtime
- *(llama)* configure from a clean build tree on Windows
- *(llama)* unwrap the glob result before copying
- *(llama)* take the first glob path directly on Windows
- *(llama)* match the runtime through a post-project fragment and link build-info
- *(llama)* drop the unsupported --clean-first build argument
- *(llama)* use the cmake crate's cflag method
- *(llama)* force /MT and clean the Windows build
- *(llama)* enable CMP0091 so the runtime library takes effect
- *(llama)* build llama.cpp with the static MSVC runtime
- *(llama)* pass plain paths to CMake on Windows

### Other

- split spec 65 into focused specifications; Cloudflare control moves to spec 77 ([#553](https://github.com/ewe-studios/ewe_platform/pull/553))
