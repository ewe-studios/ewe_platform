# The control plane in a container (spec 77 feature 10, section 6.8).
#
# Build the programs first, then copy them into this directory (see README.md):
#   agent-sandbox
#   agent-slave-x86_64 and agent-slave-aarch64
FROM debian:stable-slim

COPY agent-sandbox /usr/local/bin/agent-sandbox
COPY agent-slave-x86_64 agent-slave-aarch64 /usr/local/bin/

# The control plane and the daemon must see the same run directory path.
ENV EWE_SANDBOX_STATE_DIR=/var/run/ewe-sbx

ENTRYPOINT ["/usr/local/bin/agent-sandbox", "serve", "--listen", "tcp:0.0.0.0:7070", "--token-file", "/run/secrets/sandbox_token"]
