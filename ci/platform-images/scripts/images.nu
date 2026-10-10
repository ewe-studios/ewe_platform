#!/usr/bin/env nu
# platform-images: stage an image layer's build context, hash its inputs, and
# check or tag images in the registry. Everything here is driven by the
# consumer's config.toml; nothing about a particular repository is hardcoded.
#
#   nu images.nu prepare --config ci/ewe-platform-image/config.toml --layer base --out /tmp/pi/base
#   nu images.nu prepare --config ... --layer image --parent docker.io/ns/ci-base@sha256:... --out /tmp/pi/image
#   nu images.nu exists docker.io/ns/ci-base:hash-0123...
#   nu images.nu promote docker.io/ns/image@sha256:... --tags "latest sha-abc1234"
#
# Layers:
#   base   generic layer 1, built from this unit's images/linux (FROM the
#          flavor's upstream image, e.g. ubuntu or nvidia/cuda)
#   image  the consumer's layer 2, built from its own Dockerfile FROM a base
#          image pinned by digest
#
# The hash is a sha256 over every staged context file (path, executable bit,
# content), the build args, and for layer 2 the parent digest. Only the first
# 20 hex digits are used in the tag: `hash-<20 hex>`.

const HASH_SCHEME = "platform-images/v1"
const HASH_LEN = 20
const LINUX_PLATFORM = "linux/amd64"

# Where this unit lives (the directory above scripts/).
def unit-dir []: nothing -> path {
    $env.FILE_PWD | path dirname
}

def fail [msg: string] {
    error make --unspanned {msg: $msg}
}

def load-config [config: path]: nothing -> record {
    if not ($config | path exists) { fail $"config not found: ($config)" }
    let cfg = open $config
    for key in [registry base image] {
        if $key not-in $cfg { fail $"($config): missing [($key)] table" }
    }
    for key in [namespace username] {
        if $key not-in $cfg.registry { fail $"($config): [registry] needs `($key)`" }
    }
    $cfg
}

# The registry username: the config's, unless PLATFORM_IMAGES_REGISTRY_USERNAME
# is set (workflows set it from a repository variable to override).
def registry-username [cfg: record]: nothing -> string {
    let override = $env.PLATFORM_IMAGES_REGISTRY_USERNAME? | default ""
    if ($override | is-empty) { $cfg.registry.username } else { $override }
}

def repository [cfg: record, name: string, flavor: string]: nothing -> string {
    let host = $cfg.registry.host? | default "docker.io"
    let suffix = if $flavor == "default" { "" } else { $"-($flavor)" }
    $"($host)/($cfg.registry.namespace)/($name)($suffix)"
}

def flavor-config [cfg: record, flavor: string]: nothing -> record {
    let flavors = $cfg.base.flavors? | default {}
    if $flavor not-in $flavors {
        fail $"flavor `($flavor)` is not defined under [base.flavors] \(defined: ($flavors | columns | str join ', '))"
    }
    $flavors | get $flavor
}

# sha256 of the staged context: one line per file, sorted by path.
def hash-context [context: path, exclude: list<string>]: nothing -> list<string> {
    let files = (
        ^find $context -not -type d
        | lines
        | each {|f| $f | path relative-to $context }
        | where {|rel| not ($exclude | any {|ex| $rel == $ex or ($rel | str starts-with $"($ex)/") }) }
        | sort
    )
    $files | each {|rel|
        let full = $context | path join $rel
        let kind = ($full | path type)
        if $kind == "symlink" {
            $"($rel)\tlink\t(^readlink $full | str trim)"
        } else {
            let mode = if (do { ^test -x $full } | complete).exit_code == 0 { "x" } else { "-" }
            $"($rel)\t($mode)\t(open --raw $full | hash sha256)"
        }
    }
}

def compute-hash [lines: list<string>, build_args: record, parent: string]: nothing -> string {
    let args = $build_args | transpose k v | sort-by k | each {|r| $"arg\t($r.k)=($r.v)" }
    let body = [$HASH_SCHEME ...$lines ...$args $"parent\t($parent)"] | str join "\n"
    $body | hash sha256 | str substring 0..<($HASH_LEN)
}

def copy-into [src: path, dest: path] {
    if not ($src | path exists) { fail $"missing input: ($src)" }
    mkdir ($dest | path dirname)
    ^cp -a $src $dest
}

