---
feature: "Sandboxfile builder"
status: "pending"
priority: "medium"
model: "sonnet"
phase: "3"
depends_on: ["13-container-image-source"]
crates: ["foundation_platform_sandbox"]
---

# Feature 14: The `Sandboxfile` builder

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** virtual machine (VM); Secure Hash Algorithm 256 (SHA-256);
Hypertext Transfer Protocol (HTTP); behaviour-driven development (BDD); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

## 1. Goal

Build a sandbox image from a small build file, with no Docker. Each step that changes the
image runs in one of our own microVMs, so a build of code that is not trusted is as
isolated as a run.

```
FROM alpine:3.22
ENV PIP_NO_CACHE_DIR=1
WORKDIR /opt/app
ADD ./app /opt/app
FETCH https://example.com/tool.tar.gz sha256:9f2c...e1 AS /tmp/tool.tar.gz
RUN --network host apk add --no-cache python3
RUN --shell "python3 -" <<EOF
import compileall
compileall.compile_dir("/opt/app")
EOF
CMD ["python3", "/opt/app/main.py"]
```

```
agent-sandbox image build -f Sandboxfile --tag app .
agent-sandbox run --image app
```

## 2. Story

- **S1**: "A user writes a build file and gets an image."
- **S2**: "The user changes the last step and builds again; only that step runs."
- **S3**: "A `RUN` step fails; the user sees its output and its exit code; no image is made."
- **S4**: "A remote file changed since the build file was written."
- **S5**: "The build file has a mistake."
- **S6**: "A step needs the network, and the host has no driver that gives a guest network."
- **S7**: "A build is stopped halfway."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Parse | builder | a file | A list of steps, or an error with a line number. |
| U2 | Build | user | `ImageBuilder::build(file, context, opts)` | An `ImageRef`. |
| U3 | Watch | user | `build_with_progress` | Step start, cache hit, output, step end. |
| U4 | Cache | builder | each step | A snapshot for each step key. |

## 3. Bounded context

**Inside this feature:** the build file grammar, the parser, step keys, the step cache, the
executor that drives a build sandbox, the progress stream.

**Outside:** bases (feature 13 importer; feature 15 registry); the image store, `Writable`
mode and unpack (feature 12); downloads (feature 03 `fetch`).

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| `Sandboxfile` | The parsed build file: a list of steps. | `images::sandboxfile` |
| Step | One instruction with its arguments and its line number. | the parser |
| Build context | The directory that `ADD` paths are relative to. | the user |
| Step key | 32 hex characters that name the state after a step. | `images::build::keys` |
| Snapshot | A work image stored under a step key: `<cache_dir>/images/steps/<key>.ext4`. | the builder |
| Metadata | `env`, `workdir`, `user`, `cmd`, `kernel` gathered so far. | the builder |
| Build sandbox | A `Writable` sandbox on a copy of the last snapshot. One for each changing step. | the builder, through `SandboxControl` |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| parse | entity | build | Text to steps. |
| key | entity | each step | Computes the step key from the key before, the step and its inputs. |
| execute step | story | a cache miss | snapshot copy, boot, apply, stop, store under the key. |
| build | story | user | parse, base, steps, commit. |

## 4. Rules

### 4.1 Law worksheet

