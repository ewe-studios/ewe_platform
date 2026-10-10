#!/usr/bin/env nu
# Smoke-test a platform-images Linux image from the inside:
#
#   docker run --rm <image> nu /opt/platform-images/scripts/smoke-test.nu --layer base --browsers [chrome firefox]
#   docker run --rm <image> nu /opt/platform-images/scripts/smoke-test.nu --layer image --browsers [chrome firefox]
#
# base:  every tool runs; pkg-config finds the desktop/graphics stacks;
#        Vulkan enumerates lavapipe; GL and EGL render with llvmpipe (under
#        Xvfb / surfaceless); the requested browsers start, and headless
#        Chrome gets WebGL2 and WebGPU on the software stack; with --cuda,
#        nvcc works.
# image: the base checks, plus the default Rust toolchain and tools, the
#        baked submodules at the SHAs in their lock, and a warm cargo registry.
#
# Prints a table and exits non-zero if any check failed.

# Run an external command; its first non-empty output line, or an error.
def run [bin: string, ...args: string]: nothing -> string {
    let out = ^$bin ...$args | complete
    if $out.exit_code != 0 {
        error make --unspanned {msg: ($"exit ($out.exit_code): ($out.stderr) ($out.stdout)" | lines | first 3 | str join " | ")}
    }
    $out.stdout | lines | where {|l| $l | str trim | is-not-empty } | first 1 | str join "" | str trim
}

# Full stdout of an external command, or an error.
def output [bin: string, ...args: string]: nothing -> string {
    let out = ^$bin ...$args | complete
    if $out.exit_code != 0 {
        error make --unspanned {msg: ($"exit ($out.exit_code): ($out.stderr)" | lines | first 3 | str join " | ")}
    }
    $out.stdout
}

def check [name: string, body: closure]: nothing -> record {
    try {
        let detail = do $body | into string
        {check: $name, ok: true, detail: ($detail | str substring 0..<110)}
    } catch {|e|
        {check: $name, ok: false, detail: ($e.msg | str substring 0..<110)}
    }
}

def base-checks [browsers: list<string>, cuda: bool]: nothing -> list<record> {
    let tools = [
        [nu --version] [mise --version] [node --version] [npm --version] [npx --version] [deno --version]
        [cmake --version] [ninja --version] [protoc --version] [gn --version]
        [sccache --version] [cargo-binstall -V] [wasm-opt --version] [wasm-pack --version]
        [wasmtime --version] [actionlint --version] [rustup --version]
        [clang --version] [ld.lld --version] [gcc --version] [python3 --version] [git --version]
        [glslc --version] [spirv-val --version] [Xvfb -help]
    ]
    mut results = $tools | each {|t| check $t.0 {|| run $t.0 ...($t | skip 1) } }
    let modules = [
        gtk+-3.0 webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0 dbus-1 ayatana-appindicator3-0.1 librsvg-2.0
        gl egl glesv2 osmesa gbm vulkan x11 x11-xcb xcb xkbcommon xkbcommon-x11 xrandr xi xcursor wayland-client
        openssl zlib sqlite3 libudev
    ]
    $results = $results | append ($modules | each {|m| check $"pkg-config ($m)" {|| run pkg-config --modversion $m } })

    $results = $results | append (check "vulkan: lavapipe device" {||
        let dev = output vulkaninfo --summary | lines | where {|l| $l =~ 'deviceName' } | str join " " | str trim
        if not ($dev =~ 'llvmpipe') { error make --unspanned {msg: $"no lavapipe device: ($dev)"} }
        $dev
    })
    $results = $results | append (check "opengl: glx under xvfb" {||
        let r = output xvfb-run -a glxinfo -B | lines | where {|l| $l =~ 'OpenGL renderer' } | str join " " | str trim
        if not ($r =~ 'llvmpipe') { error make --unspanned {msg: $"unexpected renderer: ($r)"} }
        $r
    })
    $results = $results | append (check "opengl: egl surfaceless" {||
        let r = output eglinfo -p surfaceless | lines | where {|l| $l =~ 'OpenGL.*renderer' } | first 1 | str join " " | str trim
        if not ($r =~ 'llvmpipe') { error make --unspanned {msg: $"unexpected renderer: ($r)"} }
        $r
    })
    if "chrome" in $browsers {
        $results = $results | append (check "chrome" {|| run google-chrome --version })
        $results = $results | append (check "chrome: webgl2 + webgpu" {||
            # The page reports the contexts it gets; --virtual-time-budget lets
            # the async WebGPU adapter request finish before the DOM is dumped.
            let page = "<body><script>(async()=>{const c=document.createElement('canvas');const gl=!!c.getContext('webgl2');let gpu='no-api';if(navigator.gpu){const a=await navigator.gpu.requestAdapter();gpu=a?'adapter':'no-adapter';}document.body.textContent='webgl2='+gl+' webgpu='+gpu;})()</script></body>"
            let dom = (output google-chrome --headless=new --no-sandbox --disable-dev-shm-usage
                --enable-unsafe-webgpu --enable-features=Vulkan --use-angle=swiftshader --enable-unsafe-swiftshader
                --virtual-time-budget=10000 --dump-dom $"data:text/html,($page | url encode)")
            let found = $dom | parse --regex 'webgl2=(?<gl>\w+) webgpu=(?<gpu>[\w-]+)'
            if ($found | is-empty) { error make --unspanned {msg: "the page did not report"} }
            if $found.0.gl != "true" or $found.0.gpu != "adapter" {
                error make --unspanned {msg: $"webgl2=($found.0.gl) webgpu=($found.0.gpu)"}
            }
            $"webgl2=($found.0.gl) webgpu=($found.0.gpu)"
        })
    }
    if "firefox" in $browsers {
        $results = $results | append (check "firefox" {|| run firefox --version })
    }
    if $cuda {
        $results = $results | append (check "nvcc" {|| output nvcc --version | lines | last })
    }
    $results
}

