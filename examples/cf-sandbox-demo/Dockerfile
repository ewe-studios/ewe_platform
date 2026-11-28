# The container image of the demo sandbox.
#
# `entrypoint.sh` starts the demo model server and then the static slave, so
# one container holds both. The model server listens on `DEMO_MODEL_PORT`
# (default 8081) and the control API reaches it over the container port.
#
# `mise run build` copies both programs here before `wrangler` builds the
# image. They are in `.gitignore`.
FROM alpine:3.22

COPY agent-slave /usr/local/bin/agent-slave
COPY demo-model /usr/local/bin/demo-model
COPY entrypoint.sh /entrypoint.sh

RUN chmod +x /entrypoint.sh /usr/local/bin/agent-slave /usr/local/bin/demo-model \
    && mkdir -p /workspace

ENTRYPOINT ["/entrypoint.sh"]
CMD ["--listen", "tcp:0.0.0.0:1024"]