```
Entity: Step key
Conserved value:         key(i) = H(key(i-1), canonical step text, input hashes)
Bound:                   none
Inverse pairs:           none
Idempotent verbs:        key (a pure function)
Ordering pairs:          key(i-1) -> key(i)
Monotone values:         none
Freeze points:           the canonical text rules of version 1
Links to other entities: a snapshot file name = its key

Entity: Snapshot
Conserved value:         a snapshot file never changes after it is stored
Bound:                   total snapshot bytes <= BuildLimits.cache_max_bytes (oldest use removed first)
Inverse pairs:           store / evict
Idempotent verbs:        store of a key that exists (keeps the first)
Ordering pairs:          execute step -> store
Monotone values:         none
Freeze points:           content at store
Links to other entities: the final image's rootfs = the snapshot of the last changing step

Entity: Build (story)
Conserved value:         a failed build adds no image to the store; snapshots of the steps that passed stay (they are the cache)
Bound:                   each RUN <= step_timeout
Inverse pairs:           none
Idempotent verbs:        build of an unchanged file and context (every step is a cache hit; the same image key)
Ordering pairs:          steps run in file order; FROM is first
Monotone values:         step index
Freeze points:           none
Links to other entities: image key = H(last step key, final metadata)
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | Parsing is total: every input gives a `Sandboxfile` or an error with a line number. It never panics. |
| I2 | `print(parse(text))` parses to the same steps (the canonical form is stable). |
| I3 | A step key depends only on the key before it, the canonical step text and the content of its inputs. It does not depend on time, host paths or file modification times. |
| I4 | A build with no change runs no sandbox and returns the same image key. |
| I5 | A change to step `n` runs steps `n` and later, and no step before `n`. |
| I6 | A `RUN` step with a non-zero exit fails the build with `BuildStepFailed { line, exit }`; no image is committed; its work file is removed. |
| I7 | `FETCH` content is checked against its SHA-256 before it enters the image. A mismatch fails the build. |
| I8 | An `ADD` source cannot be outside the build context, also through a symbolic link. |
| I9 | A build sandbox has no network unless its step has `--network host`. |
| I10 | Files that `ADD` and `FETCH` put in the image are owned by root, with the mode of the rule in section 6.3. |
| I11 | Two builds at the same time that need the same step key give the same snapshot content, and neither corrupts the other. |
| I12 | The builder holds no build output whole in memory: `RUN` output streams to the progress consumer. |

### 4.3 Must have

- The instructions `FROM`, `KERNEL`, `ENV`, `WORKDIR`, `USER`, `ADD`, `FETCH`, `RUN`, `CMD`.
- `ImageBuilder`, `BuildOptions`, `BuildLimits`, `BuildProgress`.
- `agent-sandbox image build`.

### 4.4 Must do

- Print the failed step's line, text and last output on a failure.
- Choose a driver that can do what the steps need (section 6.6).

### 4.5 Must not

- Must not run a step on the host.
- Must not follow a redirect of a `FETCH` to a different scheme (`https` to `http`).

## 5. Acceptance scenarios

```gherkin
Feature: Sandboxfile builder

  Scenario: A build file becomes an image
    Given the build file of section 1 without the network step, and a context with ./app/main.py
    When the user builds it with the tag "app"
    And runs a sandbox from "app" with no command
    Then the program python3 /opt/app/main.py runs in /opt/app with PIP_NO_CACHE_DIR=1

  Scenario: An unchanged build does no work
    Given a finished build
    When the same build runs again
    Then every step reports a cache hit
    And no sandbox started
    And the image key is the same

  Scenario: Only the changed step and the later ones run
    Given a finished build of 5 changing steps
    When the text of step 4 changes and the build runs again
    Then steps 1 to 3 report a cache hit
    And steps 4 and 5 run

  Scenario: A change in an added file is a change in the step
    Given a finished build with ADD ./app /opt/app
    When one byte of ./app/main.py changes
    Then the ADD step and the later steps run

  Scenario: A touched file is not a change
    Given a finished build with ADD ./app /opt/app
    When the modification time of ./app/main.py changes and its content does not
    Then every step reports a cache hit

  Scenario: A failed RUN stops the build
    Given a build file with RUN sh -c "echo oops >&2; exit 3"
    When the user builds it
    Then the result is BuildStepFailed with the line number and exit 3
    And the progress stream had the Stderr output "oops\n"
    And the store has no new image

  Scenario: A changed remote file is refused
    Given FETCH with a SHA-256 that does not match the server's file
    When the user builds
    Then the result is ArtifactHashMismatch with the step's line number
    And the file is not in any snapshot

  Scenario: ADD cannot leave the context
    Given ADD ../secret /x, and ADD ./link /y where ./link is a symbolic link to /etc
    When the user builds each
    Then each fails with InvalidSpec that names the line

  Scenario: Added files belong to root
    Given ADD ./tool /usr/local/bin/tool where ./tool has mode 0755 and the host owner is uid 1000
    When the image runs "stat -c '%u %a' /usr/local/bin/tool"
    Then stdout is "0 755\n"

  Scenario: No network unless the step asks
    Given RUN wget -q -T 3 -O- http://example.com
    When the user builds on a driver with a guest network
    Then the step fails
    Given RUN --network host wget -q -T 5 -O /dev/null http://<local test server>
    Then the step passes

  Scenario: A network step on a host without a network driver is refused before any work
    Given a build file with RUN --network host and a host whose only usable driver has network_host false
    When the user builds
    Then the result is UnsupportedNetwork with the step's line number
    And no sandbox started

  Scenario: A syntax error names its line
    Given a build file whose line 3 is "ADD onlyone"
    When it is parsed
    Then the error is InvalidSpec with line 3 and the text "ADD needs a source and a destination"

  Scenario: A stopped build leaves a usable cache
    Given a build that is killed during step 3
    When the build runs again
    Then steps 1 and 2 report a cache hit
    And the build ends with a correct image
