# Specification 90 learnings

## 2026-11-22

Cargo builds the current command as `ewe_platform`. Its help text uses `platform`.
The release workflow packages each executable target that Cargo reports.
The Makefile also names the current executable directly.
These callers require the compatibility entry. Both executable names can share one source.
[Manifest](../../bin/platform/Cargo.toml) · [Release workflow](../../.github/workflows/release.yaml) · [Makefile](../../Makefile)

The WebAssembly build and bundle commands already delegate to package libraries.
Each library can expose its existing operations through a Rust task set.
[Build adapter](../../bin/platform/src/wasm_bins/mod.rs) · [Bundle adapter](../../bin/platform/src/wasm_bundle/mod.rs)

The deployment package also supplies a standalone command.
Its separate entry demonstrates why a package needs control over runtime setup.
[Deployment entry](../../backends/foundation_deployment_platform/src/bin/testbed.rs)

The command table omitted `gen_api normalize`. The current parser registers it, so both invocation forms retain it.
The old platform handlers already parse each command's arguments. The adapter reuses the same command definition and calls the same leaf.
The build helper records directory dependencies because file dependencies alone cannot detect added files.

An upload can reach Maven Central before the client receives its deployment identifier.
A durable attempt record prevents a retry from starting a second upload.
A matching receipt allows the task to continue.
The fake registry reproduces both cases.

Unix assigns each child to a new process group at launch.
Windows starts the child suspended, assigns its job, and resumes the primary thread.
Both mechanisms let the process backend stop descendants as part of cancellation.

A local attempt record does not survive a fresh continuous integration (CI) runner.
The draft release stores a matching attempt asset before the Central upload.
The fake registry test deletes the local record to verify that the next runner still stops.

Valtron can provide one background worker. It cannot block that worker on one pipe while a second pipe fills.
The replacement uses Valtron to schedule child startup.
The child supervisor owns independent operating system workers for the blocking pipes and the child wait.
The native signal task cancels the command through Valtron.
Review wakeups, sink failure, and completion ordering before integration.

The supervisor sends one exit status. A pending output write can still fail after the child exits.
Read the exit status after all pipe operations complete. This keeps the status available for cleanup after a pipe failure.
An unused input pipe still needs a completion signal. A closed channel alone cannot distinguish success from a worker failure.
The standard process identifier is a number on both Unix and Windows.

A slow output consumer exposes the double read. The regression test fails on the checkpoint after the macro import repair.
Use separate Cargo target directories when comparing worktrees. The build output path for an integration test can match across both worktrees.

Dropping the process future can abandon a silent child. The startup result must also own the right to terminate that child.
The supervisor keeps the Unix group leader until the owner releases the right to send a signal.
The Windows job handle keeps the tree identity after the leader exits.
Darwin rejects a group signal when the group contains only its exited leader.
The runner checks the leader state and group membership before it accepts this case.

The workspace error derive can infer a source from an unnamed field. Explicit source attributes preserve the runner's original error chains.

The native compiler finds the mise compiler cache shim during the platform build.
Select the installed compiler cache with `MISE_SCCACHE_VERSION=0.18.0` when the shim lacks a default.
An external Cargo target moves compile-fail fixtures outside the repository.
Give the temporary package its own migration files so path resolution follows the production contract.

### Check package requirements outside the workspace

Cargo can resolve a newer local crate even when the manifest still permits an older release.
Compare each local dependency requirement with the local package version.
Root workspace metadata omits excluded packages. Read their manifests separately.

## Build and deployment task ownership

Task discovery confirms command availability. It cannot prove migration completeness. Trace each registered handler through its child processes and workflow callers.

The existing single-file wrapper embeds WebAssembly bytes. The browser runtime build resolves imports across JavaScript files. These operations serve different callers.

The platform demo uses a unit-returning build entry. The Todo build entry returns a result and conditionally runs a generator. Installation must preserve both forms.

### Browser runtime output

