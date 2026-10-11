# syntax=docker/dockerfile:1.27.1
#
# ewe_platform's Windows CI VM image (layer 2), FROM ewestudios/ci-windows
# (platform-images layer 1) pinned by digest. It only adds ewe_platform's
# provisioning layer, which runs inside the VM after layer 1 (see
# layer/install.ps1): the pinned nightly, the repository's mise tools, and
# the submodules at their gitlink SHAs.
#
# Context staged by `nu ci/platform-images/scripts/images.nu prepare --layer vm-image --os windows ...`:
# layer/ holds install.ps1 plus files/, mise/ and submodules.lock.json.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

COPY layer/ /opt/platform-images/layers/50-ewe-platform/