```

## 6. Technical breakdown

### 6.1 Grammar

- The file is UTF-8 text. One instruction for each logical line. A line that ends with `\`
  continues on the next line (the `\` and the line break are removed). A line whose first
  character after spaces is `#` is a comment. Empty lines are ignored.
- The instruction word is not case-sensitive. The canonical form is upper case.
- A here-document: an instruction line that ends with `<<WORD` takes the lines that
  follow, up to a line that is exactly `WORD`, as its text. Only `RUN` accepts one.
- The first instruction must be `FROM`. `FROM` appears one time.

| Instruction | Form | Rule |
|---|---|---|
| `FROM <base>` | `alpine:<version>`, `image:<tag-or-key>`, `docker:<name>`, `tar:<path>`, `oci:<reference>` | `oci:` needs feature 15; without it: `InvalidSpec` "oci: is not available in this build". |
| `KERNEL <name>` | a kernel name of feature 03's manifest, or `default` | Goes to the manifest. At most one. |
| `ENV <KEY>=<value>` | one pair; the value is the rest of the line; surrounding double quotes are removed | Metadata. Later steps and the final image see it. |
| `WORKDIR <path>` | absolute | Metadata. `RUN` steps after it run there; the slave makes it. |
| `USER <user>` | `uid`, `uid:gid`, `name`, `name:group` | Metadata, for the final image. `RUN` steps always run as root. |
| `ADD [--mode <octal>] <source> <destination>` | `source` is relative to the build context; `destination` is absolute | Section 6.3. |
| `FETCH <url> sha256:<64 hex> AS <destination>` | `https://` or `http://`; `destination` absolute, a file path | Section 6.4. |
| `RUN [--shell "<program and arguments>"] [--network none|host] <text>` or with a here-document | default shell `/bin/sh -e -s`; default network `none` | Section 6.5. |
| `CMD <JSON array of strings>` | at least one element | Metadata. At most one. |

Every other word, a missing argument, a relative destination, a bad hash, a bad JSON
array, or a second `FROM`, `CMD` or `KERNEL` is `InvalidSpec { field: "sandboxfile",
reason }`, and `reason` starts with `line <n>: `.

```rust
pub struct Sandboxfile { pub steps: Vec<Step> }
pub struct Step { pub line: u32, pub kind: StepKind }
pub enum StepKind {
    From(BaseRef), Kernel(String), Env { key: String, value: String }, Workdir(String), User(String),
    Add { source: String, destination: String, mode: Option<u32> },
    Fetch { url: String, sha256: String, destination: String },
    Run { shell: Vec<String>, network: NetworkMode, script: String },
    Cmd(Vec<String>),
}
impl Sandboxfile {
    pub fn parse(text: &str) -> SandboxResult<Self>;
    pub fn print(&self) -> String;          // the canonical form
}
```

### 6.2 Step keys and the cache

`H` is SHA-256; a key is the first 32 hex characters.

| Step | Key |
|---|---|
| `FROM` | `H("ewe-build-v1\n", base image key, "\n", arch, "\n", size_mib)`. The base is first made into a sandbox image by the importer of feature 13 (`alpine`, `docker`, `tar`), resolved in the store (`image`), or pulled (feature 15, `oci`); its key is the base image key. |
| `ENV`, `WORKDIR`, `USER`, `CMD`, `KERNEL` | `H(previous key, canonical line)`. No snapshot: the step points at the snapshot of the step before. |
| `ADD` | `H(previous key, canonical line, tree hash)`. The tree hash is `H` over the sorted list of entries, each as: relative path, type, mode (the rule of section 6.3), size, and `H(content)` or the link target. Modification times and owners are not in it. |
| `FETCH` | `H(previous key, canonical line)`. The line has the content hash. |
| `RUN` | `H(previous key, canonical line with the script)`. |

Cache directory `<cache_dir>/images/steps/`: `<key>.ext4` (the snapshot) and `<key>.json`
(the metadata after the step, and the snapshot key for a metadata-only step). A step is a
cache hit when its `.json` exists and the snapshot it names exists. Store: write to
`<key>.tmp-<random>`, rename. A reader updates the access time file `<key>.used`; when
the total size passes `cache_max_bytes`, the builder removes snapshots by oldest use, never
one that the running build named.

### 6.3 `ADD`

- `source` is joined to the build context. The builder resolves it and every entry below
  it; a path that is outside the context after resolving `..` and symbolic links fails
  with `InvalidSpec` and the line. A symbolic link inside the tree is added as a link, not
  followed; its target is not checked (it is a guest path).