Removing whitespace alone produced 376,336 bytes. Scope-aware compression and local name shortening restore the previous distribution size.
Keep legal comments from every input and add them to both output scripts.
The parity fixture must reset the current view before replaying its frame. The frame applicator now keeps a cursor map for each view.

The browser parity test compares a 312-byte frame. It passes with the readable script and with the compact script.
Cargo metadata describes individual crates. Build groups need explicit source directories, build recipes, and package references in the continuous integration policy.

### 2026-11-22. Native release verification

Save the publication receipt before a retry can start a second upload.
The offline publication tests distinguish a missing receipt, a recoverable receipt, and an already published release.
Install the clock and timer in the shared host entry point. A dispatched task then receives them through its context.

The legacy platform separates logical resource names from the root supplied by the host.
Mobile builds also need a packaging step. Staging files alone does not place them in the installed application.

The classic entry imports the frame runtime. Todo loads its action module separately. An action-only edit preserves both classic bundle files.

A relocated CMake build tree can retain absolute paths. Regenerate its build directory before testing the platform executable.
The platform build also checks the packaged release record against the canonical one.

A browser response can fail after a successful open. Each streamed resource chunk therefore carries its own result.
Track each source directory as well as its files. Cargo can then detect new directory entries.

Browser stream readers stay on their JavaScript thread.
Native readers need movable streams so application workers can await the same resource operation.

The embedding derive accepts an environment variable followed by a slash and a path.
A build script can supply its staging directory before macro expansion.

Android generated asset registration orders the producer once per variant.
Xcode needs a folder reference and a producer output that refer to the same derived directory.
Select the Arm simulator when the local native framework contains only Arm simulator code.

The existing native core retains the application context. Both native hosts can install a reader after core creation.
A dropped stream must close its native file even when the caller stops before the final chunk.

Generate bindings from the current shared library before building either native host.
The bridge preserves distinct errors for missing files and I/O failures.

A file ownership list cannot identify empty managed directories. Record managed directory names with a trailing slash.
Keep user files during cleanup, even when their parent was a managed directory.

A Cargo test executable can run in a private unit directory. Its parent does not identify the public resource directory.
An embedded server host must supply that directory. A standalone server can resolve its executable path.

Kotlin already uses close for generated handle disposal. Swift treats open as a keyword in callback declarations.
Use openResourceFile and closeFile for the package callbacks.

Use a companion browser module for the resource service. Its JavaScript bindings own the browser fetch context.
The Todo action module keeps its existing host import interface. Both native hosts use the same Rust open operation.
The iPhone test must use the installed application bundle. The test runner has a different bundle.

Cargo compares tracked output timestamps with the start of the previous build.
Writing an equal index on every run keeps the package dirty. Preserve unchanged files and index timestamps.
Reject output directories that contain inputs or lie inside a declared input directory.

A browser resource can fail before or after open. Test both the response status and a stream that fails after its first chunk.
A test for early browser disposal must check that the underlying response body receives cancellation.

The release manifest names a local candidate. There is no public native-ui-v0.1.1 tag at this checkpoint.
Generated native packages use the same release setting as their existing dependencies.
Local verification uses private candidate tags and the prepared native libraries.

A hidden staging index travels with an Xcode folder reference. Put the public payload in a child folder.
A local Swift package mirror can verify an unpublished candidate without publishing a tag or replacing a public package fingerprint.

The Apple resource build selects a payload subdirectory before it copies staged files.
The staging index belongs to the parent directory. The application bundle then contains only declared resources.
The optional server dependency needs a Windows C compiler when Cargo cross-compiles it from macOS.

## 2026-11-24. Windows resource verification

The resource suite checks filesystem behavior by running on the target system.
[Specification 91](../91-windows-resource-staging/requirements.md) records the Windows prerequisites and platform-specific test count.

A package manifest can declare JavaScript modules with a .js extension. The current crate release workflow must explicitly start downstream publication.

## 2026-11-24 master integration

ZIP 9 returns a result when it decodes an archive member name. The verifier propagates that error before it checks the path.
The wire bundle and manifest fixtures each affect two test selections. Each change also selects the package that owns the changed file.

