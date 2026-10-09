#!/bin/bash
grep -q 'mise activate' ~/.zprofile || echo 'eval "$($HOME/.local/bin/mise activate zsh)"' >> ~/.zprofile
grep -q 'mise activate' ~/.bashrc 2>/dev/null || echo 'eval "$($HOME/.local/bin/mise activate bash)"' >> ~/.bashrc 2>/dev/null || true
