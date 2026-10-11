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
const STATUS_WINDOWS = "C:/OEM/status"
const STATUS_MACOS = "/usr/local/platform-images/status"

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

# scp spells the port option -P.
def scp-args [key: path, port: int]: nothing -> list<string> {
    ssh-args $key $port | each {|a| if $a == "-p" { "-P" } else { $a } }
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

def container-running [name: string]: nothing -> bool {
    let r = ^docker inspect --format '{{.State.Running}}' $name | complete
    $r.exit_code == 0 and ($r.stdout | str trim) == "true"
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
    check-os $os
    if ($image | is-empty) or ($storage | is-empty) or ($key | is-empty) { fail "--image, --storage and --key are required" }
    if not ("/dev/kvm" | path exists) { fail "/dev/kvm does not exist: this host cannot run the VM (see the README on KVM)" }
    let pub = $"($key).pub"
    if not ($pub | path exists) { fail $"no public key at ($pub)" }
    mkdir $storage
    if (container-running $name) { fail $"a container named ($name) is already running" }
    ^docker rm -f $name | complete | ignore

    mut args = [
        run --detach --name $name
        --device /dev/kvm --device /dev/net/tun --cap-add NET_ADMIN
        --stop-timeout 120
        --publish $"127.0.0.1:($port):22"
        --publish $"127.0.0.1:($web_port):8006"
        --volume $"($storage | path expand):/storage"
        --env $"PI_SSH_AUTHORIZED_KEY=(open --raw $pub | str trim)"
    ]
    for kv in [[VERSION $version] [RAM_SIZE $ram] [CPU_CORES $cpus] [DISK_SIZE $disk]] {
        if ($kv.1 | is-not-empty) { $args = ($args | append [--env $"($kv.0)=($kv.1)"]) }
    }
    ^docker ...$args $image
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
    check-os $os
    let deadline = (date now) + $timeout
    let probe = if $ssh_only {
        if $os == "windows" { "exit 0" } else { "true" }
    } else if $os == "windows" {
        $"if \(Test-Path '($STATUS_WINDOWS)/provisioned.ok') { exit 0 } elseif \(Test-Path '($STATUS_WINDOWS)/provision.failed') { exit 3 } else { exit 2 }"
    } else {
        $"if [ -f ($STATUS_MACOS)/provisioned.ok ]; then exit 0; elif [ -f ($STATUS_MACOS)/provision.failed ]; then exit 3; else exit 2; fi"
    }
    mut last_log = (date now)
    loop {
        if not (container-running $name) {
            ^docker logs --tail 50 $name
            fail $"the VM container ($name) stopped"
        }
        let r = ssh-run $os $key $port $probe
        if $r.exit_code == 0 { print $"($name) is ready"; return }
        if $r.exit_code == 3 { fail $"provisioning failed in ($name); `vm.nu logs` fetches provision.log" }
        if (date now) > $deadline { fail $"($name) was not ready after ($timeout)" }
        if ((date now) - $last_log) > 5min {
            print $"still waiting for ($name) \(ssh: ($r.exit_code)\); container log:"
            ^docker logs --tail 5 $name
            $last_log = (date now)
        }
        sleep 30sec
    }
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
    check-os $os
    let tarball = mktemp --suffix .tar.gz
    let excludes = $exclude | each {|e| $"--exclude=./($e)" }
    ^tar -czf $tarball ...$excludes -C $src .
    if $os == "windows" {
        # PowerShell does not read binary stdin reliably; copy with scp, then
        # unpack with Windows' own tar.exe.
        let target = $"(default-user $os)@127.0.0.1"
        ^scp ...(scp-args $key $port) $tarball $"($target):($dest).tar.gz"
        let r = ssh-run $os $key $port $"New-Item -ItemType Directory -Force -Path '($dest)' | Out-Null; tar -xzf '($dest).tar.gz' -C '($dest)'; if \($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; Remove-Item '($dest).tar.gz'"
        if $r.exit_code != 0 { fail $"unpacking in the VM failed: ($r.stderr)" }
    } else {
        let r = ssh-run $os $key $port $"mkdir -p '($dest)' && tar -xzf - -C '($dest)'" --stdin (open --raw $tarball)
        if $r.exit_code != 0 { fail $"copying into the VM failed: ($r.stderr)" }
    }
    rm $tarball
    print $"pushed ($src) to ($dest)"
}

# Copy a file or directory out of the VM.
def "main pull" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --src: string                   # path in the VM
    --dest: path
] {
    check-os $os
    mkdir $dest
    let target = $"(default-user $os)@127.0.0.1"
    ^scp -r ...(scp-args $key $port) $"($target):($src)" $dest
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
    let probe = if $os == "windows" {
        "if (Get-Process -Name Runner.Listener -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }"
    } else {
        # [R]: keep pgrep from matching the shell that runs this command.
        "pgrep -f '[R]unner.Listener' >/dev/null"
    }
    (ssh-run $os $key $port $probe).exit_code == 0
}

# Wait for the ephemeral runner to finish its job (it exits afterwards).
def "main wait-runner" [
    --os: string
    --key: path
    --port: int = $SSH_PORT
    --timeout: duration = 6hr
] {
    check-os $os
    let deadline = (date now) + $timeout
    while (main runner-alive --os $os --key $key --port $port) {
        if (date now) > $deadline { fail $"the runner was still busy after ($timeout)" }
        sleep 30sec
    }
    print "runner finished"
}

# Collect the provisioning log, the runner log and the container log.
def "main logs" [
    --os: string
    --name: string = "pi-vm"
    --key: path
    --port: int = $SSH_PORT
    --out: path
] {
    check-os $os
    mkdir $out
    ^docker logs $name o+e> ($out | path join container.log)
    let files = if $os == "windows" {
        [[$"($STATUS_WINDOWS)/provision.log" provision.log] [$"($STATUS_WINDOWS)/versions.txt" versions.txt] ["C:/actions-runner/runner.log" runner.log]]
    } else {
        [["/tmp/platform-images-provision.log" provision.log] [$"($STATUS_MACOS)/versions.txt" versions.txt] ["/usr/local/actions-runner/runner.log" runner.log]]
    }
    for f in $files {
        let r = ssh-run $os $key $port (if $os == "windows" { $"Get-Content -Raw '($f.0)'" } else { $"cat '($f.0)'" })
        if $r.exit_code == 0 { $r.stdout | save --force ($out | path join $f.1) } else { print $"\(no ($f.0) in the VM\)" }
    }
}

# Stop the VM (an orderly shutdown via dockurr, up to 2 minutes) and remove
# the container. The disk in --storage stays.
def "main down" [--name: string = "pi-vm"] {
    let r = ^docker inspect $name | complete
    if $r.exit_code != 0 { print $"no container ($name)"; return }
    ^docker stop --time 120 $name
    ^docker rm $name
}

def main [] {
    print "usage: nu vm.nu <up|wait|provision|push|pull|exec|runner|wait-runner|logs|down> ... (see the header of this file)"
}
