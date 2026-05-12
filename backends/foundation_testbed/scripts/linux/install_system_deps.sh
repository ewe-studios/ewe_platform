#!/bin/bash
# System dependencies for building Tauri apps on Debian/Ubuntu
# The DEPS placeholder is replaced by bootstrap_linux.rs
sudo apt-get update -qq
DEBIAN_FRONTEND=noninteractive sudo apt-get install -y {{DEPS}}