The Todo checkpoint changes two package versions. Standalone example and test workspaces keep their own copies of these versions.
Each lockfile needs a separate dependency check after the merge.

## Feature 2. Cargo build hook installation

Cargo can inherit a renamed build dependency from the workspace.
The installed Rust call must use that local dependency name.
A syntax span locates the opening brace after Unicode comments.
Function attributes inside the brace must precede installed calls.
Write the manifest before the script so a retry can finish a partial installation.

The runtime entry imports a file from the core WebAssembly crate.
Cargo registry directories include version numbers, so the provider prepares that import from the core crate's embedded source.

The standalone script bundler leaves dynamic imports outside its input graph.
Application bundling must preserve lazy evaluation and retain one namespace object for each module.
The application loader caches a module before it evaluates that module's dependencies.

A Cargo build script can compile browser targets when each application uses a separate child target directory.
The browser target skips server packaging. Each browser library selects its own feature and binding format.
Wasm bindings resolve their binary references against the original module location.
The application bundle keeps each module location and resolves it against the public script address.

The resource provider can select its lookup table in the consuming crate.
A debug build dependency can then produce an embedded application without changing the server's registration.

A resource wildcard can share a prefix with an application route.
The router must keep the nearest wildcard candidate while it tries more specific segments.
A browser test that needs a private helper can load its own test script.
Waiting for the native dialog to open makes the following input action reliable.

Manifest fixture changes can affect the contract group and the crate that owns the changed file.
The planner selects the contract group by its declared recipe, so the policy can rename the group.

Docker writes the container identifier before the guest starts.
The task can remove the container after the guest fails or the caller cancels.
The attached Docker start command reports the guest exit status.

Java and Kotlin report versions on standard error. The tool checks both process streams.
A test can substitute the host tools on the executable search path.
Build the check executable before changing this path.

The wire generator must preserve optional field presence, unknown union verification, and native builder compatibility.
A byte comparison against the checked-in files tests these details across all four output languages.

Cargo treated the Wasm crate's missing wildcard path as a changed input on every build.
The service generator also watched a descriptor that it rewrote on each run.
The browser compiler dependency file includes generated Rust and its original protocol inputs.
Track the source inputs and omit files inside the private child target directory.

An archive must build from the target directory Cargo selected, including when its path has spaces.
A development dependency with a path and version can introduce an otherwise hidden publication cycle.
Android license acceptance needs input until the tool closes its pipe. Preserve the child status after closure.

Image plans preserve build argument order for callers and sort arguments for the hash.
The previous and current planners produce identical hashes when the Cargo lockfile bytes match.
File equality for staged output must compare link targets and Unix permissions.
Changing only an executable bit changes the image hash and must update the staged file.
A fresh guest has neither Cargo nor the workspace source. Guest migration requires an independent bootstrap.

Cargo reports the actual executable path for each selected target. This works with a custom target directory and a renamed binary.
A release archive needs fixed metadata and sorted entries to produce the same bytes on repeat builds.
The vendor Android manifest can list the same archive for a version and its current alias.
Accept identical records. Reject records that use one filename with different sizes or checksums.
The Python package selection placed shared planner source under a global directory. The Rust package needs an explicit equivalent rule.
Issue #790 tracks this change and its regression test.

The default Docker driver on this host cannot export a Docker archive. A temporary container-based Buildx builder supports both requested export formats.
For a single-platform image, Buildx can create an index when it promotes a tag.
The promotion task disables index conversion to preserve the tested digest. Issue #791 records the previous behavior.
The native Swift test needs the same task host as the installed command. Its child process provides an independent background job registry.

The previous manual publication loop selected retired packages after a valid package.
Validate the entire selection with offline Cargo metadata before publication. Let Cargo order packages and select the configured registry.
The manual WebAssembly fixture paths and runtime bundle have moved or disappeared. Issue #792 records the unresolved caller contract.
Git can sign fixture tags using the host configuration. Disposable test repositories must select unsigned fixture tags explicitly.

