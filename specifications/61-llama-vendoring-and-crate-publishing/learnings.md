# Spec 61 learnings

_Written in ASD-STE100 Simplified Technical English (STE)._

## The repository supplies the build environment

`.cargo/config.toml` sets `TOOLS_DIR`, `LLAMA_DIR` and `EMSDK_DIR` as
repo-relative values. mise exports absolute values too. Every cargo command in
the repository sees them, so `build.rs` never takes its "missing env" path.

A consumer has no such config and no such variables. The published
`infrastructure_llama_bindings 0.0.5` panics at `build.rs:343` with
`get TOOLS_DIR environment: NotPresent`.

**How to apply:** test the packaged crate outside the repository with the build
variables unset. A pass inside the repository proves nothing.

## The CI package check does not build

`scripts/ci/package_workspace.py` says it runs `cargo package --no-verify` and
does not build the tarball. release-plz verifies each package at publish time,
but it runs in the repository with the config and the env. Both checks miss a
`build.rs` that needs the repository.

**How to apply:** add one extract-and-build check with a clean environment.

## `cargo package` follows the tracked symlink

The spec said symlinks cannot work. The measurement says otherwise:
`infrastructure/llama-bindings/llama.cpp` is a symlink (mode 120000), and
`cargo package --list` lists 1216 files under it, including `include/llama.h`
and `src/llama.cpp`. The tarball holds the real sources.

**How to apply:** keep the symlink. The defect is not the link; it is that
`build.rs` ignores the copy inside the package.

## The tarball fits the limit

The package is 1229 files, 28.1 MiB unpacked and 5.6 MiB compressed. It is under
crates.io's 10 MiB default. No pruning or limit increase is needed.

## The include list missed CMake inputs, not sources

The first isolated build passed the environment check and then failed in CMake:

```
ggml-version.h.in does not exist
llama-version.h.in does not exist
common/parsers/sources.cmake not found
```

The `include` list had `*.h` and `*.cpp` globs. CMake also reads `.h.in`
templates, `.cmake` files and `LICENSE`. Add `**/*.in`, `**/*.cmake` and the
license file.

**How to apply:** after a vendored build, let CMake run on the tarball. A
source-file list alone is not enough.

## The llama.cpp update changed the public API

Tag `b12563` removed the old names. The wrapper needed these moves:

- state: `llama_state_get_size`, `llama_state_get_data` and
  `llama_state_set_data` take a size; the session file calls are
  `llama_state_save_file` and `llama_state_load_file`;
- model: `llama_model_load_from_file`, `llama_init_from_model`,
  `llama_model_free`, `llama_model_n_ctx_train`, `llama_model_n_embd`;
- vocab: `llama_vocab_bos`, `llama_vocab_eos`, `llama_vocab_nl`,
  `llama_vocab_is_eog`, `llama_vocab_get_attr`, `llama_vocab_n_tokens`;
- sampler: `llama_sampler_init_grammar_lazy_patterns`.

**How to apply:** grep the generated `bindings.rs` for "similarly named
function" hints. Rust names the closest replacement in each error.

## Three environment variables, three different answers

- `TOOLS_DIR` is required and stored in `_tools_directory`, which nothing reads.
- `EMSDK_DIR` is required only for the emscripten prefix path.
- `LLAMA_DIR` has no default, but the package always holds `llama.cpp/`.

**How to apply:** default `LLAMA_DIR`, delete `TOOLS_DIR`, and make `EMSDK_DIR`
optional. Then the same `build.rs` serves the repository and a consumer.
