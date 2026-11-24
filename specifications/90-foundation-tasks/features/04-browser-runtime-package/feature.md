# Feature 4. Publish the browser runtime as a JavaScript package

The owner requests this feature on 2026-11-24. The design uses one source tree for module imports and classic scripts.

## Glossary

| Term | Meaning |
|---|---|
| User interface (UI) | The controls and views that an application displays. |
| ECMAScript module | A JavaScript file that declares imports and exports. |
| Classic script | JavaScript that a browser runs through a regular script element. |
| npm | The public JavaScript package registry and its command-line tool. |
| Tarball | The compressed package archive that npm installs. |
| Integrity digest | A checksum that identifies the exact bytes of a tarball. |

## Consumer contract

A developer installs the proposed package `@ewe-studios/foundation-wasm-ui` and imports its public entry from an application bundler.
Confirm control of the npm scope before the first publication.
Use `.js` for modules and declare `"type": "module"` in the package manifest.
The explicit manifest gives Node.js the module format and preserves existing source filenames. [Node.js package formats](https://nodejs.org/api/packages.html#packagejson-and-file-extensions)

```js
import { WasmLoader, createApp, registerFrameApp } from "@ewe-studios/foundation-wasm-ui";
```

A developer can also copy one classic script from the installed package and load it directly in a browser.

```html
<script src="./bundle.js"></script>
```

| Published file | Contract |
|---|---|
| `index.js` | Root module with documented named exports. All imports resolve inside the installed package. |
| `bundle.js` | Readable, self-contained classic script for development. |
| `bundle.min.js` | Minified classic script with the same behavior for deployment. |
| `package.json`, readme, license | Package identity, public entries, instructions, and distribution terms. |

Expose both script filenames through package subpath exports.
Each classic file installs the existing runtime globals, including `FoundationUiBootstrap` and `FoundationWasmRuntime`.
A consumer supplies its application WebAssembly files and generated application bindings.
Feature 1 also supports building the final application bundle with those inputs.

## Build and release

```mermaid
flowchart LR
    Source[Runtime sources] --> Build[Package build task]
    Build --> Modules[Public module entry and dependencies]
    Build --> Scripts[Readable and minified classic scripts]
    Modules --> Pack[npm pack]
    Scripts --> Pack
    Pack --> Test[External consumer checks]
    Test --> Publish[Publish the tested archive]
```

The release job publishes the archive that passes the consumer checks.

1. Put the package manifest and public entry under `backends/foundation_wasm_ui`. Use its runtime modules as the source.
2. Include the required `foundation_wasm` runtime from its existing source during packaging. Preserve relative imports inside the archive.
3. Keep initialization order, browser registrations, license notices, and existing public runtime names.
4. Reuse the Feature 1 Rust bundler for both classic files. Export the package task through the existing platform task runner.
5. Preserve existing Cargo consumers and their release artifact paths. Keep the readable and compact choices explicit in the new package.
6. Derive the package version from the tagged `foundation_wasm_ui` crate. Validate the manifest version before packing.
7. Publish one public npm version for each new `foundation_wasm_ui-v<version>` release, including releases with unchanged JavaScript.
8. Invoke publication from the successful crate release workflow. Repository-token tag pushes do not start a second workflow automatically.
9. Allow an explicit retry for a tagged revision. Build from that revision and serialize publication for that package version.
10. Use npm trusted publishing from the release job. Configure the registry publisher before enabling automatic publication. [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/)
11. Compare the local archive integrity with the registry on retry. Accept a matching version and fail on different bytes.
12. Record the tag, commit, npm version, and archive integrity. Keep release credentials out of builds and test jobs.

## Acceptance checks

- From two clean checkouts of one tag, build and pack the package. Compare archive digests and inspect the npm file list.
- Install the archive outside the repository. Bundle a consumer that imports the three public functions and boots a browser view.
- Load each classic script separately in a clean browser page. Verify startup, action dispatch, and a rendered update.
- Check browser requests. Each classic runtime loads from one JavaScript file. Application WebAssembly requests may follow.
- Compare observable behavior across the module entry and both classic scripts. Verify that the minified file is smaller.
- Run the existing Cargo runtime and application bundle checks after the source move.
- Exercise a release retry against a registry fixture. Cover a missing version, a matching archive, and a conflicting archive.
- Verify publication from a new crate release and installation of its exact public version before closing this feature.
