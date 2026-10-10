#!/usr/bin/env nu
# Install the toolchain a rust-toolchain.toml pins (channel, components,
# targets) with rustup, plus any extras, and make it the default.
#
#   nu install-rust-toolchain.nu rust-toolchain.toml --components "clippy rust-analyzer" --targets wasm32-wasip1
#
# Installs with `--profile minimal` plus the listed components rather than the
# file's `profile`, so an image does not carry rust-docs; components the file
# lists are always installed.

def fail [msg: string] {
    error make --unspanned {msg: $msg}
}

def main [
    toolchain_file: path
    --components: string = ""         # extra components, space separated (e.g. "clippy")
    --targets: string = ""            # extra targets, space separated
    --no-default                      # don't `rustup default` the channel
] {
    if not ($toolchain_file | path exists) { fail $"not found: ($toolchain_file)" }
    let tc = open $toolchain_file | get -o toolchain
    if $tc == null { fail $"($toolchain_file) has no [toolchain] table" }
    let channel = $tc | get -o channel
    if ($channel | is-empty) { fail $"($toolchain_file) has no toolchain.channel" }

    let components = $tc | get -o components | default [] | append ($components | split row ' ' | where {|c| $c != '' }) | uniq
    let targets = $tc | get -o targets | default [] | append ($targets | split row ' ' | where {|t| $t != '' }) | uniq

    mut args = [toolchain install $channel --profile minimal --no-self-update]
    if not ($components | is-empty) { $args = ($args | append [--component ($components | str join ",")]) }
    if not ($targets | is-empty) { $args = ($args | append [--target ($targets | str join ",")]) }
    print $"rustup ($args | str join ' ')"
    ^rustup ...$args

    if not $no_default { ^rustup default $channel }

    # Fail now, not in the first CI job, if something did not install.
    let installed = ^rustup component list --installed --toolchain $channel | lines
    for c in $components {
        if not ($installed | any {|line| $line | str starts-with $c }) {
            fail $"component ($c) is not installed for ($channel)"
        }
    }
    let installed_targets = ^rustup target list --installed --toolchain $channel | lines
    for t in $targets {
        if $t not-in $installed_targets { fail $"target ($t) is not installed for ($channel)" }
    }
    ^rustc $"+($channel)" --version
}
