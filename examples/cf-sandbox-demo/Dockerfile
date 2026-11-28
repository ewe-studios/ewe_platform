# The container image of the demo sandbox.
#
# The image runs the static slave, which is process identifier 1 and owns the
# signal handling. `mise run build` copies the program here before `wrangler`
# builds the image. It is in `.gitignore`.
#
# A third model path needs a model host beside the slave in this image. It
# waits for issue #1276.
FROM alpine:3.22

COPY agent-slave /usr/local/bin/agent-slave

RUN chmod +x /usr/local/bin/agent-slave && mkdir -p /workspace

ENTRYPOINT ["/usr/local/bin/agent-slave"]
CMD ["--listen", "tcp:0.0.0.0:1024"]
