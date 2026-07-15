#!/bin/bash
cat <<'MISE_EOF' > /tmp/bootstrap-mise.toml
{{MISE_TOML}}
MISE_EOF
export MISE_CONFIG_FILE=/tmp/bootstrap-mise.toml
$HOME/.local/bin/mise install
rm -f /tmp/bootstrap-mise.toml
