# =============================================================================
# CI test image — prebuilds Dawn (via depot_tools), emsdk, and Rust toolchain
# so the `check` workflow can skip multi-minute `gclient sync` on every run.
#
# Build + push manually (or via .github/workflows/publish-ci-image.yaml):
#   docker build -f ci-test.Dockerfile -t <user>/ewe-platform-ci:latest .
#   docker push <user>/ewe-platform-ci:latest
# =============================================================================
ARG UBUNTU_VERSION=22.04
FROM ubuntu:${UBUNTU_VERSION}

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl ca-certificates git pkg-config cmake \
        libssl-dev libclang-dev clang gcc g++ mold \
        python3 python3-pip \
        libz-dev libelf-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust toolchain (stable + clippy + rustfmt)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        bash -s -- -y --default-toolchain stable --profile minimal \
            --component clippy --component rustfmt
ENV PATH=/root/.cargo/bin:$PATH

# depot_tools — needed for gclient
RUN git clone --depth=1 \
        https://chromium.googlesource.com/chromium/tools/depot_tools.git \
        /opt/depot_tools
ENV PATH=/opt/depot_tools:$PATH

# Dawn — clone + gclient sync so the tree is fully hydrated at image build time
RUN git clone https://dawn.googlesource.com/dawn /opt/dawn && \
    cp /opt/dawn/scripts/standalone.gclient /opt/dawn/.gclient && \
    (cd /opt/dawn && gclient sync --no-history)

# emsdk (kept in sync with prior test-build.Dockerfile)
RUN git clone --depth=1 https://github.com/emscripten-core/emsdk.git /opt/emsdk && \
    cd /opt/emsdk && \
    ./emsdk install latest && \
    ./emsdk activate latest
ENV EMSDK_DIR=/opt/emsdk
ENV PATH=/opt/emsdk:/opt/emsdk/upstream/emscripten:$PATH

# The `check` workflow checks out the repo into $GITHUB_WORKSPACE and expects
# tools/dawn + tools/emsdk to exist relative to the workspace. The workflow
# creates symlinks at job start — see .github/workflows/check.yaml.

WORKDIR /workspace
