# syntax=docker/dockerfile:1.27.1
#
# ewe_platform CI image (layer 2), FROM the platform-images base (layer 1)
# pinned by digest. It adds only what is specific to this repository:
#   - the nightly toolchain rust-toolchain.toml pins, its components and
#     targets, plus clippy, the Cranelift backend and the wasi targets
#   - the tools pinned in the repository's mise.toml (mise.lock)
#   - the submodules the build needs, at their gitlink SHAs
#   - a warm cargo registry for Cargo.lock
#
# The build context is staged by
#   nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml --layer image ...
# (files/, mise/, submodules.lock.json, cargo-workspace/); see README.md.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

ARG SUBMODULES_DIR=/opt/ewe/submodules
ARG RUST_EXTRA_COMPONENTS=""
ARG RUST_EXTRA_TARGETS=""

SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

# Toolchain from rust-toolchain.toml, made the default so cargo works anywhere
# in the image. CI still sets RUSTUP_TOOLCHAIN to the same channel.
COPY files/rust-toolchain.toml /opt/ewe/rust-toolchain.toml
# (`[a b]` is a nushell list literal; the build args are space separated.)
RUN nu /opt/platform-images/scripts/install-rust-toolchain.nu /opt/ewe/rust-toolchain.toml \
      --components "[${RUST_EXTRA_COMPONENTS}]" \
      --targets "[${RUST_EXTRA_TARGETS}]"

# Repository tools (wasm-bindgen matching Cargo.lock, cargo-nextest, ...),
# put ahead of the base image's tools on PATH. cargo:* tools come through
# cargo-binstall (from the base image), building from source only if needed.
ENV PATH=/opt/ewe/tools/bin:${PATH}
COPY mise/ /opt/ewe/mise/
RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    pi-mise-install /opt/ewe/mise /opt/ewe/tools/bin \
 && rm -rf /var/cache/mise /opt/rust/cargo/registry/src

# Submodules at the pinned SHAs, and the lock link-submodules.nu checks.
COPY submodules.lock.json /opt/ewe/submodules.lock.json
RUN nu /opt/platform-images/scripts/bake-submodules.nu /opt/ewe/submodules.lock.json "${SUBMODULES_DIR}"
ENV PLATFORM_IMAGES_SUBMODULES_LOCK=${SUBMODULES_DIR}/submodules.lock.json

# Warm cargo registry: fetch every crate in Cargo.lock (for all targets) from a
# manifest-only skeleton of the workspace, then drop the skeleton and the
# unpacked sources (cargo unpacks again from the cached .crate files).
COPY cargo-workspace/ /tmp/cargo-workspace/
RUN cd /tmp/cargo-workspace \
 && cargo fetch --locked \
 && rm -rf /tmp/cargo-workspace "${CARGO_HOME}/registry/src"

LABEL org.opencontainers.image.title="ewe_platform CI" \
      org.opencontainers.image.source="https://github.com/ewe-studios/ewe_platform"
