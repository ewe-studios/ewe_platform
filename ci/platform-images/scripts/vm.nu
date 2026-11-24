#!/usr/bin/env nu
# platform-images: run a Windows or macOS VM image (dockurr, QEMU/KVM in a
# Linux container) on a Linux host and drive it over SSH.
#
#   nu vm.nu up --os windows --image <ref> --name ci-win --storage /mnt/vm --key ~/.ssh/ci
#   nu vm.nu wait --os windows --name ci-win --key ~/.ssh/ci --timeout 90min
#   nu vm.nu provision --os macos --name ci-mac --key ~/.ssh/ci       (macOS only)
#   nu vm.nu push --os windows --key ~/.ssh/ci --src . --dest C:/work/repo
#   nu vm.nu exec --os windows --key ~/.ssh/ci --cwd C:/work/repo --script build.ps1
#   nu vm.nu runner --os windows --key ~/.ssh/ci --jitconfig-file jit.txt
#   nu vm.nu wait-runner --os windows --key ~/.ssh/ci --timeout 6hr
#   nu vm.nu logs --os windows --name ci-win --key ~/.ssh/ci --out logs/
#   nu vm.nu down --name ci-win
#
# Every command fails (non-zero exit) when what it was asked to do did not
# happen. SSH goes to 127.0.0.1:<port>, which `up` maps to the VM's port 22.

const SSH_PORT = 2222

def fail [msg: string] {
    error make --unspanned {msg: $msg}
}

def check-os [os: string] {
    if $os not-in [windows macos] { fail $"--os must be windows or macos, not `($os)`" }
}

def default-user [os: string]: nothing -> string {
    # dockurr/windows creates `Docker` unless USERNAME is set; the macOS user
    # is whatever the one-time manual install created.
    if $os == "windows" { $env.PI_VM_USER? | default "Docker" } else { $env.PI_VM_USER? | default "ci" }
}

def ssh-args [key: path, port: int]: nothing -> list<string> {
    [
        -i $key -p ($port | into string)
        -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null
        -o LogLevel=ERROR -o ConnectTimeout=10 -o ServerAliveInterval=30
    ]
}

# Run a command in the VM. Returns {exit_code, stdout, stderr}; callers decide.
def ssh-run [os: string, key: path, port: int, command: string, --stdin: string = ""]: nothing -> record {
    let target = $"(default-user $os)@127.0.0.1"
    if ($stdin | is-empty) {
        ^ssh ...(ssh-args $key $port) $target $command | complete
    } else {
        $stdin | ^ssh ...(ssh-args $key $port) $target $command | complete
    }
}

# Run a command in the VM, streaming its output; fail on a non-zero exit.
def ssh-stream [os: string, key: path, port: int, command: string] {
    let target = $"(default-user $os)@127.0.0.1"
    ^ssh ...(ssh-args $key $port) $target $command
}

# Use the checkout's independent task runner for host container operations.
def host-task [operation: string, arguments: list<string>, --value] {
    let manifest = $env.FILE_PWD | path join .. .. .. backends foundation_ci Cargo.toml | path expand
    let args = [run --locked --quiet --profile uat --manifest-path $manifest -- task run $operation ...$arguments]
    if $value {
        let result = (^cargo ...$args)
        if $env.LAST_EXIT_CODE != 0 { exit $env.LAST_EXIT_CODE }
        return ($result | str trim)
    }
    ^cargo ...$args
    if $env.LAST_EXIT_CODE != 0 { exit $env.LAST_EXIT_CODE }
}

# Start the VM container. A fresh /storage installs the OS (Windows:
# unattended, with the image's provisioning; macOS: manually, see README).
def "main up" [
    --os: string
    --image: string                 # the VM image (layer 2 or layer 1)
    --name: string = "pi-vm"
    --storage: path                 # host directory for the VM disk (/storage)
    --key: path                     # private key; <key>.pub is authorised in a fresh Windows VM
    --port: int = $SSH_PORT         # host port for the VM's SSH
    --web-port: int = 8006          # host port for dockurr's web viewer (127.0.0.1 only)
    --version: string = ""          # dockurr VERSION (Windows edition / macOS release); image default if empty
    --ram: string = ""              # RAM_SIZE, e.g. 10G
    --cpus: string = ""             # CPU_CORES
    --disk: string = ""             # DISK_SIZE
] {
    host-task images:vm-up [
        --os $os --image $image --name $name --storage ($storage | into string) --key ($key | into string)
        --port ($port | into string) --web-port ($web_port | into string)
        --version $version --ram $ram --cpus $cpus --disk $disk
    ]
    print $"started ($name) from ($image): web viewer on http://127.0.0.1:($web_port), SSH on 127.0.0.1:($port)"
}

