#!/bin/sh
# The entrypoint of the demo sandbox container.
#
# The platform starts this file with `--listen tcp:0.0.0.0:1024`. It starts the
# demo model server on the configured port and then replaces itself with the
# real static slave, so the slave is process identifier 1 and owns the signal
# handling.
set -eu

model_port="${DEMO_MODEL_PORT:-8081}"
/usr/local/bin/demo-model --listen "0.0.0.0:${model_port}" &
exec /usr/local/bin/agent-slave.real "$@"
