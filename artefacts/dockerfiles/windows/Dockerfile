# ewe-test-windows — Windows test environment for foundation_platform.
#
# Extends ewestudios/windows (dockurr/windows copied to our Docker Hub).
# dockurr/windows runs Windows 11 inside QEMU in a Docker container.
# VNC is provided by QEMU (the VM display is the VNC output).
# First-boot provisioning is done via /oem/install.bat.
#
# Pattern from: artefact/uncloud/recipes/dev-windows/Dockerfile
# Base from:    ewestudios/windows (originally dockurr/windows)
FROM ewestudios/windows:latest

# /oem/install.bat is executed by dockurr once, after Windows setup completes.
# It installs Rust, Edge WebView2, OpenSSH, and the test runner.
COPY oem/ /oem/
