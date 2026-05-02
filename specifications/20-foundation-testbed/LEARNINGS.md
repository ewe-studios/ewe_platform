# Learnings — 20-foundation-testbed

_Note: This file is updated after each milestone/task completion._

## Design Decisions

- **QEMU as direct child process**, not libvirt — avoids root/sudo, simpler debugging, no daemon dependency
- **User-mode networking** (`-netdev user`) — zero root, declarative port forwarding in QEMU args
- **VNC for headless, SPICE for headful** — VNC universally available, SPICE better for interactive use
- **tar+scp instead of rsync** — rsync requires daemon on VM; tar+scp works with any SSH server, built into Windows 10+
- **WinRM for Windows bootstrap, SSH for everything else** — WinRM is the only reliable way to run elevated commands on fresh Windows; once OpenSSH is installed, all subsequent communication uses SSH
- **mise handles all tool installation inside VMs** — rust, cargo-binstall, sccache, tauri-cli, nushell, and OS-specific deps via mise. Reduces per-OS scripting surface.
- **nushell (`nu`) as the VM shell** — replaces bash (Linux) and PowerShell/cmd.exe (Windows) with a single cross-platform shell. Bootstrap scripts, build wrappers, and runner commands are written in nushell syntax, not OS-specific shell dialects. mise installs nushell as a tool.
- **Sync API** — no tokio needed; CLI calls are sequential, blocking is fine
- **Pre-built qcow2 images** — avoids 20-40 min unattended OS install; bootstrap runs tool installation on already-working OS
