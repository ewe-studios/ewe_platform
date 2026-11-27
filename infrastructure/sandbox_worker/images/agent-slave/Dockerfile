# The donor image of the sandbox. It holds one file: the static slave.
#
# `mise run sandbox:slave-image` copies the amd64 slave into a temporary build
# directory and runs this file there. The image has one layer and no other
# entry.
FROM scratch
COPY agent-slave /usr/local/bin/agent-slave
