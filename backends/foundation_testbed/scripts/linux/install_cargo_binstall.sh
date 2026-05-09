#!/bin/bash
mkdir -p ~/.cargo/bin
curl -sSfL "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-${TARGET}.tgz" | tar -xz -C ~/.cargo/bin
chmod +x ~/.cargo/bin/cargo-binstall
