# syntax=docker/dockerfile:1.27.1
#
# platform-images, macOS VM image (layer 1): dockurr/macos (QEMU/KVM in a
# Linux container) plus the CI provisioning for the guest.
#
# LICENSING: Apple's macOS licence only permits running macOS on Apple
# hardware. Run this image only on a Linux host that is itself Apple
# hardware (e.g. an Intel Mac running Linux, registered as a self-hosted
# runner), never on GitHub-hosted runners or other non-Apple machines.
# dockurr/macos says the same. No macOS bits are in this image: dockurr
# downloads the installer from Apple's servers on first boot.
#
# dockurr/macos has no unattended install. The install is a one-time manual
# step over VNC (see ../../README.md); afterwards `nu scripts/vm.nu provision`
# copies guest/ into the VM over SSH and runs guest/provision.sh, which
# installs the CI toolchain: Xcode Command Line Tools, mise with the shared
# pinned tools, rustup, the Vulkan SDK (MoltenVK) and the GitHub Actions
# runner. Repository layers add /opt/platform-images/layers/<name>/install.sh.
#
# Context staged by `nu scripts/images.nu prepare --layer vm-base --os macos`.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

COPY guest/ /opt/platform-images/guest/
COPY --chmod=755 bin/ /opt/platform-images/bin/

ENV PI_VM_OS=macos \
    VERSION="15" \
    RAM_SIZE="8G" \
    CPU_CORES="4" \
    DISK_SIZE="128G"

ENTRYPOINT ["/usr/bin/tini", "-s", "/opt/platform-images/bin/pi-vm-entry"]

LABEL org.opencontainers.image.title="platform-images macos VM" \
      org.opencontainers.image.description="dockurr/macos with CI provisioning (Xcode CLT, mise tools, rustup, Vulkan SDK, GitHub Actions runner). Apple hardware only."
