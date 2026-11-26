FROM alpine:3.22
COPY agent-slave /usr/local/bin/agent-slave
RUN mkdir -p /workspace
CMD ["/usr/local/bin/agent-slave", "--listen", "tcp:0.0.0.0:1024"]
