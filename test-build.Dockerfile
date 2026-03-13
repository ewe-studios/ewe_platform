ARG CUDA_VERSION=12.3.1
ARG UBUNTU_VERSION=22.04
FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-devel-ubuntu${UBUNTU_VERSION} AS base-cuda

# Install requirements for rustup install + bindgen: https://rust-lang.github.io/rust-bindgen/requirements.html
RUN DEBIAN_FRONTEND=noninteractive apt update -y && apt install -y curl llvm-dev libclang-dev clang pkg-config libssl-dev cmake git build-essential g++ libz-dev libssl-dev libelf-dev
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH=/root/.cargo/bin:$PATH

# Build and install mold linker
RUN git clone --branch stable --depth=1 https://github.com/rui314/mold.git /tmp/mold && \
    cd /tmp/mold && \
    cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_COMPILER=c++ -B build && \
    cmake --build build -j$(nproc) && \
    cmake --build build --target install && \
    rm -rf /tmp/mold

# Configure cargo to use clang with mold linker
RUN mkdir -p /root/.cargo && echo '[target.x86_64-unknown-linux-gnu]\nlinker = "clang"\nrustflags = ["-C", "link-arg=-fuse-ld=mold"]' > /root/.cargo/config.toml

COPY . .
RUN cargo build --bin simple --features cuda

FROM nvcr.io/nvidia/cuda:${CUDA_VERSION}-runtime-ubuntu${UBUNTU_VERSION} AS base-cuda-runtime

COPY --from=base-cuda /target/debug/simple /usr/local/bin/simple

ENTRYPOINT ["/usr/local/bin/simple"]
