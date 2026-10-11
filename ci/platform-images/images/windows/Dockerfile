# syntax=docker/dockerfile:1.27.1
#
# platform-images, Windows VM image (layer 1): dockurr/windows (QEMU/KVM in a
# Linux container), with a provisioning step that runs inside the VM at the
# end of the unattended install. It installs the CI toolchain: OpenSSH,
# PowerShell 7, Git for Windows, Visual Studio Build Tools (MSVC), LLVM, the
# Vulkan SDK, WebView2, mise with the shared pinned tools (nushell, node,
# python, cmake, ninja, protoc, ...), rustup, and the GitHub Actions runner.
# Nothing repository-specific; a repository adds a layer under
# /opt/platform-images/layers (see ../../README.md).
#
# No Windows bits are in this image: dockurr downloads the ISO from Microsoft
# when the VM is first installed. Which ISO is the VERSION variable (default
# below: Windows Server 2022 Standard evaluation, 180 days). See the README
# for licensing.
#
# Context staged by `nu scripts/images.nu prepare --layer vm-base --os windows`.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

COPY guest/ /opt/platform-images/guest/
COPY --chmod=755 bin/ /opt/platform-images/bin/

ENV PI_VM_OS=windows \
    VERSION="2022" \
    RAM_SIZE="10G" \
    CPU_CORES="4" \
    DISK_SIZE="128G"

ENTRYPOINT ["/usr/bin/tini", "-s", "/opt/platform-images/bin/pi-vm-entry"]

LABEL org.opencontainers.image.title="platform-images windows VM" \
      org.opencontainers.image.description="dockurr/windows with unattended CI provisioning: MSVC, LLVM, Vulkan SDK, Git, PowerShell 7, mise tools, rustup, OpenSSH, GitHub Actions runner"