def image-checks []: nothing -> list<record> {
    let tools = [[rustc --version] [cargo --version] [cargo clippy --version] [cargo fmt --version]
        [wasm-bindgen --version] [wasm-bindgen-test-runner --version] [cargo-nextest --version] [cargo-audit --version]
        [bacon --version] [pnpm --version] [pitchfork --version]]
    mut results = $tools | each {|t| check ($t | str join " ") {|| run $t.0 ...($t | skip 1) } }
    $results = $results | append (check "rust targets" {|| output rustup target list --installed | lines | str join " " })
    $results = $results | append (check "rust components" {|| output rustup component list --installed | lines | str join " " })
    let lock = $env.PLATFORM_IMAGES_SUBMODULES_LOCK? | default ""
    if ($lock | is-empty) {
        $results = $results | append {check: "submodules lock", ok: false, detail: "PLATFORM_IMAGES_SUBMODULES_LOCK is not set"}
    } else {
        let root = $lock | path dirname
        for sub in (open $lock | get submodules) {
            $results = $results | append (check $"submodule ($sub.path)" {||
                let head = run git -c "safe.directory=*" -C ($root | path join $sub.path) rev-parse HEAD
                if $head != $sub.sha { error make --unspanned {msg: $"at ($head), lock says ($sub.sha)"} }
                $head
            })
        }
    }
    $results = $results | append (check "cargo registry" {||
        let crates = glob $"($env.CARGO_HOME)/registry/cache/*/*.crate" | length
        if $crates == 0 { error make --unspanned {msg: "no .crate files in the registry cache"} }
        $"($crates) crates cached"
    })
    $results
}

def main [
    --layer: string = "base"            # base | image
    --browsers: list<string> = []       # browsers the image should have
    --cuda                              # the image should have the CUDA toolkit
] {
    let results = match $layer {
        "base" => (base-checks $browsers $cuda)
        "image" => ((base-checks $browsers $cuda) | append (image-checks))
        _ => { error make --unspanned {msg: $"unknown layer ($layer)"} }
    }
    print ($results | table --index false --width 200)
    let failed = $results | where ok == false
    if not ($failed | is-empty) {
        print $"($failed | length) check\(s) failed: ($failed | get check | str join ', ')"
        exit 1
    }
    print $"all ($results | length) checks passed"
}
