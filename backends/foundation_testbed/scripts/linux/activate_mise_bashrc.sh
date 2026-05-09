#!/bin/bash
grep -q 'mise activate' ~/.bashrc || echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc
