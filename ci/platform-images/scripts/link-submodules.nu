#!/usr/bin/env nu
# Put submodules into a checkout from the copies baked into the image.
#
#   nu link-submodules.nu                         # every submodule in the lock
#   nu link-submodules.nu tools/llama.cpp         # just these paths
#   nu link-submodules.nu --mode submodule ...    # git-clean checkout instead of a symlink
#
# For each path, the gitlink SHA in the checkout's HEAD is compared with the
# SHA the image baked (its submodules.lock.json):
#   - same SHA: use the baked copy, no network.
#       --mode symlink (default): <path> becomes a symlink to the baked copy.
#       --mode submodule: a real submodule checkout cloned from the baked copy
#         (for tools that need `git status` clean, e.g. `cargo publish`).
#   - different SHA, or not baked: the image is stale for this submodule. Say
#     so loudly and fetch that one submodule at the right SHA from its remote,
#     so the build is never wrong, only slower.
#
# The lock defaults to $PLATFORM_IMAGES_SUBMODULES_LOCK, which the layer 2
# image sets. Exits non-zero if any submodule could not be put in place.

def fail [msg: string] {
    error make --unspanned {msg: $msg}
}

def in-github []: nothing -> bool {
    ($env.GITHUB_ACTIONS? | default "") == "true"
}

def warn [msg: string] {
    if (in-github) { print $"::warning title=stale CI image::($msg)" } else { print --stderr $"warning: ($msg)" }
}

# git in the workspace. The checkout may belong to another uid than the one
# running in the container, so trust it explicitly for these commands.
def ws-git [ws: path, ...args: string] {
    ^git -c $"safe.directory=($ws)" -C $ws ...$args
}

def gitlink-sha [ws: path, sub_path: string]: nothing -> string {
    let line = ws-git $ws ls-tree HEAD -- $sub_path | str trim
    if ($line | is-empty) { fail $"($sub_path) is not in HEAD of ($ws)" }
    let fields = $line | split row "\t" | first | split row " "
    if $fields.0 != "160000" { fail $"($sub_path) is not a submodule in ($ws)" }
    $fields.2
}

# The path must be absent, an empty directory (what a checkout without
# submodules leaves), or our own symlink from an earlier run.
def clear-path [target: path] {
    let kind = $target | path type
    if $kind == "symlink" {
        rm $target
    } else if $kind == "dir" {
        if not (ls -a $target | is-empty) {
            fail $"($target) is not empty; refusing to replace it \(already initialised?)"
        }
        rm $target
    } else if $kind != null and $kind != "" {
        fail $"($target) exists and is a ($kind), not an empty submodule directory"
    }
}

def fetch-submodule [ws: path, sub_path: string] {
    let target = $ws | path join $sub_path
    if ($target | path type) == "symlink" { rm $target }
    ws-git $ws submodule update --init --recursive --depth 1 -- $sub_path
}

def main [
    ...paths: string                # submodule paths; default: all in the lock
    --lock: path                    # default: $PLATFORM_IMAGES_SUBMODULES_LOCK
    --workspace: path = "."         # the checkout
    --mode: string = "symlink"      # symlink | submodule
] {
    if $mode not-in [symlink submodule] { fail $"--mode must be symlink or submodule, not ($mode)" }
    let ws = $workspace | path expand
    let lock_path = if $lock != null { $lock } else { $env.PLATFORM_IMAGES_SUBMODULES_LOCK? | default "" }
    let baked = if ($lock_path | is-empty) {
        warn "no submodules lock (PLATFORM_IMAGES_SUBMODULES_LOCK unset); fetching every requested submodule"
        []
    } else {
        if not ($lock_path | path exists) { fail $"submodules lock not found: ($lock_path)" }
        open $lock_path | get submodules
    }
    let baked_root = if ($lock_path | is-empty) { "" } else { $lock_path | path expand | path dirname }
    let wanted = if ($paths | is-empty) { $baked | get path } else { $paths }
    if ($wanted | is-empty) { fail "nothing to link: no paths given and the lock is empty" }

    let results = $wanted | each {|p|
        let want_sha = gitlink-sha $ws $p
        let entry = $baked | where path == $p
        let baked_dir = if ($baked_root | is-empty) { "" } else { $baked_root | path join $p }
        if ($entry | is-empty) {
            warn $"($p) is not baked into this image; fetching ($want_sha) from its remote"
            fetch-submodule $ws $p
            {path: $p, sha: $want_sha, source: "fetched (not baked)"}
        } else if $entry.0.sha != $want_sha {
            warn $"($p): image has ($entry.0.sha), this commit needs ($want_sha). Fetching it; rebuild the image to make this fast again."
            fetch-submodule $ws $p
            {path: $p, sha: $want_sha, source: "fetched (image stale)"}
        } else {
            if not ($baked_dir | path exists) { fail $"lock lists ($p) but ($baked_dir) is missing from the image" }
            let target = $ws | path join $p
            clear-path $target
            if $mode == "symlink" {
                ^ln -s $baked_dir $target
            } else {
                # Clone from the baked copy instead of the remote, then let
                # git record it as a normal, initialised submodule.
                let name = (
                    ws-git $ws config -f .gitmodules --get-regexp '^submodule\..*\.path$'
                    | lines | parse "{key} {path}" | where path == $p | get key.0
                    | str replace --regex '^submodule\.' '' | str replace --regex '\.path$' ''
                )
                mkdir $target
                ws-git $ws config $"submodule.($name).url" $baked_dir
                ^git -c protocol.file.allow=always -c $"safe.directory=($ws)" -c $"safe.directory=($baked_dir)" -C $ws submodule update --init --recursive -- $p
            }
            let got = ^git -c $"safe.directory=*" -C $target rev-parse HEAD | str trim
            if $got != $want_sha { fail $"($p): linked copy is at ($got), expected ($want_sha)" }
            {path: $p, sha: $want_sha, source: $"baked \(($mode))"}
        }
    }
    print ($results | table --index false)
}
