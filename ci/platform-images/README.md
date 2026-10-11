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
├── mise/                   pinned tools for every platform (mise.toml + mise.lock:
│                           URL + sha256 for linux-x64, windows-x64, macos-x64)
├── images/
│   ├── linux/              layer 1: the generic Linux base image
│   │   ├── Dockerfile
│   │   ├── packages/*.txt  apt packages, one per line
│   │   └── bin/            build-time helpers (pi-mise-install, pi-install-browsers)
│   ├── windows/            layer 1 VM: dockurr/windows + guest/ provisioning (PowerShell)
│   ├── macos/              layer 1 VM: dockurr/macos + guest/ provisioning (bash)
│   └── vm/bin/             pi-vm-entry: the VM images' container entrypoint
├── scripts/                nushell, also in every image
│   ├── images.nu           stage a layer's context, hash it, look it up, tag it
│   ├── vm.nu               boot a VM, wait, push/exec/pull over SSH, start a JIT runner
│   ├── install-rust-toolchain.nu
│   ├── bake-submodules.nu
│   ├── link-submodules.nu  (symlinks on Linux/macOS, junctions on Windows)
│   └── smoke-test.nu
└── actions/                composite actions
    ├── setup-nu            nushell on a host runner, from the lock (no API, checksummed)
    ├── ensure-image        hash → look up → build/push (or export) one layer
    ├── link-submodules     baked submodules into a checkout
    ├── boot-vm             start a Windows/macOS VM image and wait until it is provisioned
    └── register-runner     register the VM as an ephemeral just-in-time runner (GitHub App)
```

The reusable workflows are
[`.github/workflows/platform-images.yaml`](../../.github/workflows/platform-images.yaml)
(ensure the images exist) and
[`.github/workflows/platform-images-vm.yaml`](../../.github/workflows/platform-images-vm.yaml)
(run work in a Windows/macOS VM). GitHub only runs reusable workflows from
`.github/workflows`, so they live there. If the unit moves to its own
repository, they move along and callers reference them as
`<owner>/<repo>/.github/workflows/<file>@<ref>`.

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
- **mise tools** (`mise/mise.toml`, shared by every platform): nushell, node + npm, deno,
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

- **A tool in layer 1:** edit `mise/mise.toml`, shared by every platform, then refresh
  its lock with `mise lock --platform linux-x64,windows-x64,macos-x64`
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

## Windows and macOS: VM images

Windows and macOS don't run in Linux containers, so they come as VM images.
The unit uses [dockurr](../../infrastructure/docker/windows/readme.md)
images: a Linux container that runs QEMU/KVM, installs the OS into
`/storage` on first boot and serves a web viewer on port 8006. The layering
and content hashing are the same as for Linux:

| Layer | Image | Built from | Contains |
|---|---|---|---|
| 1 | `<namespace>/<vm.<os>.base_name>` (`ci-windows`, `ci-macos`) | `images/<os>/Dockerfile`, FROM the dockurr image (`base_from`, pinned by digest) | the provisioning that runs **inside** the VM, the shared `mise/` tools, `scripts/` |
| 2 | `<namespace>/<vm.<os>.name>` (`ewe-platform-ci-windows`, …) | the repository's `vm.<os>.dockerfile` | the repository's provisioning layer (`layer_dir`) with its toolchain file, mise tools and submodule lock |

The images hold **no Windows or macOS bits**, only provisioning scripts:
dockurr downloads the OS from Microsoft or Apple when a VM is first
installed. (`images.nu prepare --layer vm-base|vm-image --os windows|macos`
stages the contexts; `platform-images.yaml` with `os: windows|macos`
ensures the images like the Linux ones.)

### What gets installed in the VM

| | Windows (unattended) | macOS (after the one-time manual install) |
|---|---|---|
| How | dockurr runs `C:\OEM\install.bat` at the end of its unattended install. It runs `provision.ps1` (layer 1), then each `C:\OEM\layers\<name>\install.ps1` (layer 2). | `vm.nu provision` copies `guest/` (and the layers) in over SSH and runs `provision.sh`, then each `layers/<name>/install.sh`. |
| System | OpenSSH Server (key auth, default shell pwsh), long paths, no Windows Update reboots, Defender real-time scanning off (best effort) | Xcode Command Line Tools (via `softwareupdate`) |
| Toolchain | Visual Studio 2022 Build Tools (MSVC + Windows SDK), LLVM 22 (`LIBCLANG_PATH`), Git for Windows (+ bash), PowerShell 7, Vulkan SDK (`VULKAN_SDK`), WebView2 | clang, git, make, macOS SDK (CLT), Vulkan SDK / MoltenVK (`VULKAN_SDK`) |
| mise tools | nushell, node, python 3.12, cmake, ninja, protoc, sccache, deno, binaryen, wasm-pack, wasmtime, actionlint, cargo-binstall: the same versions as Linux, from the same lock | same |
| Rust | rustup, no toolchain (`C:\rust`); layer 2 installs the pinned one | rustup (`/usr/local/rust`); layer 2 installs the pinned one |
| CI | GitHub Actions runner (`C:\actions-runner`), started per job with a JIT config | GitHub Actions runner (`/usr/local/actions-runner`) |

Every download is checked before it runs. GitHub releases, rustup and mise
have a pinned sha256 (`guest/components.json` / `components.env`, plus
`mise.lock`). The Microsoft and LunarG downloads, which are only published
behind moving redirect URLs (VS Build Tools, WebView2, Vulkan SDK), are
checked by Authenticode or code signature against the expected signer.
Provisioning stops at the first failure. It records the outcome in
`C:\OEM\status\` or `/usr/local/platform-images/status/` (`provisioned.ok`
or `provision.failed`, `provision.log`, `versions.txt`), and `vm.nu wait`
reports it.

### Two ways to use a VM in a workflow

Both run on a **Linux host job** that boots the VM: `ubuntu-latest` for
Windows, or a self-hosted Linux host (required for macOS, see licensing).

1. **SSH mode** (`platform-images-vm.yaml` with `mode: ssh`): the host job
   copies the checkout into the VM (`vm.nu push`) and runs a script there
   (`vm.nu exec`: PowerShell 7 on Windows, bash on macOS; stops at the first
   failure). It's simple, needs no extra secrets, and works for anything
   that is just commands. Use it for builds and tests.
2. **Runner mode** (`mode: runner`): the host job registers the VM as an
   **ephemeral just-in-time runner** of the repository with a unique label
   (`register-runner`). A separate job in the caller,
   `runs-on: [self-hosted, <label>]`, then runs *inside* the VM with normal
   `uses:` steps (checkout, caches, actions). The host keeps the VM up until
   that one job is done (`vm.nu wait-runner`); the runner then unregisters
   itself. Use it when the work needs actions, not just commands.

`.github/workflows/platform-vms.yaml` shows both: `windows-ssh` (nightly)
and `windows-runner` + `windows-in-vm` (manual).

### Runner registration: GitHub App and secrets

Registering a JIT runner calls
`POST /repos/{owner}/{repo}/actions/runners/generate-jitconfig`, which needs
**Administration: read & write** on the repository. `register-runner` mints
a short-lived installation token for a GitHub App with
`actions/create-github-app-token@v3.2.0` (`client-id` + `private-key`), so
no long-lived token is stored.

- Create a GitHub App with the repository permission **Administration: Read
  and write** (nothing else), and install it on the repository.
- Store its **Client ID** (not the numeric App ID) and a **private key** as
  repository secrets. ewe_platform uses `EWE_RUNNER_REGISTRA_CLIENT_ID` and
  `EWE_RUNNER_REGISTRA_PRIVATE_KEY`; `platform-images-vm.yaml` takes the
  secret *names* as inputs (`app-client-id-secret`,
  `app-private-key-secret`, `registry-password-secret`), so other
  repositories can use their own. Callers pass `secrets: inherit`.
- Runner mode only runs on **trusted events** (push, workflow_dispatch,
  schedule). It refuses `pull_request` and `pull_request_target`, both in
  the workflow and in `register-runner`, because a registered runner runs
  whatever job asks for its label. Labels are unique per run
  (`<prefix>-<run id>-<attempt>`), so only the intended job lands on it.

### KVM on GitHub-hosted runners

GitHub's standard Linux runners (`ubuntu-latest`, 4 vCPU, 16 GB) expose
`/dev/kvm`. `boot-vm` checks it exists and installs GitHub's documented
udev rule, so the device is usable by non-root processes too:

```sh
echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' \
  | sudo tee /etc/udev/rules.d/99-kvm4all.rules
sudo udevadm control --reload-rules && sudo udevadm trigger --name-match=kvm
```

The dockurr container itself runs as root and only needs
`--device /dev/kvm --device /dev/net/tun --cap-add NET_ADMIN`, which
`vm.nu up` passes. The VM disk goes to `/mnt` on GitHub-hosted runners
(`/mnt` has the most free space). Self-hosted hosts need hardware
virtualisation enabled; check with `kvm-ok` (package `cpu-checker`).

### How long it takes

These are estimates for a fresh disk on `ubuntu-latest`. GitHub-hosted runs
always start from a fresh disk.

| Step | Windows (Server 2022) | macOS |
|---|---|---|
| ISO download | 5-10 min (~6 GB from Microsoft) | first boot only, manual |
| OS install | 15-25 min, unattended | ~1 h once, by hand (VNC) |
| Layer 1 provisioning | 25-40 min (VS Build Tools alone 10-20) | 15-30 min |
| Layer 2 (ewe_platform) | 5-10 min (toolchain, tools, submodules) | 5-10 min |
| Total before the work starts | **~50-85 min** | provisioning only, if the disk is kept |

That is why the VM workflows are opt-in (nightly, manual) and never run on
pull requests.

### Can the provisioned disk be baked in or cached?

Not publicly, and not on GitHub-hosted runners:

- **Licensing.** A provisioned disk contains Windows or macOS.
  Redistributing it in a public image isn't allowed (Microsoft's
  evaluation terms, Apple's EULA). That's why the images carry only scripts
  and dockurr downloads the OS from the vendor.
- **Size.** A provisioned Windows disk is about 40-50 GB (OS, VS Build
  Tools, Vulkan SDK, toolchain), roughly 15-20 GB compressed. That's above
  the 10 GB GitHub Actions cache limit per repository and impractical to
  pull per job.
- **What works:** a **persistent `/storage` on a self-hosted Linux host**.
  Provisioning happens once, and later runs boot the installed disk in about
  a minute (`storage` input; reuse the authorised key with `ssh-key-path`).
  This is the recommended setup for frequent Windows jobs, and the only one
  for macOS. A provisioned disk could also be pushed to a *private*
  registry for internal reuse. That's not implemented: it needs a licence
  decision (internal copies of an evaluation install) that this unit can't
  make for you.

### Licensing

- **macOS:** Apple's licence permits running macOS **only on Apple
  hardware**. dockurr/macos says the same. Running the macOS image on
  GitHub-hosted runners, or any non-Apple machine, violates it. Use it only
  on a self-hosted Linux host that is Apple hardware (e.g. an Intel Mac
  running Linux). ewe_platform's workflow targets runners labelled
  `apple-hardware` and never GitHub-hosted runners.
- **Windows:** the image defaults to `VERSION=2022`, **Windows Server 2022
  Standard Evaluation**: dockurr downloads Microsoft's evaluation ISO, valid
  for **180 days** from installation. Fresh-disk CI runs never get near
  that, but a persistent disk on a self-hosted host expires; reinstall by
  emptying `/storage`. Other `version` values include `11l` (Windows 11
  Enterprise LTSC Evaluation, 90 days) and `11` (Windows 11 Pro,
  unactivated: no time limit, but you still need a licence to use it). For
  production, use an edition you hold a licence for (dockurr's `KEY`).
  dockurr's readme notes that its keys are generic installation keys, not
  activations.

### macOS: the one-time manual install

On the self-hosted Linux host (Apple hardware), with a persistent storage
directory:

1. Start the image: `nu ci/platform-images/scripts/vm.nu up --os macos
   --image docker.io/ewestudios/ewe-platform-ci-macos:<tag> --name ci-mac
   --storage /var/lib/platform-images/macos/storage --key <key>`.
   Open the web viewer at `http://127.0.0.1:8006`.
2. Follow dockurr's install steps: Disk Utility → erase the largest
   `Apple Inc. VirtIO Block Media` disk → Reinstall macOS → skip Apple ID.
   Create the user `ci` (or set `PI_VM_USER`).
3. In the VM: System Settings → General → Sharing → **Remote Login** on.
   Then add the public key to `~ci/.ssh/authorized_keys`, and give `ci`
   passwordless sudo:
   `echo 'ci ALL=(ALL) NOPASSWD: ALL' | sudo tee /etc/sudoers.d/ci`.
4. From then on, workflows (or `vm.nu provision`) run the provisioning over
   SSH. It's idempotent, so re-running it after an image update is fine.

### Running a VM by hand

```sh
nu ci/platform-images/scripts/vm.nu up --os windows --image docker.io/ewestudios/ewe-platform-ci-windows:<tag> \
   --name win --storage /mnt/win --key ~/.ssh/ci          # ~/.ssh/ci.pub is authorised on a fresh install
nu ci/platform-images/scripts/vm.nu wait --os windows --name win --key ~/.ssh/ci
nu ci/platform-images/scripts/vm.nu push --os windows --key ~/.ssh/ci --src . --dest C:/work/repo
echo 'cargo build -p foundation_core' > /tmp/run.ps1
nu ci/platform-images/scripts/vm.nu exec --os windows --key ~/.ssh/ci --cwd C:/work/repo --script /tmp/run.ps1
nu ci/platform-images/scripts/vm.nu down --name win
```

`vm.nu up` publishes the VM's SSH (`--port`, default 2222) and dockurr's web
viewer (`--web-port`, default 8006) on 127.0.0.1 only. It doesn't publish
RDP; run the container yourself with `-p 3389:3389` if you need it. The
default Windows account is `Docker` / `admin`; set `USERNAME`/`PASSWORD` on
the container for anything long-lived.

## Limits

- `linux/amd64` only. Google Chrome has no Linux arm64 build, and
  GitHub-hosted runners are amd64.
- Layer 1 is about 5.5 GB unpacked (default flavor, with browsers). The
  CUDA flavor adds about 10 GB, which is why the workflows free runner disk
  before building or running it.
- The VM images are x86_64 guests: Windows x64, and Intel macOS (what
  dockurr/macos emulates). Their tool lock covers windows-x64 and macos-x64.
  pitchfork has no Intel-macOS build and is limited to Linux/Windows in
  ewe_platform's `mise.toml`.
