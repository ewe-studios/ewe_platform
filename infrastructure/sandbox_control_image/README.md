# The sandbox control plane in a container

This directory runs `agent-sandbox serve` in a container. The control plane then
starts sibling containers on the Docker Engine of the host. This is the
operator's choice (spec 77 feature 10, section 6.8).

Written in ASD-STE100 Simplified Technical English (STE).

## Abbreviations

Docker Engine (Engine); Transmission Control Protocol (TCP); Unix domain socket
(UDS); application programming interface (API); central processing unit (CPU).

## Trust warning

Read this first.

A process that has the Docker socket controls the whole Docker Engine of the
host. It can start a privileged container, mount any host path, and read any
secret. **Treat this container as trusted.** Do not publish its port to another
host. The compose file publishes the port on `127.0.0.1` only.

## What it does

- The container runs `agent-sandbox serve --listen tcp:0.0.0.0:7070`.
- It binds the host socket `/var/run/docker.sock`.
- It binds the host directory `/var/run/ewe-sbx` at the same path. The control
  plane and the Engine then see the same run directory path. The UDS link works
  because both sides name the same path.
- It reads the bearer token from the secret file `sandbox_token`.

The driver is the Docker driver (`EWE_SANDBOX_DRIVERS=docker`). The driver puts
`agent-slave` into each sandbox container and gives the control plane the
endpoint of that slave.

## Build

The image holds three programs. Build them first, then copy them into this
directory:

```bash
mise run sandbox:bin
cp target/agent/agent-sandbox infrastructure/sandbox_control_image/
cp target/agent/agent-slave-x86_64 infrastructure/sandbox_control_image/
cp target/agent/agent-slave-aarch64 infrastructure/sandbox_control_image/
```

The Dockerfile copies the two slave names as they are. `HostConfig::discover`
finds `agent-slave-<arch>` beside `agent-sandbox`.

## Run

1. Make the run directory on the host:

   ```bash
   sudo mkdir -p /var/run/ewe-sbx
   sudo chmod 700 /var/run/ewe-sbx
   ```

2. Write the token:

   ```bash
   openssl rand -hex 32 > infrastructure/sandbox_control_image/sandbox_token
   chmod 600 infrastructure/sandbox_control_image/sandbox_token
   ```

3. Start the stack:

   ```bash
   docker compose up --build
   ```

4. Run a command in a sandbox through the published port:

   ```bash
   agent-sandbox run \
     --connect tcp:127.0.0.1:7070 \
     --token-file infrastructure/sandbox_control_image/sandbox_token \
     --driver docker \
     --image alpine:3.22 \
     -- echo hello
   ```

   The output is `hello`.

To stop the stack, run `docker compose down`.

## macOS

Docker Desktop on macOS runs the containers in a virtual machine (VM). A UDS in
a bind-mounted directory does not cross its file sharing. This image is for a
Linux Docker host. On macOS, run the control plane on the host itself and set
the driver link to TCP, or run the control plane in a Linux VM.

## Test

`test/run.sh` builds the programs, starts the stack, runs one command through
the published port, and stops the stack. It needs `docker` and `mise`.
