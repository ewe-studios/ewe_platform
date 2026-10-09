#!/bin/bash
mkdir -p /home/vagrant/.cargo/bin
curl -sSfL "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-${TARGET}.tgz" | tar -xz -C /home/vagrant/.cargo/bin
chmod +x /home/vagrant/.cargo/bin/cargo-binstall