The Google Cloud directory contains 303 files named openapi.json. None declares an OpenAPI version.
302 files declare a Google Discovery version. The canonical generator requires servers and component schemas.
A disposable source tree reproduces the failure with two tracked specifications. It also verifies that both batch callers preserve an existing file.

## Application generation command

AnswerMe uses foundation_repl for terminal interaction. Existing mobile generators each create their own destination. Combined generation needs a shared staging plan.

The Android Todo identifiers come from its Gradle application modules. The current native device tasks repeat those identifiers and invoke repository test scripts.

The existing template generator uses EmbedDirectoryAs with always_embed. Task providers can use the same pattern for script directories in both build profiles.

An external tool requirement can be independent of a script requirement. Rust can invoke the platform tool directly and check its result.
## Feature 4 publication credentials

The publication process can read a literal token assignment without running shell commands from the user environment file.

The npm client reads package metadata again during an authenticated publication.
The public integrity check uses a separate command without publication credentials.
A browser can finish loading an external classic script before the document is ready.
The external consumer waits for browser initialization before sending its first action.

Node.js 24 and 26 produce identical npm tar streams but different compressed bytes.
The pack task recompresses the stream with fixed gzip metadata and the locked Rust compressor.

## Feature 5 external application checks

The generated web build needs two package sources. Cargo resolves Rust dependencies. npm verifies the runtime archive against its lock file.
An isolated registry can serve the release archive with its original integrity value.
A real browser test checks that the JavaScript bindings connect the generated module to its Rust counter.

## Project-local task execution

A project can declare its task interface without running Cargo during help or task discovery.
The shared task registry rejects namespace conflicts before it invokes the local runner.

## Native release inputs and device checks

The available Android 0.1.0 package lacks the resource interfaces used by the generated application.
The older Apple archive also lacks current binding symbols.
Testing the current application requires building the native libraries and bindings from the same source.
An installed command test confirms Android startup failure and cancellation remove the supervised emulator process.

The installed command tree can validate the full guide without a separate list of commands.
Each command's help supplies the child names, usage line, and available options.

Cargo reports an unused build dependency when the shared application owns the resource hook.
The standalone and composed native templates therefore use different build dependency declarations.

A host tool can prepare an Android build on macOS. A script sent to the device needs a separate platform selection.
Keep companion paths relative to the embedded directory. Keep the selected interpreter and its arguments separate.
The task package must declare the directory as a Cargo build input for incremental builds.

Start the process monitor before transferring child ownership. A failed monitor must leave the original process guard available for cleanup.

Validate target selection before checking for a native library. Server applications can also request a target absent from their manifest.

Java 27 writes 17,006 bytes when verifying the generated Android bundle with a test key. Match the complete success line while streaming all output.

Inspect resources inside the installed Apple bundle and built Android archive. A successful compiler exit alone does not check the application title.

Cargo unifies features across dependencies on the same target. Select the JavaScript event loop in the WebAssembly dependency table.

A combined application needs four public inputs for these checks. Verify the Git tag, Maven package, npm package, and Rust resource crate separately.
The npm archive in the generated lock file must match the candidate checksum. An earlier local archive has different bytes.

Wrangler accepts `d1 migrations apply DATABASE --local`. It rejects the earlier execute command.
Configuration output can report whether a token exists without printing its value.

Cargo NDK checks dependency artifacts when its output directory option is set. A dependency can declare a shared library but produce only a Rust library.
Stage the declared application library from each completed target build. Keep the framework library in its Android library package.

Android can report a successful activity start while another application owns the focused window.
Under load, Android can report a timeout before the requested window appears.
Check the exact activity name until the configured deadline. Treat an application error dialog as a launch failure.

Wrangler lists buckets in the terminal. Its object commands do not list the contents of a bucket.
The storage interface returns a cursor when more objects are available. Preserve each key exactly when writing it.

The Todo Server examples render the item rows and their controls. The generated server starter displays the open item count.
Verify a shared update by adding an item on one device and reading its title on the other device.