- A file source: the destination is the file path; if `destination` ends with `/`, the
  file goes into that directory with its own name.
- A directory source: its content goes into `destination` (which is made).
- Owner 0:0. Mode: `--mode` when given (files only); else the host mode's permission bits
  (`& 0o777`) for files, `0755` for directories. Modification time: 0.
- The builder writes a tar stream of the entries (parents first, sorted) and calls
  `sandbox.unpack("/", stream, ..)` in the step's build sandbox. Missing parent
  directories of the destination are tar directory entries with mode `0755`.
- Limits: `BuildLimits.max_context_bytes` (default 2 GiB) and `max_context_entries`
  (default 100 000) for one `ADD`.

### 6.4 `FETCH`

- The builder downloads with feature 03's `fetch` (streamed to a temporary file, hashed,
  size limit `ArtifactLimits.max_download_bytes`, status first). The hash must equal the
  step's; if not, `ArtifactHashMismatch`, and the reason has the line.
- A redirect is followed at most 5 times, never from `https` to `http`.
- The verified file is cached under `<cache_dir>/fetch/<sha256>`; a second build reads it
  from there.
- The file enters the image as a one-entry tar (plus parent directories) through
  `unpack`, owner 0:0, mode `0644`.
- The build sandbox needs no network for `FETCH`: the host downloads.

### 6.5 `RUN`

- The build sandbox runs `exec` with `argv = shell`, `stdin: Piped`, `cwd` = the current
  `WORKDIR` metadata (or `/`), `env` = the `ENV` metadata so far, `user: None` (root).
- The builder writes the script text to standard input and closes it. So the interpreter
  must read its program from standard input: `/bin/sh -e -s` and `python3 -` do. The
  README says this for `--shell`.
- Output events go to the progress stream as they arrive.
- Exit code 0: the step passed. Every other exit, a signal, or `step_timeout` (default 30
  minutes): `BuildStepFailed { line, exit, signal }`.
- `--network host` sets the build sandbox's `NetworkMode::Host` for this step only.

### 6.6 The executor

```rust
pub struct BuildOptions {
    pub tag: Option<String>,
    pub size_mib: u32,                  // default 4096
    pub vcpus: u8,                      // default 2
    pub memory_mib: u32,                // default 2048
    pub no_cache: bool,                 // default false
    pub limits: BuildLimits,
}
pub enum BuildProgress {
    StepStarted { index: u32, total: u32, line: u32, text: String },
    CacheHit { index: u32 },
    Output { index: u32, stderr: bool, data: Vec<u8> },
    StepDone { index: u32, millis: u64 },
    Committing,
}
impl ImageBuilder {
    pub fn new(ctx: Ctx, host: SandboxHost, store: ImageStore, importer: ImageImporter,
               control: Arc<dyn SandboxControl>) -> Self;
    pub async fn build(&self, file: &Sandboxfile, context: &Path, options: BuildOptions) -> SandboxResult<ImageRef>;
    pub fn build_with_progress(&self, file: Sandboxfile, context: PathBuf, options: BuildOptions) -> BuildStream;
}
```

Before any work: `control.probe()`. The build needs a usable driver with `Writable` in
`root_modes`; if a step has `--network host`, with `network_host` too. The builder picks
the first driver in the control plane's order that has what the file needs and sets
`spec.driver` to it for every build sandbox. None: `NoDriver`, or `UnsupportedNetwork`
with the line of the first network step. (In this spec, a guest network on a microVM
exists only on libkrun; so a file with a network step builds on libkrun with block
support. The README states this limit, and requirements section 7.3 lists guest networks
for Firecracker as later work.)

For each step, in order (a `#[task]` state machine; one state for each row):

