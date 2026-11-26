#!/usr/bin/env nu
# platform-images: run a Windows or macOS VM image (dockurr, QEMU/KVM in a
# Linux container) on a Linux host and drive it over SSH.
#
#   nu vm.nu up --os windows --image <ref> --name ci-win --storage /mnt/vm --key ~/.ssh/ci
#   nu vm.nu wait --os windows --name ci-win --key ~/.ssh/ci --timeout 90min
#   nu vm.nu provision --os macos --name ci-mac --key ~/.ssh/ci       (macOS only)
#   nu vm.nu push --os windows --key ~/.ssh/ci --src . --dest C:/work/repo
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
    host-task images:vm-provision [
        --os $os --name $name --key ($key | into string) --port ($port | into string)
    ]
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
    print "usage: nu vm.nu <up|wait|provision|push|pull|wait-runner|logs|down> ... (see the header of this file)"
}