# Wait until the VM is reachable over SSH and provisioned. Windows
# provisions itself at the end of the unattended install; macOS needs
# `provision` first (this then only waits for SSH unless --provisioned).
def "main wait" [
    --os: string
    --name: string = "pi-vm"
    --key: path
    --port: int = $SSH_PORT
    --timeout: duration = 120min
    --ssh-only                      # only wait for SSH, not for provisioning
] {
    mut args = [
        --os $os --name $name --key ($key | into string) --port ($port | into string)
        --timeout-ms (($timeout / 1ms) | math ceil | into int | into string)
    ]
    if $ssh_only { $args = ($args | append "--ssh-only") }
    host-task images:vm-wait $args
}

# macOS only: copy the guest provisioning (layer 1 + repository layers) out
# of the image and run it in the VM over SSH.
def "main provision" [
    --os: string = "macos"
    --name: string = "pi-vm"
    --key: path
    --port: int = $SSH_PORT
] {
    if $os != "macos" { fail "only macOS is provisioned over SSH; Windows provisions itself during the install" }
    let tmp = mktemp -d
    ^docker cp $"($name):/opt/platform-images/guest" $"($tmp)/guest"
    let layers = ^docker exec $name sh -c 'test -d /opt/platform-images/layers && echo yes || echo no' | str trim
    if $layers == "yes" { ^docker cp $"($name):/opt/platform-images/layers" $"($tmp)/guest/layers" }
    let tarball = $"($tmp)/guest.tar.gz"
    ^tar -czf $tarball -C $tmp guest
    let dest = "/tmp/platform-images-guest"
    let r = ssh-run $os $key $port $"rm -rf ($dest) && mkdir -p ($dest) && tar -xzf - -C ($dest)" --stdin (open --raw $tarball)
    if $r.exit_code != 0 { fail $"copying the provisioning into the VM failed: ($r.stderr)" }
    rm -r $tmp
    ssh-stream $os $key $port $"bash ($dest)/guest/provision.sh 2>&1 | tee /tmp/platform-images-provision.log; exit \${PIPESTATUS[0]}"
}

# Copy a directory into the VM (as a tarball over SSH; Windows has tar.exe).
def "main push" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --src: path = "."
    --dest: string                  # e.g. C:/work/repo or /Users/ci/work/repo
    --exclude: list<string> = [target]
] {
    mut args = [
        --os $os --key ($key | into string) --port ($port | into string)
        --src ($src | into string) --dest $dest --explicit-excludes
    ]
    for pattern in $exclude { $args = ($args | append ["--exclude" $pattern]) }
    host-task images:vm-push $args
}

# Copy a file or directory out of the VM.
def "main pull" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --src: string                   # path in the VM
    --dest: path
] {
    host-task images:vm-pull [
        --os $os --key ($key | into string) --port ($port | into string)
        --src $src --dest ($dest | into string)
    ]
}

# Run a script in the VM, in --cwd, streaming its output. Windows: a
# PowerShell 7 script that stops at the first failing command (including
# native commands); macOS: a bash script with `set -euo pipefail`.
def "main exec" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --cwd: string
    --script: path                  # file with the commands
] {
    check-os $os
    let body = open --raw $script
    let remote = if $os == "windows" { "C:/platform-images/exec.ps1" } else { "/tmp/platform-images-exec.sh" }
    let prologue = if $os == "windows" {
        "$ErrorActionPreference = 'Stop'\n$PSNativeCommandUseErrorActionPreference = $true\nSet-Location -LiteralPath '" + $cwd + "'\n"
    } else {
        $"set -euo pipefail\ncd '($cwd)'\n"
    }
    let write = if $os == "windows" {
        $"Set-Content -Path '($remote)' -Value \([Console]::In.ReadToEnd\(\)\)"
    } else {
        $"cat > '($remote)'"
    }
    let r = ssh-run $os $key $port $write --stdin ($prologue + $body)
    if $r.exit_code != 0 { fail $"writing the script into the VM failed: ($r.stderr)" }
    if $os == "windows" {
        ssh-stream $os $key $port $"pwsh -NoProfile -File '($remote)'"
    } else {
        ssh-stream $os $key $port $"bash -l '($remote)'"
    }
}

