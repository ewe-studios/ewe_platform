#!/bin/bash
mkdir -p /home/vagrant/.config/mise
touch /home/vagrant/.config/mise/config.toml
grep -q 'cargo_binstall' /home/vagrant/.config/mise/config.toml || printf '\n[settings]\ncargo_binstall = true\n' >> /home/vagrant/.config/mise/config.toml