# Gitlink SHA of a submodule path in HEAD, and its URL from .gitmodules.
def submodule-entry [repo: path, sub_path: string]: nothing -> record {
    let tree = ^git -C $repo ls-tree HEAD -- $sub_path | str trim
    if ($tree | is-empty) { fail $"submodule ($sub_path) is not in HEAD" }
    let parts = $tree | split row "\t" | first | split row " "
    if ($parts.0 != "160000") { fail $"($sub_path) is not a submodule \(git mode ($parts.0))" }
    let sha = $parts.2
    let names = (
        ^git -C $repo config -f .gitmodules --get-regexp '^submodule\..*\.path$'
        | lines
        | parse "{key} {path}"
        | where path == $sub_path
    )
    if ($names | is-empty) { fail $"($sub_path) has no entry in .gitmodules" }
    let name = $names.0.key | str replace --regex '^submodule\.' '' | str replace --regex '\.path$' ''
    let url = do { ^git -C $repo config -f .gitmodules --get $"submodule.($name).url" } | complete
    if $url.exit_code != 0 or ($url.stdout | str trim | is-empty) {
        fail $"submodule ($sub_path) has no url in .gitmodules, so it cannot be baked"
    }
    {path: $sub_path, url: ($url.stdout | str trim), sha: $sha}
}

# Layer 1: this unit's images/linux plus scripts/.
def stage-base [cfg: record, flavor: string, context: path]: nothing -> record {
    let unit = unit-dir
    let fl = flavor-config $cfg $flavor
    let linux = $unit | path join images linux
    copy-into ($linux | path join Dockerfile) ($context | path join Dockerfile)
    for dir in [packages mise bin] {
        copy-into ($linux | path join $dir) ($context | path join $dir)
    }
    copy-into ($unit | path join scripts) ($context | path join scripts)
    let extra = $cfg.base.packages? | default []
    if not ($extra | is-empty) {
        $"# Extra packages from the consumer config \([base] packages)\n($extra | str join "\n")\n"
        | save --force ($context | path join packages 90-config.txt)
    }
    {
        name: $cfg.base.name
        build_args: {
            BASE_IMAGE: $fl.from
            BROWSERS: ($cfg.base.browsers? | default [] | str join " ")
        }
        exclude: []
    }
}

# Layer 2: the consumer's Dockerfile and the inputs its config lists.
def stage-image [cfg: record, flavor: string, repo: path, parent: string, context: path]: nothing -> record {
    if ($parent | is-empty) { fail "--parent is required for the image layer" }
    let img = $cfg.image
    copy-into ($repo | path join $img.dockerfile) ($context | path join Dockerfile)
    for f in ($img.files? | default []) {
        copy-into ($repo | path join $f) ($context | path join files $f)
    }

    # mise: only [tools] (and [settings]) matter to the image, so tasks and env
    # edits in the consumer's mise.toml do not change the hash.
    if ($img.mise_config? | is-not-empty) {
        let src = open ($repo | path join $img.mise_config)
        let tools_only = {tools: ($src.tools? | default {})}
        let tools_only = if ($src.settings? | is-not-empty) { $tools_only | insert settings $src.settings } else { $tools_only }
        mkdir ($context | path join mise)
        $tools_only | to toml | save --force ($context | path join mise mise.toml)
        if ($img.mise_lock? | is-not-empty) {
            copy-into ($repo | path join $img.mise_lock) ($context | path join mise mise.lock)
        }
    }

    # A mise tool that must match a crate version in Cargo.lock (wasm-bindgen's
    # CLI must equal the wasm-bindgen crate, or generated bindings break).
    for check in ($img.version_checks? | default []) {
        let tools = open ($repo | path join $img.mise_config) | get -o tools | default {}
        let pinned = $tools | get -o $check.tool
        if $pinned == null { fail $"version check: ($check.tool) is not in ($img.mise_config)" }
        let locked = (
            open --raw ($repo | path join "Cargo.lock") | from toml | get package
            | where name == $check.cargo_package | get version | uniq
        )
        if ($locked | is-empty) { fail $"version check: ($check.cargo_package) is not in Cargo.lock" }
        if ($pinned | into string) not-in $locked {
            fail $"version check: ($img.mise_config) pins ($check.tool) = ($pinned), but Cargo.lock has ($check.cargo_package) ($locked | str join ', ')"
        }
    }

    # Submodules to bake, at the gitlink SHAs of the checked-out commit.
    let subs = $img.submodules? | default [] | each {|p| submodule-entry $repo $p }
    {submodules: $subs} | to json --indent 2 | save --force ($context | path join submodules.lock.json)

    # A manifest-only skeleton of the cargo workspace for `cargo fetch`: every
    # tracked Cargo.toml, Cargo.lock and cargo config, and an empty stub for
    # every tracked .rs file (so cargo finds every target). It is excluded from
    # the hash: what gets fetched is decided by Cargo.lock, which is hashed
    # when the config lists it under `files`.
    let exclude = if ($img.cargo_fetch? | default false) {
        let ws = $context | path join cargo-workspace
        let tracked = ^git -C $repo ls-files -- '*Cargo.toml' 'Cargo.lock' '.cargo/config.toml' 'rust-toolchain.toml' '*.rs' | lines
        for f in $tracked {
            let dest = $ws | path join $f
            mkdir ($dest | path dirname)
            if ($f | str ends-with ".rs") { "" | save --force $dest } else { ^cp -a ($repo | path join $f) $dest }
        }
        [cargo-workspace]
    } else { [] }

    let extra_args = $img.build_args? | default {}
    {
        name: $img.name
        build_args: ($extra_args | merge {BASE_IMAGE: $parent})
        exclude: $exclude
    }
}