# Start the GitHub Actions runner in the VM with a just-in-time config: an
# ephemeral runner that takes one job and unregisters itself. It runs
# detached (Windows: a scheduled task as SYSTEM; macOS: nohup), so this
# returns once it is up; `wait-runner` waits for it to finish.
def "main runner" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --jitconfig-file: path          # the encoded_jit_config from the GitHub API
] {
    check-os $os
    let jit = open --raw $jitconfig_file | str trim
    if ($jit | is-empty) { fail $"($jitconfig_file) is empty" }
    let start = if $os == "windows" {
        # The JIT config (a few KB) goes into a file only SYSTEM/Administrators
        # can read; a small launcher passes it to run.cmd. (cmd's `set /p`
        # would truncate it at 1021 characters.)
        [
            "$jit = [Console]::In.ReadToEnd().Trim()"
            "Set-Content -Path C:/actions-runner/.jitconfig -Value $jit -NoNewline"
            "icacls C:/actions-runner/.jitconfig /inheritance:r /grant 'SYSTEM:F' /grant 'Administrators:F' | Out-Null"
            "Set-Content -Path C:/actions-runner/run-jit.ps1 -Value '$jit = (Get-Content -Raw C:/actions-runner/.jitconfig).Trim(); & C:/actions-runner/run.cmd --jitconfig $jit *> C:/actions-runner/runner.log; exit $LASTEXITCODE'"
            "$action = New-ScheduledTaskAction -Execute 'C:\\Program Files\\PowerShell\\7\\pwsh.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -File C:\\actions-runner\\run-jit.ps1' -WorkingDirectory 'C:\\actions-runner'"
            "$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -LogonType ServiceAccount -RunLevel Highest"
            "$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero)"
            "Register-ScheduledTask -TaskName platform-images-runner -Action $action -Principal $principal -Settings $settings -Force | Out-Null"
            "Start-ScheduledTask -TaskName platform-images-runner"
        ] | str join "; "
    } else {
        # `;`, not `&&`, before the `&`: a trailing `&` backgrounds the whole
        # && chain, and a background job reads stdin from /dev/null.
        "umask 077; cd /usr/local/actions-runner || exit 1; cat > .jitconfig || exit 1; [ -s .jitconfig ] || { echo 'empty JIT config' >&2; exit 1; }; nohup bash -lc './run.sh --jitconfig \"$(cat .jitconfig)\"' > runner.log 2>&1 < /dev/null & echo started"
    }
    let r = ssh-run $os $key $port $start --stdin $jit
    if $r.exit_code != 0 { fail $"starting the runner failed: ($r.stderr)" }
    # Give it a moment and make sure it did not exit straight away.
    sleep 20sec
    let alive = main runner-alive --os $os --key $key --port $port
    if not $alive {
        let log = ssh-run $os $key $port (if $os == "windows" { "Get-Content C:/actions-runner/runner.log -Tail 30" } else { "tail -30 /usr/local/actions-runner/runner.log" })
        fail $"the runner exited right after starting:\n($log.stdout)"
    }
    print "runner started"
}

def "main runner-alive" [--os: string, --key: path, --port: int = $SSH_PORT]: nothing -> bool {
    host-task --value images:vm-runner-alive [--os $os --key ($key | into string) --port ($port | into string)] | into bool
}

# Wait for the ephemeral runner to finish its job (it exits afterwards).
def "main wait-runner" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --timeout: duration = 6hr
] {
    host-task images:vm-wait-runner [
        --os $os --key ($key | into string) --port ($port | into string)
        --timeout-ms (($timeout / 1ms) | math ceil | into int | into string)
    ]
}

# Collect the provisioning log, the runner log and the container log.
def "main logs" [
    --os: string
    --name: string = "pi-vm"
    --key: path
    --port: int = $SSH_PORT
    --out: path
] {
    host-task images:vm-logs [
        --os $os --name $name --key ($key | into string) --port ($port | into string) --out ($out | into string)
    ]
}

# Stop the VM (an orderly shutdown via dockurr, up to 2 minutes) and remove
# the container. The disk in --storage stays.
def "main down" [--name: string = "pi-vm"] {
    host-task images:vm-down [--name $name]
}

def main [] {
    print "usage: nu vm.nu <up|wait|provision|push|pull|exec|runner|wait-runner|logs|down> ... (see the header of this file)"
}
