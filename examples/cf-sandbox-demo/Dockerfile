# The container image of the demo sandbox.
#
# The platform starts `/usr/local/bin/agent-slave` with
# `--listen tcp:0.0.0.0:1024`. That path is a wrapper script: it starts the real
# static slave and the demo model server, so one container holds both. The
# model server listens on `DEMO_MODEL_PORT` (default 8081) and the control API
# reaches it over the container port.
#
# `mise run build` copies the two programs here before `wrangler` builds the
# image. Both files are in `.gitignore`.
FROM alpine:3.22

COPY agent-slave /usr/local/bin/agent-slave.real
COPY demo-model /usr/local/bin/demo-model
COPY entrypoint.sh /usr/local/bin/agent-slave

RUN chmod +x /usr/local/bin/agent-slave /usr/local/bin/agent-slave.real /usr/local/bin/demo-model \
    && mkdir -p /workspace

CMD ["/usr/local/bin/agent-slave", "--listen", "tcp:0.0.0.0:1024"]