| Step kind | Cache hit | Cache miss |
|---|---|---|
| `FROM` | Use the base image's `rootfs.ext4` as the first snapshot (no copy; it is immutable). | The importer or the store makes the base image. |
| metadata | Read `.json`. | Update the metadata; write `.json`. |
| `ADD`, `FETCH`, `RUN` | Read `.json`; the snapshot is `<key>.ext4`. | `store.snapshot(previous snapshot)` to a work file; create a `Writable` sandbox on it (`vcpus`, `memory_mib`, the step's network mode, `boot_timeout`); apply the step; `stop()`; `killed: true` is a failure (`Io { op: "unmount image" }`); move the work file to `<key>.ext4`; write `.json`. |

After the last step: the image key is `H(last step key, canonical JSON of the final
metadata)`; `store.commit` with a `snapshot` copy of the last snapshot (the cache keeps its
own file), the manifest from the metadata, `steps` = the canonical lines, `source` =
`sandboxfile:<image key>`.

`no_cache: true` treats every step after `FROM` as a miss and still stores the results.

Each changing step boots one sandbox. A microVM boots in well under a second, and a clean
stop gives a consistent file to copy with no freeze logic.

Fault table:

| Step that fails | What the builder does | What the caller sees | What is left behind |
|---|---|---|---|
| Parse, probe, driver choice | Nothing started. | `InvalidSpec`, `NoDriver`, `UnsupportedNetwork` | nothing new |
| The base | Nothing more. | the importer's error | nothing new |
| `ADD` path check or limits | No sandbox for that step. | `InvalidSpec` or `ArtifactTooLarge`, with the line | the snapshots of earlier steps |
| `FETCH` download or hash | No sandbox for that step. | the fetch error, with the line | the same |
| The build sandbox does not start | Removes the work file. | the start error | the same |
| The step fails in the guest | `kill()` the sandbox; removes the work file. | `BuildStepFailed`, or the unpack error | the same |
| The build is dropped or the process dies | The sandbox dies with the control plane; the work file stays in `images/work/` until `ImageStore` removes old work files. | (gone) | the snapshots of the steps that finished |

### 6.7 Command line

```
agent-sandbox image build [-f <file>] [--tag T] [--size-mib N] [--no-cache] [<context dir>]
```

Default file: `<context>/Sandboxfile`; default context: the current directory. Progress
goes to standard error: `[3/7] RUN apk add ...`, `[3/7] cached`, and the step's output with
the prefix `[3/7] `. The exit code is 0, or 125 with the error.

## 7. Tests

| Law or scenario | Test | Where | Oracle |
|---|---|---|---|
| I1 | `parse_never_panics` (property: any byte string that is valid UTF-8) | `tests/sandboxfile_*.rs` | bound |
| I2 | `parse_print_roundtrip` (property: generated step lists; `parse(print(f)) == f`) | the same | identity |
| grammar, "A syntax error names its line" | `parse_table` (one good and one bad case for each row of section 6.1; continuation lines; here-documents; comments) | the same | example |
| I3 | `key_depends_only_on_content` (property: change time, host path prefix and modification times, the key stays; change each input, the key changes) | `tests/build_keys_*.rs` | metamorphic |
| "A touched file is not a change", "A change in an added file ..." | `tree_hash_cases` | the same | metamorphic |
| I8, "ADD cannot leave the context" | `add_path_rules` (`..`, absolute, a link that points out, a link that points in) | `tests/build_add_*.rs` (real temporary directories) | expected outcome |
| I10, section 6.3 | `add_tar_entries` (owners, modes, parents, order) | the same | example |
| I4, I5, the three cache scenarios | `build_sim_cache` (simulated control plane with a `ScriptedSlave`; counts sandboxes for each step) | `tests/build_sim_*.rs` | metamorphic |
| I6, "A failed RUN stops the build" | `build_sim_run_fails` | the same | expected outcome |
| I7, "A changed remote file is refused" | `build_fetch_hash` (local HTTP server) | the same | expected outcome |
| "A network step on a host without ..." | `build_network_needs_driver` | the same | expected outcome |
| the fault table | `build_fault_matrix` | the same | fault |
| I11 | `build_concurrent_same_key` (two builders in two tasks, one cache directory) | the same | property over interleavings |
| cache eviction | `cache_evicts_oldest`, `cache_keeps_running_build` | the same | bound |
| I12 | `run_output_streams` (a step that prints 100 MiB; the builder's buffer stays under 1 MiB) | the same | bound |
| "A build file becomes an image", "Added files belong to root", I9, "A stopped build ..." | `e2e_build_app`, `e2e_build_owners`, `e2e_build_network`, `e2e_build_resume` | `tests/e2e_images.rs` (Firecracker; the network test on libkrun with block support; each returns with the probe report when not usable) | example |
| CLI | `cli_image_build_parse`, `cli_image_build_flow` | CLI test files | example |

## 8. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_platform_sandbox --features "images testing slave" --all-targets --profile uat --no-deps -- -D warnings
cargo test -p foundation_platform_sandbox --features "images testing slave" --profile uat sandboxfile_ build_
cargo test -p foundation_platform_sandbox --features "images testing slave" --profile uat e2e_images -- --nocapture
```

## 9. Done when

- Each scenario of section 5 and each row of section 7 passes.
- The build file of section 1 (with a local HTTP server for `FETCH`) builds and runs.
- A second build of an unchanged file starts no sandbox.
- The README has the grammar table, the `--shell` rule and the network limit.
