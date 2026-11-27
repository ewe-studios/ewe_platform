# foundation_sandbox

One library that starts sandboxes and runs commands in them. A sandbox is a
Firecracker microVM, a libkrun microVM or a container. The same code serves a
host program, a Cloudflare Worker and a nested container (spec 77).

## The model

- **The control plane** (`SandboxManager`) owns every sandbox. It validates a
  request, asks a driver to start a substrate, waits until the slave answers,
  passes commands to the slave and cleans up.
- **The slave** (`agent-slave`) serves `SlaveService`: `Info`, `Prepare`,
  `Exec`, file calls and `Shutdown`. It is process identifier 1 in a microVM.
- **A driver** starts and stops one kind of substrate and says how to reach the
  slave in it. Firecracker is the first driver.
- **A caller** uses the control plane as a library (`SandboxProvider`), over
  remote procedure call (RPC) (`SandboxRpcClient` with `SandboxService`), or
  through a `Provider` facade for deployment code.

## The programs

| Program | Purpose |
|---|---|
| `agent-sandbox` | The control plane. Sub-commands: `run`, `serve`, `create`, `exec`, `list`, `stop`, `kill`, `doctor`. |
| `agent-slave` | The guest program. It serves `SlaveService` on a vsock port, a Unix domain socket (UDS) or TCP. |

`agent-slave` builds with `--no-default-features --features slave,cli`. That
build has no TLS library and no Docker client, so it builds as a static musl
program.

```bash
mise run sandbox:slave
file target/sandbox/agent-slave-x86_64   # must say "statically linked"
```

## Host requirements for Firecracker

- Linux with Kernel-based Virtual Machine (KVM). The user must open `/dev/kvm`
  for reading and writing: join the `kvm` group, or run
  `setfacl -m u:$USER:rw /dev/kvm`.
- A `firecracker` program on `PATH`, or `EWE_FIRECRACKER=<path>`.
- `agent-slave-<arch>` beside `agent-sandbox`, or `EWE_SANDBOX_SLAVE=<path>`.

Run `agent-sandbox doctor` to see one report for each driver and the reason for
each failed check.

## First sandbox

```bash
agent-sandbox run --rootfs /images/ubuntu.squashfs -- echo hello
```

With no `--rootfs`, `run` fetches the pinned continuous integration (CI) kernel
and root file system into the cache and uses them.

## Build an image from a Sandboxfile

A `Sandboxfile` is a small build file. Each step that changes the image runs in
one of our own microVMs, so a build of code that is not trusted is isolated.

```bash
agent-sandbox image build -f Sandboxfile --tag app .
agent-sandbox run --image app
```

| Instruction | Form |
|---|---|
| `FROM` | `alpine:<version>`, `image:<tag-or-key>`, `docker:<name>` or `tar:<path>`. `oci:` needs feature 15. |
| `KERNEL` | A kernel name of the artifact manifest, or `default`. At most one. |
| `ENV` | `KEY=value`. The value is the rest of the line. |
| `WORKDIR` | An absolute path. |
| `USER` | `uid`, `uid:gid`, `name` or `name:group`. |
| `ADD` | `[--mode <octal>] <source> <destination>`. The source is relative to the build context. |
| `FETCH` | `<url> sha256:<64 hex> AS <destination>`. The host downloads and checks the file. |
| `RUN` | `[--shell "<program and arguments>"] [--network none\|host] <text>`, or a here-document. |
| `CMD` | A JSON array of strings. At most one. |

The first instruction is `FROM`. A line that ends with `\` continues on the
next line. A line whose first character after spaces is `#` is a comment.

The default shell is `/bin/sh -e -s`. The builder writes the script to the
standard input of the shell, so `--shell` must name a program that reads its
program from standard input, for example `python3 -`.

A `RUN --network host` step needs a driver with a guest network. In this spec
only libkrun with block support gives one. A host without such a driver refuses
the build with `UnsupportedNetwork` before it starts any work.

## The library

```rust
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::{SandboxManager, SandboxProvider};
use foundation_sandbox::shared::error::SandboxResult;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};

async fn run_one(ctx: Ctx, manager: Arc<SandboxManager>) -> SandboxResult<()> {
    let spec = SandboxSpec::new(RootFs::SquashfsImage("/images/ubuntu.squashfs".into()));
    let sandbox = manager.create(spec).await?;
    let output = sandbox.run(Command::new(["echo", "hello"])).await?;
    assert_eq!(output.stdout, b"hello\n");
    sandbox.stop().await?;
    Ok(())
}
```

## Serve the control plane

```bash
agent-sandbox serve --listen unix:/run/ewe-sandbox.sock
agent-sandbox create --connect unix:/run/ewe-sandbox.sock --rootfs /images/ubuntu.squashfs
```

```bash
agent-sandbox serve --listen tcp:127.0.0.1:7777 --token-file /etc/ewe/token
agent-sandbox list --connect tcp:127.0.0.1:7777 --token-file /etc/ewe/token
```

## Security notes

- The virtual machine boundary isolates the guest. The virtual machine monitor
  (VMM) process runs with the rights of the caller; this version does not
  confine it.
- `serve` on a UDS trusts every process that can open the socket. The socket
  file has mode `0600`.
- `serve` on TCP uses one shared token and no TLS. Use it on `127.0.0.1` or a
  private network. Mutual TLS comes with spec 87.
- A path in a spec is a path on the machine of the control plane. A remote
  caller that can create sandboxes can read each root image that the control
  plane's user can read.
- The token is never logged.
