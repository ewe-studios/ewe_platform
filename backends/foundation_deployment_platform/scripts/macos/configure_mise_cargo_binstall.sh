#!/bin/bash
mkdir -p ~/.config/mise
touch ~/.config/mise/config.toml
grep -q 'cargo_binstall' ~/.config/mise/config.toml || printf '\n[settings]\ncargo_binstall = true\n' >> ~/.config/mise/config.toml
