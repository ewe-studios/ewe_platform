# WireGuard mesh node — Docker container image (spec-55, F10).
#
# Build:  docker build -t ewe-wireguard -f backends/foundation_wireguard/Dockerfile .
# Run:    docker run --rm -e WG_SECRET=<seed> ewe-wireguard

FROM rust:1.91-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang lld pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace sources (only what's needed to build foundation_wireguard)
COPY Cargo.toml Cargo.lock ./
COPY backends/ backends/
COPY bin/ bin/
COPY infrastructure/ infrastructure/
COPY tests/ tests/
COPY benches/ benches/

# Remove unstable Cargo features (codegen-backend requires nightly) and pin
# workspace members to only backends/ (the other globs fail if empty).
RUN sed -i '/^codegen-backend/d' Cargo.toml \
    && sed -i '/members = \[/,/\]/c\members = ["backends/*"]' Cargo.toml

# Docker-friendly cargo config — no unstable nightly features
RUN mkdir -p .cargo
COPY backends/foundation_wireguard/.docker-cargo-config.toml .cargo/config.toml

# Build the example binary
RUN cargo build --package foundation_wireguard --example docker_self_assemble --release

# ── Final image ───────────────────────────────────────────────────────────

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/examples/docker_self_assemble /usr/local/bin/ewe-wireguard

EXPOSE 51820/udp

ENTRYPOINT ["/usr/local/bin/ewe-wireguard"]
