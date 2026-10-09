#!/bin/bash
mkdir -p /home/vagrant/.config/mise
touch /home/vagrant/.config/mise/config.toml

# Add [tools] section if not present
if ! grep -q '^\[tools\]$' /home/vagrant/.config/mise/config.toml; then
    cat > /home/vagrant/.config/mise/config.toml << 'EOF'
[tools]
rust = "stable"
"aqua:nushell/nushell" = "latest"
cargo-binstall = "latest"
sccache = "latest"
"cargo:tauri-cli" = "2"

EOF
fi

# Remove old invalid cargo_binstall setting if present (from [settings] section)
sed -i '/^cargo_binstall = true$/d' /home/vagrant/.config/mise/config.toml

# Add cargo.binstall setting if not present
grep -q 'cargo\.binstall' /home/vagrant/.config/mise/config.toml || {
    # Add cargo.binstall setting - check if [settings] section exists
    if grep -q '^\[settings\]$' /home/vagrant/.config/mise/config.toml; then
        # Add under existing [settings] section
        sed -i '/^\[settings\]$/a cargo.binstall = true' /home/vagrant/.config/mise/config.toml
    else
        # Add new [settings] section
        printf '\n[settings]\ncargo.binstall = true\n' >> /home/vagrant/.config/mise/config.toml
    fi
}
