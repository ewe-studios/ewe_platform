# syntax=docker/dockerfile:1.27.1
#
# ewe_platform's macOS CI VM image (layer 2), FROM ewestudios/ci-macos
# (platform-images layer 1) pinned by digest. It only adds ewe_platform's
# provisioning layer, run inside the VM after layer 1 by `vm.nu provision`
# (see layer/install.sh). Apple hardware only: see the platform-images
# README on licensing.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

COPY layer/ /opt/platform-images/layers/50-ewe-platform/
