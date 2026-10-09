# foundation_keychain — native Bitwarden-compatible vault server.
#
# Build from the WORKSPACE ROOT (path dependencies need the whole workspace):
#   docker build -f backends/foundation_keychain/Dockerfile -t foundation-keychain .
#
# Run:
#   docker run -p 8080:8080 -v keychain-data:/data foundation-keychain

FROM rust:1.87-bookworm AS builder
# turso_core + boring-sys build deps.
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake clang libclang-dev perl pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release -p foundation_keychain --bin keychain

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/keychain /usr/local/bin/keychain
ENV KEYCHAIN_DB=/data/keychain.db
ENV LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080
VOLUME /data
ENTRYPOINT ["keychain"]
