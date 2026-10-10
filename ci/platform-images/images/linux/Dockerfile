# syntax=docker/dockerfile:1.27.1
#
# platform-images, Linux base image (layer 1).
#
# A generic CI/build environment any repository can build FROM: compilers and
# build tools, the GTK/WebKit desktop stack, Mesa GL/EGL/GLES + Vulkan
# (lavapipe) + Xvfb for GPU-less graphics tests, headless browsers, mise with
# a pinned tool set (nushell, node, deno, cmake, ninja, protoc, sccache,
# binaryen, wasm-pack, wasmtime, ...), and rustup without a toolchain.
# Nothing repository-specific: no Rust toolchain, no submodules, no repo paths.
# A repository adds those in its own layer 2 (see ../../README.md).
#
# The build context is staged by `nu scripts/images.nu prepare --layer base`;
# it contains this file plus packages/, mise/, bin/ and scripts/.
#
# Flavors differ only in BASE_IMAGE: ubuntu:24.04 (default) or an
# nvidia/cuda *-devel-ubuntu24.04 image (CUDA toolkit for GPU builds).

ARG BASE_IMAGE=docker.io/library/ubuntu:24.04
FROM ${BASE_IMAGE}

# Space separated: chrome, firefox (see bin/pi-install-browsers).
ARG BROWSERS="chrome firefox"
ARG MISE_VERSION=2026.9.18
ARG MISE_SHA256=4312f8fd72a8d6a869cd2aca7444929e2a0ef6f45d2c6f2866a1eacc5bdc2e84
ARG RUSTUP_VERSION=1.29.0
ARG RUSTUP_SHA256=4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10
ARG TARGETARCH

SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    TZ=Etc/UTC

RUN if [ "${TARGETARCH:-amd64}" != "amd64" ]; then \
      echo "platform-images: only linux/amd64 is supported (got ${TARGETARCH})" >&2; exit 1; \
    fi

# System packages: every packages/*.txt, one name per line, `#` comments.
COPY packages/ /opt/platform-images/packages/
RUN apt-get update \
 && sed -e 's/#.*//' /opt/platform-images/packages/*.txt | tr -s ' \t\n' '\n' | grep -v '^$' \
      | xargs apt-get install -y --no-install-recommends \
 && rm -rf /var/lib/apt/lists/*

COPY bin/ /opt/platform-images/bin/
RUN /opt/platform-images/bin/pi-install-browsers ${BROWSERS}

# mise, verified against its release checksum.
RUN curl -fsSL -o /tmp/mise.tar.gz \
      "https://github.com/jdx/mise/releases/download/v${MISE_VERSION}/mise-v${MISE_VERSION}-linux-x64.tar.gz" \
 && echo "${MISE_SHA256}  /tmp/mise.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/mise.tar.gz -C /tmp \
 && install -m 0755 /tmp/mise/bin/mise /usr/local/bin/mise \
 && rm -rf /tmp/mise /tmp/mise.tar.gz \
 && mise --version

# Pinned tools from mise/mise.toml + mise.lock, with wrappers in
# /opt/platform-images/tools/bin. A GITHUB_TOKEN build secret is optional:
# locked installs don't need the GitHub API, but mise uses it if a tool
# without a lock entry for this platform has to be resolved.
ENV MISE_DATA_DIR=/opt/mise \
    MISE_CACHE_DIR=/var/cache/mise \
    MISE_CONFIG_DIR=/etc/mise \
    MISE_VERSION_CHECK=0 \
    PATH=/opt/platform-images/tools/bin:/opt/platform-images/bin:${PATH}
COPY mise/ /opt/platform-images/mise/
RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    /opt/platform-images/bin/pi-mise-install /opt/platform-images/mise /opt/platform-images/tools/bin \
 && rm -rf /var/cache/mise \
 && nu --version

# rustup, no toolchain: each repository installs its own pinned toolchain in
# layer 2 (scripts/install-rust-toolchain.nu).
ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH=/opt/rust/cargo/bin:/opt/platform-images/tools/bin:/opt/platform-images/bin:${PATH}
RUN curl -fsSL -o /tmp/rustup-init \
      "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" \
 && echo "${RUSTUP_SHA256}  /tmp/rustup-init" | sha256sum -c - \
 && chmod +x /tmp/rustup-init \
 && /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain none \
 && rm /tmp/rustup-init \
 && rustup --version

# The platform-images nushell scripts (link-submodules.nu, ...), so layer 2
# Dockerfiles and jobs inside the image can call them.
COPY scripts/ /opt/platform-images/scripts/

# Graphics without a GPU: Vulkan uses lavapipe, GL/EGL uses llvmpipe. Override
# (or unset) these to use a real GPU, e.g. with the CUDA flavor on a GPU host.
# For a display, wrap the command: `xvfb-run -a <command>`.
ENV VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
    VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json \
    LIBGL_ALWAYS_SOFTWARE=1 \
    PLATFORM_IMAGES_HOME=/opt/platform-images

LABEL org.opencontainers.image.title="platform-images linux base" \
      org.opencontainers.image.description="Generic CI/build base: compilers, GTK/WebKit, Mesa/Vulkan/Xvfb, headless browsers, mise tools, rustup"