def write-github-output [values: record] {
    let out = $env.GITHUB_OUTPUT? | default ""
    if ($out | is-empty) { fail "--github-output needs GITHUB_OUTPUT (run inside GitHub Actions)" }
    for row in ($values | transpose k v) {
        let v = $row.v | into string
        if ($v | str contains "\n") {
            let delim = $"EOF_(random chars --length 16)"
            $"($row.k)<<($delim)\n($v)\n($delim)\n" | save --append $out
        } else {
            $"($row.k)=($v)\n" | save --append $out
        }
    }
}

# Stage a layer's build context and print its plan (JSON).
def "main prepare" [
    --config: path          # the consumer's config.toml
    --layer: string         # base | image
    --flavor: string = "default"
    --parent: string = ""   # image layer only: the base image, pinned by digest
    --out: path             # output directory (the context goes to <out>/context)
    --repo: path = "."      # repository root (for files, submodules, git)
    --github-output         # also write the plan to $GITHUB_OUTPUT
] {
    if ($config == null) or ($out == null) or ($layer == null) { fail "--config, --layer and --out are required" }
    let cfg = load-config $config
    let repo = $repo | path expand
    let out = $out | path expand
    let context = $out | path join context
    if ($context | path exists) { rm -r $context }
    mkdir $context

    let staged = match $layer {
        "base" => (stage-base $cfg $flavor $context)
        "image" => (stage-image $cfg $flavor $repo $parent $context)
        _ => (fail $"unknown layer `($layer)` \(expected base or image)")
    }
    let lines = hash-context $context $staged.exclude
    let hash = compute-hash $lines $staged.build_args $parent
    let repository = repository $cfg $staged.name $flavor
    let tag = $"hash-($hash)"
    let plan = {
        layer: $layer
        flavor: $flavor
        hash: $hash
        repository: $repository
        tag: $tag
        ref: $"($repository):($tag)"
        cache_ref: $"($repository):buildcache"
        context: $context
        dockerfile: ($context | path join Dockerfile)
        platform: $LINUX_PLATFORM
        build_args: ($staged.build_args | transpose k v | each {|r| $"($r.k)=($r.v)" } | str join "\n")
        registry_host: ($cfg.registry.host? | default "docker.io")
        registry_username: (registry-username $cfg)
    }
    $lines | str join "\n" | save --force ($out | path join inputs.txt)
    $plan | to json --indent 2 | save --force ($out | path join plan.json)
    if $github_output { write-github-output $plan }
    $plan | to json --indent 2
}

# Is <ref> in the registry? Prints {exists, digest}. A missing image, or one
# that cannot be read without credentials, counts as missing; any other
# failure (network, registry errors) is an error.
def "main exists" [
    ref: string
    --github-output
] {
    let res = do { ^docker buildx imagetools inspect $ref --format '{{json .Manifest}}' } | complete
    let result = if $res.exit_code == 0 {
        let manifest = $res.stdout | from json
        {exists: true, digest: $manifest.digest}
    } else {
        let err = $res.stderr | str lowercase
        if ($err =~ 'not found|manifest unknown|name unknown|no such manifest') {
            {exists: false, digest: ""}
        } else if ($err =~ 'unauthorized|denied|authentication required') {
            print --stderr $"::warning::cannot read ($ref) without registry credentials; treating it as missing"
            {exists: false, digest: ""}
        } else {
            fail $"checking ($ref) failed:\n($res.stderr)"
        }
    }
    if $github_output { write-github-output $result }
    $result | to json
}

# Point extra tags at an image that is already in the registry.
def "main promote" [
    ref: string             # repository@sha256:... (or :tag)
    --tags: string          # space separated, e.g. "latest sha-1a2b3c4"
] {
    let tags = $tags | default "" | split row ' ' | where {|t| $t != '' }
    if ($tags | is-empty) { fail "--tags is empty" }
    let repo = $ref | str replace --regex '[@:][^/]*$' ''
    let args = $tags | each {|t| [-t $"($repo):($t)"] } | flatten
    ^docker buildx imagetools create ...$args $ref
}

def main [] {
    print "usage: nu images.nu <prepare|exists|promote> ... (see the header of this file)"
}
