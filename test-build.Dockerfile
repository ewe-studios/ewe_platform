ARG CUDA_VERSION=12.3.1
ARG UBUNTU_VERSION=22.04
ARG TARGETPLATFORM=linux/amd64
FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-devel-ubuntu${UBUNTU_VERSION} AS base-cuda

# Install requirements for rustup install + bindgen
RUN DEBIAN_FRONTEND=noninteractive apt update -y && apt install -y curl llvm-dev libclang-dev clang pkg-config libssl-dev cmake git build-essential g++ libz-dev libssl-dev libelf-dev python3 python3-pip
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH=/root/.cargo/bin:$PATH

# Set mold target based on Docker platform and install mold
COPY scripts/install-mold.sh /tmp/install-mold.sh
RUN chmod +x /tmp/install-mold.sh && \
    if [ "$TARGETPLATFORM" = "linux/arm64" ] || [ "$TARGETPLATFORM" = "linux/arm/v7" ]; then \
        MOLD_TARGET=arm /tmp/install-mold.sh; \
    else \
        /tmp/install-mold.sh; \
    fi && \
    rm /tmp/install-mold.sh

# Create tools directory and setup emsdk
RUN mkdir -p /tools && \
    git clone --depth=1 https://github.com/emscripten-core/emsdk.git /tools/emsdk && \
    cd /tools/emsdk && \
    ./emsdk install latest && \
    ./emsdk activate latest

# Setup dawn
RUN git clone --depth=1 https://github.com/google/dawn.git /tools/dawn

# Configure cargo to use clang with mold linker (platform-specific)
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
        mkdir -p /root/.cargo && \
        echo -e '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"\nrustflags = ["-C", "link-arg=-fuse-ld=mold"]' > /root/.cargo/config.toml; \
    elif [ "$TARGETPLATFORM" = "linux/arm/v7" ]; then \
        mkdir -p /root/.cargo && \
        echo -e '[target.armv7-unknown-linux-gnueabihf]\nlinker = "arm-linux-gnueabihf-gcc"\nrustflags = ["-C", "link-arg=-fuse-ld=mold"]' > /root/.cargo/config.toml; \
    else \
        mkdir -p /root/.cargo && \
        echo -e '[target.x86_64-unknown-linux-gnu]\nlinker = "clang"\nrustflags = ["-C", "link-arg=-fuse-ld=mold"]' > /root/.cargo/config.toml; \
    fi

COPY . .

# Create tools directory with symlinks to match .cargo/config.toml paths
RUN mkdir -p tools && \
    ln -sf /tools/emsdk tools/emsdk && \
    ln -sf /tools/dawn tools/dawn

# Set environment variables for build
ENV EMSDK_DIR=/tools/emsdk
ENV PATH=/tools/emsdk:/tools/emsdk/upstream/emscripten:$PATH

# Build for the target platform (CUDA only available on x86_64)
RUN if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
        cargo build --bin simple --features cuda; \
    else \
        cargo build --bin simple; \
    fi

FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu${UBUNTU_VERSION} AS base-cuda-runtime

COPY --from=base-cuda /target/debug/simple /usr/local/bin/simple

ENTRYPOINT ["/usr/local/bin/simple"]
