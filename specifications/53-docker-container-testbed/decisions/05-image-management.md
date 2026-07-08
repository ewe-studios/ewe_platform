# 05 — Image Management

**Date:** 2026-07-08
**Status:** Resolved

## Decision

Provide Docker images via **two complementary paths**: (1) multi-stage
Dockerfiles checked into the crate that define each profile's build environment,
and (2) pre-built images pushed to a container registry (GitHub Container
Registry or similar) for fast cold-start on CI. Image resolution follows the
same priority chain as the existing QEMU image import: environment variable
override → local cache (Docker image cache) → pull from registry → build from
Dockerfile.

## Table of Contents

1. [Dockerfile strategy](#dockerfile-strategy)
2. [Image profiles](#image-profiles)
3. [Image resolution chain](#image-resolution-chain)
4. [Dockerfile locations](#dockerfile-locations)
5. [Registry and versioning](#registry-and-versioning)
6. [dockurr image handling](#dockurr-image-handling)

---

## Dockerfile strategy

### Multi-stage builds for each profile

Each profile gets a directory with its Dockerfile:

```
foundation_testbed/
  docker/
    linux-build/
      Dockerfile
      testbed_key.pub         # SSH public key injected at build time
    linux-test/
      Dockerfile
      testbed_key.pub
    scripts/                  # Shared provisioning scripts (mounts into containers)
      install_mise.sh
      install_dev_deps.sh
      bootstrap_user.sh
```

### linux-build Dockerfile (example)

```dockerfile
# Stage 1: Build the testbed SSH key (separate for cacheability)
FROM alpine:latest AS key-stage
COPY testbed_key.pub /tmp/key.pub

# Stage 2: The actual test environment
FROM ubuntu:24.04

# Avoid interactive prompts during package install
ENV DEBIAN_FRONTEND=noninteractive

# System dependencies (matches what QEMU bootstrap installs)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential curl pkg-config \
    libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev librsvg2-dev \
    libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
    openssh-server sudo ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Configure SSH
RUN mkdir /run/sshd \
    && echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config \
    && echo 'PasswordAuthentication yes' >> /etc/ssh/sshd_config \
    && echo 'PubkeyAuthentication yes' >> /etc/ssh/sshd_config

# Inject testbed SSH key
COPY --from=key-stage /tmp/key.pub /root/.ssh/authorized_keys
RUN chmod 600 /root/.ssh/authorized_keys && chmod 700 /root/.ssh

# Install mise (Rust toolchain manager) — same as QEMU bootstrap
RUN curl https://mise.run | sh
ENV PATH="/root/.local/bin:${PATH}"

# Pre-install common Rust targets for faster cold starts
RUN mise use -g rust@latest \
    && rustup target add x86_64-unknown-linux-gnu wasm32-unknown-unknown

# Create workspace mount point
RUN mkdir -p /mnt/project

# Expose SSH
EXPOSE 22

# Run SSH daemon as the main process
CMD ["/usr/sbin/sshd", "-D", "-e"]
```

### Key design choices

1. **SSH daemon as PID 1** — Matches the QEMU model. Containers that run SSH
   as the main process fit the existing `Provider` abstraction perfectly. The
   alternative (`docker exec` for everything) would require rewriting every
   guest-interaction module.

2. **Pre-installed Rust toolchain** — The Docker image bakes in the Rust
   toolchain and common targets, avoiding the multi-minute bootstrap wait that
   QEMU VMs require. `mise` is still available for project-specific toolchains.

3. **No nushell dependency** — Unlike the QEMU bootstrap (which installs
   nushell as the default shell), the Docker image uses bash. nushell is
   available via mise if a project needs it, but is not required for the
   base environment.

4. **SSH key injected at build time** — The testbed's public key is `COPY`ed
   into the image. This means containers start with passwordless SSH access.
   No key injection step at runtime.

---

## Image profiles

| Profile | Image | Build context | Pull source |
|---------|-------|--------------|-------------|
| `linux-build` | `testbed/linux-build:latest` | `docker/linux-build/` | `ghcr.io/ewe-platform/testbed-linux-build:latest` |
| `linux-test` | `testbed/linux-test:latest` | `docker/linux-test/` | `ghcr.io/ewe-platform/testbed-linux-test:latest` |
| `windows-build` | `dockurr/windows` (upstream) | None (dockurr) | Docker Hub |
| `windows-test` | `dockurr/windows` (upstream) | None (dockurr) | Docker Hub |

For Windows, we don't build custom images — we use `dockurr/windows` directly
and bootstrap it at runtime (same two-phase WinRM → SSH flow as today). The
Dockerfile-based approach is Linux-only.

---

## Image resolution chain

Following the same priority chain as QEMU's `ensure_image()`:

```
1. TESTBED_IMAGE_LINUX_BUILD env var
   → Explicit override: "docker.io/my-custom/linux-build:v2"

2. Local Docker image cache
   → docker inspect testbed/linux-build:latest → exists?
   → If yes: use it (fast path — no network)

3. Registry pull
   → docker pull ghcr.io/ewe-platform/testbed-linux-build:latest
   → If succeeds: use the pulled image

4. Build from Dockerfile
   → docker build -t testbed/linux-build:latest -f docker/linux-build/Dockerfile .
   → If succeeds: use the built image, optionally push to registry

5. Error
   → "No image found for linux-build. Run 'testbed image build linux-build' first."
```

### Implementation (bollard path)

```rust
fn ensure_image(&self, profile: &ContainerProfile) -> Result<String> {
    // 1. Check env var override
    let image = std::env::var(format!("TESTBED_IMAGE_{}", profile.name.to_uppercase()))
        .unwrap_or_else(|_| profile.image.clone());

    // 2. Check if image exists locally
    if self.image_exists_locally(&image)? {
        tracing::info!(%image, "Image found in local cache");
        return Ok(image);
    }

    // 3. Try pulling from registry
    match self.pull_image(&image) {
        Ok(_) => {
            tracing::info!(%image, "Image pulled from registry");
            return Ok(image);
        }
        Err(e) => tracing::warn!(%image, %e, "Pull failed, will try local build"),
    }

    // 4. Build from Dockerfile
    if let Some(ref dockerfile) = profile.dockerfile {
        self.build_image(&image, dockerfile)?;
        tracing::info!(%image, "Image built from Dockerfile");
        return Ok(image);
    }

    // 5. Give up
    Err(TestbedError::ImageNotFound(image))
}
```

### Image existence check

```rust
fn image_exists_locally(&self, image: &str) -> Result<bool> {
    let images = rt().block_on(self.docker.list_images(Some(
        bollard::image::ListImagesOptions {
            filters: hashmap! { "reference".to_string() => vec![image.to_string()] },
            ..Default::default()
        },
    )))?;

    Ok(!images.is_empty())
}
```

---

## Dockerfile locations

Dockerfiles live in the crate, not in a separate repository. Rationale:

1. **Versioned with the code** — Changes to the build environment (new system
   deps, new Rust version) are atomic commits alongside the code that needs them.
2. **Build context is the testbed crate root** — `COPY` directives can reference
   scripts from `scripts/`, keys from `docker/*/testbed_key.pub`.
3. **No external dependency** — Users don't need to clone a separate repo to
   build images. `testbed image build linux-build` works from the crate.

---

## Registry and versioning

### Registry: GitHub Container Registry (ghcr.io)

- Free for public repos, built into GitHub Actions.
- Image: `ghcr.io/ewe-platform/testbed-linux-build:latest`
- Tagging strategy: `latest` for the current main branch build, `sha-<commit>`
  for pinned CI reproducibility.

### CI integration

```yaml
# .github/workflows/testbed-images.yml
jobs:
  build-images:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build and push linux-build
        run: |
          docker build -t ghcr.io/ewe-platform/testbed-linux-build:latest \
            -f backends/foundation_testbed/docker/linux-build/Dockerfile \
            backends/foundation_testbed/
          docker push ghcr.io/ewe-platform/testbed-linux-build:latest
```

### Local development override

```bash
# Build from local Dockerfile (with custom changes)
testbed image build linux-build

# Override with a custom image for testing
TESTBED_IMAGE_LINUX_BUILD=my-registry/custom-linux:dev testbed start linux-build
```

---

## dockurr image handling

dockurr images (`dockurr/windows`, `dockurr/macos`) are pulled from Docker Hub
directly — we don't build or republish them. The `ensure_image()` flow for
dockurr profiles:

1. Check local cache: `docker images dockurr/windows`
2. Pull if missing: `docker pull dockurr/windows`
3. No Dockerfile build step — dockurr doesn't provide one, and we wouldn't
   want to build Windows from scratch anyway.

The dockurr image tag pins a specific version for reproducibility:

```rust
const DOCKURR_WINDOWS_IMAGE: &str = "dockurr/windows:latest";
// Or pin: "dockurr/windows@sha256:..."
```
