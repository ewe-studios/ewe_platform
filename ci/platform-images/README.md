# platform-images

CI images built in layers, content-addressed, and rebuilt only when their
inputs change. This directory is a self-contained unit with nothing specific
to one repository in it: a repository describes its own images in a
`config.toml` (ewe_platform's is
[`ci/ewe-platform-image/config.toml`](../ewe-platform-image/config.toml)) and
uses the unit through its scripts, actions and the reusable workflow, the same
way any other repository would.

```
ci/platform-images/
├── images/linux/           layer 1: the generic Linux base image
│   ├── Dockerfile
│   ├── packages/*.txt      apt packages, one per line
│   ├── mise/               pinned tools (mise.toml + mise.lock)
│   └── bin/                build-time helpers (pi-mise-install, pi-install-browsers)
├── scripts/                nushell, also copied into the image at /opt/platform-images/scripts
│   ├── images.nu           stage a layer's context, hash it, look it up, tag it
│   ├── install-rust-toolchain.nu
│   ├── bake-submodules.nu
│   ├── link-submodules.nu
│   └── smoke-test.nu
└── actions/                composite actions
    ├── setup-nu            nushell on a host runner, from the lock (no API, checksummed)
    ├── ensure-image        hash → look up → build/push (or export) one layer
    └── link-submodules     baked submodules into a checkout
```

The reusable workflow is
[`.github/workflows/platform-images.yaml`](../../.github/workflows/platform-images.yaml):
GitHub only runs reusable workflows from `.github/workflows`, so it lives
there. If the unit moves to its own repository, it moves along and callers
reference it as `<owner>/<repo>/.github/workflows/platform-images.yaml@<ref>`.

## Layers

| Layer | Image | Built from | Contains |
|---|---|---|---|
| 1 | `<namespace>/<base.name>` (`-<flavor>`) | `images/linux/Dockerfile`, FROM the flavor's upstream image | everything generic: compilers, GTK/WebKit, Mesa/Vulkan/Xvfb, browsers, mise tools, rustup |
| 2 | `<namespace>/<image.name>` (`-<flavor>`) | the repository's Dockerfile, FROM layer 1 **pinned by digest** | everything repository-specific: the pinned toolchain, the repository's mise tools, baked submodules, a warm cargo registry |

**Flavors** are defined by the consumer (`[base.flavors.<name>]`) and differ
only in the upstream image layer 1 starts from, e.g. `ubuntu:24.04` (the
default) or `nvidia/cuda:12.9.2-devel-ubuntu24.04` (`cuda`, with nvcc and the
CUDA libraries). Each flavor gets its own repositories (`ci-base`,
`ci-base-cuda`, …). The CUDA flavor is a separate image rather than part of
the default one because every CI job pulls the image: the default flavor
pulls in about 2 GB, and CUDA would add about 5 GB more to every job that
doesn't need it.

### What layer 1 contains

- **Build tools:** gcc 13, clang/lld/llvm 18, libclang, libomp, pkg-config,
  autotools, gn, python3, git, git-lfs, shellcheck, and from mise: cmake
  3.31, ninja, protoc
- **Common -dev libraries:** OpenSSL, zlib, zstd, xz, bzip2, SQLite, curl,
  udev
- **Desktop/webview (Tauri, wry/tao):** GTK 3, WebKitGTK 4.1 +
  JavaScriptCore, libsoup 3, D-Bus, Ayatana AppIndicator, librsvg, xdo,
  fonts
- **Graphics without a GPU:** Mesa GL/EGL/GLES/OSMesa/GBM (llvmpipe), Vulkan
  loader and headers, lavapipe, vulkan-tools, validation layers, glslc,
  glslang, SPIR-V headers and tools, X11/xcb/xkbcommon/Wayland -dev
  libraries, Xvfb
- **Browsers:** Google Chrome stable and Firefox (from Google's and Mozilla's
  apt repositories; Ubuntu's are snap shims that don't run in containers),
  as listed in `[base] browsers`. `chromium`/`chromium-browser` point at
  Chrome.
