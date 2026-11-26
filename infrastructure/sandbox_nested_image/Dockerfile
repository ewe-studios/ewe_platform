# The nested host image (spec 77 feature 11, section 6.4).
#
# A slave in this image also serves the control plane, and it runs Firecracker
# micro virtual machines (VMs). The image holds the pinned Firecracker program,
# the pinned kernel and the continuous integration (CI) root file system.
#
# The kernel and the root file system sit in the artifact cache at the paths
# that the artifact store reads. So the container downloads nothing at run time.
# The Docker driver injects `agent-slave` at container start; this image does
# not hold it.
#
# Build it with `mise run sandbox:nested-image`. The task reads the version and
# the SHA-256 of the x86_64 release from `ci/sandbox.toml`.
FROM debian:stable-slim

# The Firecracker release. The default SHA-256 is the published x86_64 value
# from `ci/sandbox.toml`. An arm64 build must pass the arm64 value.
ARG FIRECRACKER_VERSION=1.15.0
ARG FIRECRACKER_SHA256=00cadf7f21e709e939dc0c8d16e2d2ce7b975a62bec6c50f74b421cc8ab3cab4

# The artifact versions of `src/native/artifacts/manifest.rs`.
ARG FIRECRACKER_CI_PREFIX=v1.15
ARG FIRECRACKER_KERNEL_VERSION=6.1.155
ARG FIRECRACKER_ROOTFS_VERSION=24.04

# The architecture word of the official Firecracker release for the build
# architecture.
ARG TARGETARCH=amd64

RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl; \
    rm -rf /var/lib/apt/lists/*; \
    case "${TARGETARCH}" in \
        amd64) \
            fc_arch=x86_64; \
            kernel_sha=e20e46d0c36c55c0d1014eb20576171b3f3d922260d9f792017aeff53af3d4f2; \
            rootfs_sha=68321e0482baeb3844dafe8a6b08a6902401a7afc41fbfd8c3d9ea08aadd244f; \
            ;; \
        arm64) \
            fc_arch=aarch64; \
            kernel_sha=e3544b10603acbf3db492cb52e000d22ba202cb4b63b9add027565683e11c591; \
            rootfs_sha=0efb6a3ff2982baa6ca7e3d940966516ba7ddd2df5deb3e6c2161d369a15d608; \
            ;; \
        *) \
            echo "unsupported architecture: ${TARGETARCH}" >&2; \
            exit 1; \
            ;; \
    esac; \
    curl -fsSL -o /tmp/firecracker.tgz \
        "https://github.com/firecracker-microvm/firecracker/releases/download/v${FIRECRACKER_VERSION}/firecracker-v${FIRECRACKER_VERSION}-${fc_arch}.tgz"; \
    echo "${FIRECRACKER_SHA256}  /tmp/firecracker.tgz" | sha256sum -c -; \
    tar -xzf /tmp/firecracker.tgz -C /tmp; \
    install -m 0755 \
        "/tmp/release-v${FIRECRACKER_VERSION}-${fc_arch}/firecracker-v${FIRECRACKER_VERSION}-${fc_arch}" \
        /usr/local/bin/firecracker; \
    rm -rf /tmp/firecracker.tgz "/tmp/release-v${FIRECRACKER_VERSION}-${fc_arch}"; \
    kernel_dir="/var/cache/ewe-sandbox/file/$(printf '%s' "${kernel_sha}" | cut -c1-16)"; \
    rootfs_dir="/var/cache/ewe-sandbox/file/$(printf '%s' "${rootfs_sha}" | cut -c1-16)"; \
    mkdir -p "${kernel_dir}" "${rootfs_dir}"; \
    curl -fsSL -o "${kernel_dir}/artifact" \
        "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/${FIRECRACKER_CI_PREFIX}/${fc_arch}/vmlinux-${FIRECRACKER_KERNEL_VERSION}"; \
    echo "${kernel_sha}  ${kernel_dir}/artifact" | sha256sum -c -; \
    touch "${kernel_dir}/.ready"; \
    curl -fsSL -o "${rootfs_dir}/artifact" \
        "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/${FIRECRACKER_CI_PREFIX}/${fc_arch}/ubuntu-${FIRECRACKER_ROOTFS_VERSION}.squashfs"; \
    echo "${rootfs_sha}  ${rootfs_dir}/artifact" | sha256sum -c -; \
    touch "${rootfs_dir}/.ready"

# The container stays up. The Docker driver starts the slave in it.
CMD ["sleep", "infinity"]
