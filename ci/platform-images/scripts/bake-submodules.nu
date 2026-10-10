#!/usr/bin/env nu
# Bake submodules into an image: clone each one listed in a submodules lock
# (written by `images.nu prepare`) at its pinned SHA under <dest>/<path>, with
# its nested submodules, then copy the lock to <dest>/submodules.lock.json so
# link-submodules.nu can tell which SHAs the image holds.
#
#   nu bake-submodules.nu submodules.lock.json /opt/ewe/submodules
#
# Each copy is a real git checkout (detached at the SHA, shallow), so CI can
# symlink it or clone from it without the network.

def fail [msg: string] {
    error make --unspanned {msg: $msg}
}

def git-in [dir: path, ...args: string] {
    ^git -C $dir ...$args
}

def main [lock: path, dest: path] {
    if not ($lock | path exists) { fail $"lock not found: ($lock)" }
    let entries = open $lock | get submodules
    mkdir $dest
    for e in $entries {
        let dir = $dest | path join $e.path
        if ($dir | path exists) { rm -r $dir }
        mkdir $dir
        print $"baking ($e.path) @ ($e.sha) from ($e.url)"
        git-in $dir init --quiet
        git-in $dir remote add origin $e.url
        git-in $dir fetch --quiet --depth 1 origin $e.sha
        git-in $dir -c advice.detachedHead=false checkout --quiet --detach FETCH_HEAD
        let head = git-in $dir rev-parse HEAD | str trim
        if $head != $e.sha { fail $"($e.path): checked out ($head), expected ($e.sha)" }
        if ($dir | path join .gitmodules | path exists) and ((open --raw ($dir | path join .gitmodules) | str trim) != "") {
            git-in $dir submodule update --init --recursive --depth 1
        }
    }
    ^cp $lock ($dest | path join submodules.lock.json)
    print $"baked ($entries | length) submodule\(s) into ($dest)"
}