- **mise tools** (`images/linux/mise/mise.toml`): nushell, node + npm, deno,
  cmake, ninja, protoc, sccache, cargo-binstall, binaryen (wasm-opt),
  wasm-pack, wasmtime, actionlint
- **rustup**, with no toolchain (layer 2 installs the repository's)

Dawn/WebGPU-native builds need gn, ninja and python3, which are all in
layer 1. depot_tools and the Dawn sources are per repository: a repository
that builds Dawn bakes them in its layer 2.

### GPU-less graphics and headless runs

Layer 1 sets:

| Variable | Value | Effect |
|---|---|---|
| `VK_ICD_FILENAMES`, `VK_DRIVER_FILES` | `/usr/share/vulkan/icd.d/lvp_icd.json` | Vulkan (wgpu, llama.cpp's Vulkan backend, …) uses lavapipe |
| `LIBGL_ALWAYS_SOFTWARE` | `1` | Mesa GL/EGL use llvmpipe |

There is no X server running. For a display, wrap the command:
`xvfb-run -a cargo test ...` (or start `Xvfb :99 &` and set `DISPLAY=:99`).
EGL also works without one (`eglinfo -p surfaceless`). Headless Chrome gets
WebGL2 and WebGPU on SwiftShader with
`--headless=new --enable-unsafe-webgpu --use-angle=swiftshader --enable-unsafe-swiftshader`
(the smoke test checks both), and wants `--shm-size=2g` on the container
(`container.options` in a workflow).

To use a real GPU instead (the CUDA flavor on a GPU host with the NVIDIA
container toolkit), unset `VK_ICD_FILENAMES`/`VK_DRIVER_FILES` and
`LIBGL_ALWAYS_SOFTWARE` so the driver's ICDs are found.

## Content hashes and tags

`images.nu prepare` copies exactly what a layer's build uses into a fresh
build context, and hashes it: every file's path, executable bit and sha256,
the build arguments, and for layer 2 the parent reference (layer 1 pinned by
digest). The tag is `hash-<first 20 hex digits>`.

- Layer 1's inputs: `images/linux/**`, `scripts/**`, the flavor's upstream
  image reference, the browser list and extra packages from the config.
- Layer 2's inputs: the consumer's Dockerfile, the files its config lists
  (`files`), the `[tools]`/`[settings]` of its `mise.toml` plus `mise.lock`,
  a submodules lock with the URL and **gitlink SHA** of each baked submodule
  (`git ls-tree HEAD <path>`), its build args, and layer 1's digest.
  The cargo workspace skeleton used to warm the registry is staged but not
  hashed; `Cargo.lock` (hashed through `files`) decides what gets fetched.

So a submodule bump, a toolchain bump, a new tool version or a Dockerfile
edit produces a new tag, and anything else (code changes, task edits in
mise.toml, README edits) reuses the existing image. A weekly rebuild of
layer 1 re-pushes the same layer 1 tag with updated packages. Its new
digest changes layer 2's hash, so layer 2 is rebuilt on top of it.

Besides the hash tags, the publish workflow points `latest` and
`sha-<commit>` at the images it smoke-tested. Each repository also has a
`buildcache` tag (the registry build cache, `mode=max`), so a rebuild after
a small input change only redoes the layers that changed.

Run it locally to see what a change does to the hash:

```sh
nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml \
   --layer base --out /tmp/pi/base
cat /tmp/pi/base/inputs.txt     # every hashed file
```

## How CI uses it

```
           ┌───────────────────────── image job (reusable workflow) ─────────────────────────┐
commit ──► │ hash layer 1 ── <ns>/ci-base:hash-… in Docker Hub? ── no ─► build + push        │
           │        │ yes                                                    │               │
           │        ▼                                                        ▼               │
           │ layer 1 digest ─► hash layer 2 ─ <ns>/<image>:hash-… there? ─ no ─► build + push │
           │                                         │ yes                        │          │
           └─────────────────────────────────────────┼────────────────────────────┼──────────┘
                                                     ▼                            ▼
                                       image-ref = <ns>/<image>@sha256:…  (pinned digest)
                                                     │
            package jobs: container: image-ref ──────┤
              checkout (no submodules)               │
              link-submodules ── gitlink SHA == baked SHA? ── yes ─► symlink /opt/…/<path>
                                                     │          no ─► fetch that submodule, warn "image stale"
              build / test (nothing installed)       ▼
```

1. The `image` job (the reusable workflow) runs `ensure-image` for layer 1
   and then layer 2. Each one hashes its inputs and checks the registry with
   `docker buildx imagetools inspect`. It builds only if the tag is missing,
   and pushes `:hash-…` so every later run, branch and job reuses it.
2. Jobs run with `container: image-ref` (layer 2 pinned by digest), check out
   without submodules, and run `link-submodules`.
3. **link-submodules** compares each baked submodule's SHA (from the lock
   the image carries, `$PLATFORM_IMAGES_SUBMODULES_LOCK`) with the gitlink
   SHA of the checked-out commit:
   - **match:** the path becomes a symlink to the baked copy, with no network.
     `--mode submodule` instead clones from the baked copy into a real,
     git-clean submodule checkout. Use it when a tool needs a clean
     `git status`, such as `cargo publish`.
   - **mismatch** (a pull request bumps a submodule): a warning annotation
     says the image is stale, and that submodule is fetched at the right
     SHA. CI is never wrong, just slower. Here the PR's own commit also
     changes the hash, so the image job has already built a new image and
     the mismatch only happens with an older image (e.g. `latest`).

### Pull requests without registry credentials

Pull requests from forks, and Dependabot's, get no secrets:

- If the image for their inputs exists, nothing changes: `container:` pulls
  it anonymously (the runner skips `docker login` when the credentials are
  empty). The repositories must be **public** on Docker Hub.
- If it has to be built, the image job builds both layers without pushing.
  Layer 1 is exported as an OCI layout that layer 2 builds FROM, and layer
  2 as a docker tarball uploaded as an artifact (`image-source: artifact`).
  `container:` can only pull from a registry, so the caller runs a second
  job that downloads the tarball, `docker load`s it, and runs the same steps
  with `docker exec` (check.yaml's `package-artifact`). It's slower, but the
  results are real.

## Using it from another repository

1. Copy `ci/platform-images/` and `.github/workflows/platform-images.yaml`
   (or, once the unit lives in its own repository, reference those).
2. Write a config:

   ```toml
   [registry]
   host = "docker.io"
   namespace = "myorg"
   username = "myorg"            # PLATFORM_IMAGES_REGISTRY_USERNAME (repo variable) overrides

   [base]
   name = "ci-base"
   browsers = ["chrome"]          # or [] for none
   packages = ["libfoo-dev"]      # extra apt packages for layer 1

   [base.flavors.default]
   from = "docker.io/library/ubuntu:24.04"

   [image]
   name = "myrepo-ci"
   dockerfile = "ci/image/Dockerfile"
   files = ["rust-toolchain.toml", "Cargo.lock"]   # copied to files/, hashed
   mise_config = "mise.toml"                         # optional: [tools] only
   mise_lock = "mise.lock"                           # optional
   submodules = ["third_party/foo"]                  # baked at gitlink SHAs
   cargo_fetch = true                                # warm the cargo registry
   version_checks = [{ tool = "github:wasm-bindgen/wasm-bindgen", cargo_package = "wasm-bindgen" }]

   [image.build_args]
   SUBMODULES_DIR = "/opt/myrepo/submodules"
   ```

3. Write the layer 2 Dockerfile. The staged context has `files/<path>`,
   `mise/mise.toml` + `mise/mise.lock`, `submodules.lock.json` and
   `cargo-workspace/`. Layer 1 provides the helpers, so it's short:

   ```dockerfile
   ARG BASE_IMAGE
   FROM ${BASE_IMAGE}
   ARG SUBMODULES_DIR
   COPY files/rust-toolchain.toml /opt/myrepo/rust-toolchain.toml
   RUN nu /opt/platform-images/scripts/install-rust-toolchain.nu /opt/myrepo/rust-toolchain.toml --components "clippy"
   ENV PATH=/opt/myrepo/tools/bin:${PATH}
   COPY mise/ /opt/myrepo/mise/
   RUN pi-mise-install /opt/myrepo/mise /opt/myrepo/tools/bin
   COPY submodules.lock.json /opt/myrepo/submodules.lock.json
   RUN nu /opt/platform-images/scripts/bake-submodules.nu /opt/myrepo/submodules.lock.json "${SUBMODULES_DIR}"
   ENV PLATFORM_IMAGES_SUBMODULES_LOCK=${SUBMODULES_DIR}/submodules.lock.json
   COPY cargo-workspace/ /tmp/ws/
   RUN cd /tmp/ws && cargo fetch --locked && rm -rf /tmp/ws "${CARGO_HOME}/registry/src"
   ```

4. Call the reusable workflow and run jobs in the result:

   ```yaml
   jobs:
     image:
       uses: ./.github/workflows/platform-images.yaml
       with:
         config: ci/image/config.toml
       secrets:
         registry-password: ${{ secrets.REGISTRY_TOKEN }}
     test:
       needs: image
       if: needs.image.outputs.image-source == 'registry'
       runs-on: ubuntu-latest
       container:
         image: ${{ needs.image.outputs.image-ref }}
         credentials:
           username: ${{ needs.image.outputs.registry-username }}
           password: ${{ secrets.REGISTRY_TOKEN }}
       steps:
         - uses: actions/checkout@v4.4.0
         - uses: ./ci/platform-images/actions/link-submodules
         - run: cargo test
   ```

   Reusable workflow inputs: `config`, `flavor`, `force`, `no-cache`,
   `free-disk` (for big flavors). Secret: `registry-password`. Outputs:
   `image-ref`, `image-source` (`registry`/`artifact`), `artifact-name`,
   `registry-username`, `base-ref`, `image-hash`.

## Building locally

```sh
nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml --layer base --out /tmp/pi/base
docker buildx build /tmp/pi/base/context \
  --build-arg BASE_IMAGE=docker.io/library/ubuntu:24.04 --build-arg "BROWSERS=chrome firefox" \
  -t local/ci-base

nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml --layer image \
  --parent local/ci-base --out /tmp/pi/image
docker buildx build /tmp/pi/image/context \
  $(jq -r '.build_args | split("\n") | map("--build-arg=" + .) | join(" ")' /tmp/pi/image/plan.json) \
  -t local/ewe-platform-ci

docker run --rm local/ewe-platform-ci nu /opt/platform-images/scripts/smoke-test.nu --layer image --browsers "chrome firefox"
```

(The `jq` line passes the build args `prepare` computed; for the base, the
plan's `build_args` are `BASE_IMAGE` and `BROWSERS` as shown.)

## Updating pinned versions

- **A tool in layer 1:** edit `images/linux/mise/mise.toml`, then refresh
  its lock with `mise lock --platform linux-x64,linux-arm64,macos-arm64,windows-x64`
  in that directory (this needs GitHub API access, e.g. with `GITHUB_TOKEN`
  set). The image installs with `MISE_LOCKED=1` and checks every download
  against the lock's sha256.
- **mise itself / rustup:** `MISE_VERSION`/`MISE_SHA256` and
  `RUSTUP_VERSION`/`RUSTUP_SHA256` in `images/linux/Dockerfile` (the sha256
  from the release's `SHASUMS256.txt` / `rustup-init.sha256`).
- **apt packages:** `images/linux/packages/*.txt`.
- **The upstream base image:** the consumer's `[base.flavors.*] from`.

Every one of these changes the layer 1 hash, so the next CI run builds the
new base, and layer 2 on top of it.

## Limits

- `linux/amd64` only. Google Chrome has no Linux arm64 build, and
  GitHub-hosted runners are amd64.
- Layer 1 is about 5.5 GB unpacked (default flavor, with browsers). The
  CUDA flavor adds about 10 GB, which is why the workflows free runner disk
  before building or running it.
