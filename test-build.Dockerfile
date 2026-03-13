ARG CUDA_VERSION=12.3.1
ARG UBUNTU_VERSION=22.04
FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-devel-ubuntu${UBUNTU_VERSION} AS base-cuda

# Install requirements for rustup install + bindgen
RUN DEBIAN_FRONTEND=noninteractive apt update -y && apt install -y curl llvm-dev libclang-dev clang pkg-config libssl-dev cmake git build-essential g++ libz-dev libssl-dev libelf-dev python3 python3-pip
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH=/root/.cargo/bin:$PATH

# Build and install mold linker
RUN git clone --branch stable --depth=1 https://github.com/rui314/mold.git /tmp/mold && \
    cd /tmp/mold && \
    cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_COMPILER=c++ -B build && \
    cmake --build build -j$(nproc) && \
    cmake --build build --target install && \
    rm -rf /tmp/mold

# Create tools directory and setup emsdk
RUN mkdir -p /tools && \
    git clone --depth=1 https://github.com/emscripten-core/emsdk.git /tools/emsdk && \
    cd /tools/emsdk && \
    ./emsdk install latest && \
    ./emsdk activate latest

# Setup dawn
RUN git clone --depth=1 https://github.com/google/dawn.git /tools/dawn

# Configure cargo to use clang with mold linker
RUN mkdir -p /root/.cargo && cat > /root/.cargo/config.toml << 'EOF'
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
EOF

COPY . .

# Create tools directory with symlinks to match .cargo/config.toml paths
RUN mkdir -p tools && \
    ln -sf /tools/emsdk tools/emsdk && \
    ln -sf /tools/dawn tools/dawn

# Set environment variables for build
ENV EMSDK_DIR=/tools/emsdk
ENV PATH=/tools/emsdk:/tools/emsdk/upstream/emscripten:$PATH

RUN cargo build --bin simple --features cuda

FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu${UBUNTU_VERSION} AS base-cuda-runtime

COPY --from=base-cuda /target/debug/simple /usr/local/bin/simple

ENTRYPOINT ["/usr/local/bin/simple"]
