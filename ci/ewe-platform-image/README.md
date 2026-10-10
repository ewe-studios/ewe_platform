# ewe_platform CI image

The image every CI job runs in: tests, dev builds and release builds alike.
It is layer 2 of the [platform-images](../platform-images/README.md) unit,
built FROM the generic `ewestudios/ci-base` (layer 1) pinned by digest.

| | Default flavor | CUDA flavor |
|---|---|---|
| Layer 1 | `ewestudios/ci-base` (ubuntu:24.04) | `ewestudios/ci-base-cuda` (nvidia/cuda:12.9.2-devel-ubuntu24.04) |
| Layer 2 | `ewestudios/ewe-platform-ci` | `ewestudios/ewe-platform-ci-cuda` |
| Used by | check.yaml, release.yaml | release.yaml (opt-in CUDA build), publish smoke tests |
| Tags | `hash-<inputs>`, `latest`, `sha-<commit>`, `buildcache` | same |

Docker Hub login: `ewestudios`, with the `DOCKERHUB_EWESTUDIO_TOKEN`
repository secret. The username comes from [`config.toml`](config.toml),
and a `PLATFORM_IMAGES_REGISTRY_USERNAME` repository variable overrides it.

## What layer 2 adds

On top of everything layer 1 has (compilers, GTK/WebKit, Mesa/Vulkan/Xvfb,
Chrome/Firefox, mise tools, rustup; see the
[platform-images README](../platform-images/README.md#what-layer-1-contains)):

- **Rust:** the nightly from `rust-toolchain.toml` (set as the default),
  with every component and target it lists (rustfmt, rustc-dev, rust-src,
  wasm32-unknown-unknown, the Apple and Android targets), plus clippy, the
  Cranelift backend (`rustc-codegen-cranelift-preview`, so the `dev` profile
  works in the image) and the wasm32-wasip1/wasip2 targets.
- **Tools from the repository's `mise.toml`**, at the versions `mise.lock`
  pins: wasm-bindgen-cli (checked against the `wasm-bindgen` version in
  Cargo.lock when the image is built), cargo-nextest, cargo-audit, bacon,
  pnpm, pitchfork, and the shared tools at the same versions as layer 1.
- **Submodules** at their gitlink SHAs, as real (shallow) git checkouts under
  `/opt/ewe/submodules/<path>`:
  `tools/llama.cpp`, `tools/whisper.cpp`, `tools/emsdk`. The lock is
  `/opt/ewe/submodules/submodules.lock.json`
  (`$PLATFORM_IMAGES_SUBMODULES_LOCK`).
- **A warm cargo registry** (`$CARGO_HOME` = `/opt/rust/cargo`): every crate
  in Cargo.lock, for every target, as `.crate` files.

Not baked in, on purpose:

- `tools/dawn`: no crate builds it (`DAWN_DIR` is commented out in
  `infrastructure/llama-bindings/build.rs`), and with its gclient
  dependencies it's several GB. The old `ci-test.Dockerfile` prebuilt it
  for nothing.
- `tools/depot_tools`: it has no URL in `.gitmodules`.
- `.agents`: agent docs, not a build input.
- `artefacts/models/whisper.cpp`: Hugging Face model weights (LFS), not a
  build input.
- A prebuilt llama.cpp. `infrastructure_llama_bindings`' build script always
  compiles `tools/llama.cpp` with CMake into its own `OUT_DIR`, for the
  feature set being built (CPU/Vulkan/CUDA/Metal, OpenMP, mtmd, target CPU
  flags), and deliberately refuses system ggml. A library prebuilt in the
  image could not be reused without changing that build script, so the
  image bakes the sources instead. `sccache` is in the image for wiring up
  as a CMake compiler launcher later (`CMAKE_C_COMPILER_LAUNCHER=sccache`;
  the build script forwards `CMAKE_*` variables).
- The prebuilt V8 library (`librusty_v8`): only `foundation_testbed`'s
  non-default `wasm-embedded-js` feature pulls in `v8`, so default CI builds
  never download it.

## How submodules get into a checkout

`.cargo/config.toml` forces `LLAMA_DIR=tools/llama.cpp` and
`EMSDK_DIR=tools/emsdk` (`force = true`), so the build scripts need the
sources at those paths in the checkout. An environment variable can't point
them elsewhere. CI therefore checks out without submodules and puts the
baked copies there:

```
actions/checkout (submodules: false)        image: /opt/ewe/submodules/
  tools/llama.cpp/   (empty gitlink dir)       tools/llama.cpp   @ 4f31eed…
  tools/whisper.cpp/ (empty)                   tools/whisper.cpp @ 35cb684…
  tools/emsdk/       (empty)                   tools/emsdk       @ c9ef2c9…
          │                                    submodules.lock.json
          ▼
ci/platform-images/actions/link-submodules
  for each baked submodule:
    git ls-tree HEAD <path>  ==  lock SHA ?
      yes ─► <path> -> /opt/ewe/submodules/<path>   (symlink, no network)
      no  ─► ::warning:: image stale; git submodule update --init --depth 1 <path>
```

The llama.cpp build script canonicalises `LLAMA_DIR`, so CMake builds from
`/opt/ewe/submodules/tools/llama.cpp` directly, and the
`infrastructure/llama-bindings/llama.cpp -> ../../tools/llama.cpp` symlink
resolves through it.

`cargo publish` refuses a tree whose gitlinks were replaced by symlinks, so
the publish dry-run uses `mode: submodule`: a normal submodule checkout,
cloned from the baked copy (still no network).

Locally (outside CI), use plain submodules as before:
`git submodule update --init tools/llama.cpp` (or `mise run tools:llama:init`).

## Updating

**Bump a submodule:** commit the new gitlink as usual
(`git -C tools/llama.cpp checkout <sha> && git add tools/llama.cpp`). The
gitlink SHA is part of the layer 2 hash, so the first CI run on that commit
builds and pushes an image with the new sources (in minutes: only the
submodule layer and the ones after it are rebuilt). Until a job runs in that
image, `link-submodules` fetches the new SHA and warns.

**Bump the toolchain:** edit `rust-toolchain.toml`. It's hashed, so the
next run builds an image with the new nightly.

**Bump a tool:** edit `mise.toml`, then `mise lock --platform
linux-x64,linux-arm64,macos-arm64,windows-x64` to refresh `mise.lock` (needs
GitHub API access). For wasm-bindgen, keep it equal to the `wasm-bindgen`
crate in Cargo.lock (the image build fails otherwise).

**Bake another submodule / change build args:** edit [`config.toml`](config.toml).
**Change what gets installed:** edit [`Dockerfile`](Dockerfile).

Any of these, as well as a dependency change in `Cargo.lock`, rebuilds
layer 2 on its next CI run. Layer 1 rebuilds only for changes under
`ci/platform-images/`, and weekly (`no-cache`) for security updates.

## Rebuilding by hand

- Actions → *Platform images (publish)* → Run workflow (`force` rebuilds even
  when the hash tags exist; `no-cache` also refreshes every apt package).
- Locally: see [Building locally](../platform-images/README.md#building-locally).

## Size

Measured on a local build without browsers (the vendor repositories were
unreachable from the build machine): layer 1 is 4.8 GB unpacked, and layer 2
adds the toolchain, tools, submodules and registry on top (see the PR for
the measured total). Chrome and Firefox add roughly 0.6 GB. The CUDA flavor's
upstream image alone is about 5.2 GB compressed.
