# Feature 37. Simplify Todo startup, packaging, and state

Status: Required implementation work. This document is the acceptance contract for the Todo redesign.

## Terms

A key-value (KV) store reads and writes records by key.
A universally unique identifier (UUID) identifies one Todo throughout its lifetime.
A snapshot contains confirmed records and their matching stream position.
A resource provider opens a packaged asset by its logical name.
Hypertext Transfer Protocol (HTTP) carries requests and responses.
A remote procedure call (RPC) invokes a named operation on another system.
WebSocket keeps a connection open so either endpoint can send messages.
A fixture supplies data or services for a test.

## Scope and precedence

Use this feature when changing the Todo launcher, browser packaging, shared actions, or persistence.
It consolidates Specifications 69 and 90 for the Todo application. Use this contract when older Todo startup instructions conflict.
Feature 26 requires independent native data calls. Its fixture belongs in tests.
Feature 27 supplies the single package. Feature 32 supplies named updates, local rendering, and action routing.
Feature 33 supplies replay recovery. Specification 90 Features 1 and 3 supply bundling and resources.
Existing graphics, audio, and native data contracts still apply. Feature 32 tracks its separate timed block demonstration.

## One live server

In `examples/ui_todo/src/bin/server.rs`, bind the configured address, open the configured services, construct the app once, and serve.
Expose `app_with_store(store, channel)` for the full server. Its callers supply configuration independently.
The persistent store owns its file path and format. Opening the same store after restart makes previous writes available through KV operations.
Use the store's documented empty state for a new path. Propagate malformed contents and read failures. Preserve the original file on failure.
Keep confirmed server records and idempotency receipts durable. A delayed retry after deletion must never recreate the record.
Define a consistent recovery policy for the stream position and replay history across a restart.
A server with a fresh history must direct stale clients to a matching snapshot.
Keep pending changes for each Todo server separate.

Remove these paths from production startup:

- Remove `--data-only` and `data_app()`. Move the seeded service and its launcher under `examples/ui_todo/tests/`.
- Keep the real typed data procedure on the full server. Update the iPhone and Android test runners to launch the independent fixture.
- Remove `--restore` and backup parsing from normal startup. Put legacy import and conversion in maintenance tooling.
- Keep `app_with_todos(items)` as the convenience constructor. It validates identifiers and supplies a fresh channel.
- Remove `app`, `app_with_channel`, and `app_with_public_directory`. Configure the browser binary name in `build.rs`.
- Remove the `EWE_TODO_HISTORY` startup branch. Configure replay capacity on the supplied channel. Replay tests construct their own channel.
- Remove `web_module()`, runtime module validation, and `EWE_TODO_MODULE`.

## Build and serve the assets

The app build script calls its service schema generator and one public framework packaging hook.
The hook uses the resource manifest, builds missing browser modules, generates Connect and resource bindings, and stages assets.
Framework providers own tool invocation, change tracking, output directories, validation, and failure reporting.
Use those same providers for command-line tasks. Retire `examples/ui_todo/build-module.sh` and migrate its callers.
A clean Cargo build must prepare the browser application without a separate script or a prebuilt module.

### Standard paths and project overrides

The build stages assets in `public/` on disk. The server exposes them at `pkg/` relative to the application mount.
For an application mounted at `/todo/`, the browser requests the application script at `/todo/pkg/bundle.js`.
The project can change the relative asset directory. The application route base defaults to its mount.
The project can configure a separate service address.

```mermaid
flowchart LR
    Page["Browser page at /todo/"] --> Assets["Asset requests at /todo/pkg/"]
    Page --> Service["Default service requests at /todo/"]
    Page -. "service override" .-> Other["Configured service address"]
    Assets --> Public["Packaged files in public/"]
```

The browser can use a different service address while it loads assets from the application mount.

The project can override these locations in `build.rs`. The build records the effective locations in the existing generated project configuration.
The browser entry already imports `moduleUrl` from `ewe-app-config.js`. Extend this configuration for the remaining locations.
Generate the page links from the same settings. The browser needs the script location before it can load the configuration.
Resolve package paths relative to the application mount. Keep an explicitly configured service endpoint independent of the asset location.
Test the standard layout and a project override below a subdirectory. Check the script, style, icon, modules, game, resources, and service calls.
Verify that the override requires no matching path edit in Todo.

### Proposed packaging closure sequence

The owner approves steps 1 through 4 for implementation on 2026-11-28.
This brief uses the existing Todo screen and browser controls.
Use the existing [pending](../32-portable-view-targets/screenshots/chrome-pending.png) and
[retry](../32-portable-view-targets/screenshots/chrome-retry.png) captures as visual references.

1. Keep the development server resource tree available during native builds. [Issue 1060](https://github.com/ewe-studios/ewe_platform/issues/1060)
   - Separate package-local resource staging from publication beside the development server executable.
   - Publish the server resource tree only when the packaging hook produces browser outputs. Keep native output and explicit destinations available.
   - Build browser and native settings in both orders in one scratch profile. Verify the active server still serves the same script and module.
   - Test a failed browser build. Verify the last successful public tree remains available.
2. Verify reused connections before relying on browser navigation results. [Issue 784](https://github.com/ewe-studios/ewe_platform/issues/784)
   - Start the real Todo binary with its normal server settings and a scratch store.
   - Use six independent clients. Each reads each response fully and requests up to 100 scripts on the same connection.
   - Record status, response-header wait, route, and request number. Check the read buffer and wake registration only if the stall recurs.
   - Test two attached Chromium tabs separately. Match events to their session and document. Check failed navigation and same-document navigation. [Issue 1143](https://github.com/ewe-studios/ewe_platform/issues/1143)
   - Keep a server-side fix separate from a browser-driver fix. A passing driver test alone does not clear a server header stall.
3. Resolve asset and service addresses from one project configuration. [Issue 839](https://github.com/ewe-studios/ewe_platform/issues/839)
   - Extend the generated browser configuration from the standard path settings above. Generate page links from the same values.
   - Resolve relative addresses from the public application mount. Keep the exposed prefix in the browser even when the proxy strips it upstream.
   - Route initial, action, live, Connect, and socket requests through the service base.
   - Resolve named packaged resources from the same asset base as the script, style, icon, and modules.
   - Use the canonical Todo service address for pending state.
   - Preserve the existing root namespace. Import origin-wide legacy records only into that namespace.
   - Use the existing one-script output for this brief. The two-script output amendment needs a separate approval.
4. Check the package at the origin root and below a prefix-only proxy.
   - Use a clean target directory. Check default paths and a project override with no matching edit in Todo JavaScript.
   - Record the page, stylesheet, icon, one script, application module, and lazy game module requests and media types.
   - Exercise local dialog input, remote actions, a live peer update, a named resource, and the game return.
   - Test the default, socket, and Connect transports. Include an explicitly configured service endpoint.
   - Move the development executable and its public tree. Run a standalone release outside the checkout.
   - Check missing generated inputs and confirm that each failure preserves the last successful public tree.

Use the existing server entry point for these checks. The proposed shared Cargo run tasks are separate.
[Specification 90, Feature 5](../../../90-foundation-tasks/features/05-ewe-ui-application-command/feature.md#proposed-shared-application-run-tasks)

### Proposed browser output amendment

Produce readable `bundle.js` and minified `bundle.min.js` in development and release builds.
Use the same project sources and generated bindings for both scripts. Keep intermediate scripts outside the public tree.
The development page selects the readable file. The deployment page selects the minified file.
Each page loads one script. Keep the application and companion WebAssembly binaries at explicit relative paths.
Reserve both script names. Reject manifest entries, generated inputs, and configured binary paths that conflict with either file.
Reject a path that requires either file to act as a directory. Preserve the last successful resource tree on failure.

Test both script variants through the mounted resource provider from a website subdirectory.
Open a fresh page for each variant. Record the script and binary requests.
Complete a real Connect exchange, stream a named resource, open Mathalley, and return to Todo.
Verify one script request per page and one game binary request after Play.
Mount application routes first. Mount the resource provider once:

```rust
router.mount(StaticFileHandler::from_resources(PackagedResources))?;
```

Remove the per-file route table and registration adapter in `src/server/assets.rs`.
The provider serves development files beside the executable and embeds release bytes.
The app passes no filename index, module path, mode flag, or byte lookup callback.

## Client and server actions

```mermaid
sequenceDiagram
    participant H as Host action
    participant K as Pending store
    participant S as Live server
    H->>K: Save pending change
    H->>H: Render local preview
    H->>S: Submit typed server action
    S->>S: Commit change and receipt
    S-->>H: Deliver typed result or live update
    H->>K: Clear confirmed intent
    H->>H: Update named elements
```

The submitting host renders pending work before waiting for the server.

- Use one UUID from pending submission through deletion. The framework UUID helper accepts injected entropy.
- Keep confirmed client records in memory. Persist unresolved Add and Delete operations as separate KV entries.
- On open, fetch the server snapshot and list pending keys. Keep unresolved requests available for retry.
- Bind submission to one host action. It saves intent, renders the preview, then calls the registered server action through its context.
- Return known transport failures through Rust `Result`. Socket acceptance leaves the intent pending until confirmation.
- Send typed values and receive typed deliveries. Use `html!` for view content. The framework owns wire framing and payload decoding.
- Remove `shared/outbox.rs`, `outbox::prepare`, request interception, custom persistence, and application frame parsing.
- Remove the mobile transport wrapper and browser send overrides. Keep platform transport adapters supplied by the framework.
- Share one registration list and page declaration. Use receiving views and named elements. Keep dialog, draft, search, and filter state on the host.
- Keep the confirmed count consistent with the host cache. The independent count screen retains its server-rendered demonstration.

## Browser startup and storage

The browser runtime provides the public loader, application builder, and boot operation.
The existing Cloudflare storage providers require Worker services. Browser applications need providers for storage on the user's device.
Implement optional IndexedDB and `localStorage` providers in `foundation_db` through its existing `KeyValueStore` trait.
Keep browser bindings, serialization, and typed storage errors in those providers.
Verify a standalone browser consumer can use either provider without importing `foundation_wasm_ui` or supplying Cloudflare bindings.
During boot, `foundation_wasm_ui` installs the selected provider and namespace in the host context.
Todo uses IndexedDB for durable pending changes. Each operation has its own key in the namespace for its Todo server.
Use the memory provider for temporary sessions and tests. Offer `localStorage` for simple settings.
Todo actions read and write typed records through the installed service. Remove Todo-specific JavaScript storage callbacks.

Test both browser providers through `get`, `set`, `delete`, and prefix-based `list_keys`.
Reload the page and verify stored records. Check that a new memory provider starts empty.
Open two tabs in the same namespace. Write distinct pending changes concurrently and verify both survive reload.
Confirm one change in the first tab. Verify the other change remains pending in the second tab.
Check namespace isolation and propagate denied access, quota errors, and failed IndexedDB transactions to the caller.

The build script sets the public WebAssembly path and generates the browser configuration.
The browser entry imports that configuration when it starts the application.

```js
import { moduleUrl } from "./ewe-app-config.js";

const app = createApp({ moduleLoader, moduleUrl, mount, initialRoute, storage });
await app.boot();
```

1. The loader checks response status before compiling. Return a typed load error for missing, malformed, or incompatible modules.
2. Boot supplies imports, attaches memory, installs the Rust host context and selected services, and registers the application's declared actions.
3. The application supplies its mount, initial route, typed delivery action, and optional capabilities through configuration.
4. Use the default transport or register a custom transport before boot. Applications supply endpoint choices through configuration.
5. The runtime owns the handshake, frames, socket acknowledgements, request correlation, disconnect handling, initial load, and recovery.
6. Accept serializable JavaScript values and typed Rust action inputs. Keep transport errors available through the public action call.
7. Dispose partial startup on failure. Repeated startup must not register duplicate handlers or leave open connections.

Remove Todo-specific storage imports, pointer callbacks, numeric scope translation, and prepared-request callbacks from `src/web.rs`.
Remove manual module instantiation, frame inspection, send overrides, and socket callback replacement from `web/app.js`.
The framework can use native browser bindings internally. Application authors use its public interfaces.

Test a missing module, a successful response containing an error page, and an invalid WebAssembly binary.
Verify that no host starts after a load failure. Preserve the request address and the available status or compiler cause.
Boot a minimal external app with the default transport. Repeat with a registered custom transport.
Verify typed local and remote actions, retry, reconnect, and view recovery without editing the transport instance.
Verify the deployed Todo entry still requests one JavaScript file.

### Existing browser records

Earlier Todo versions store pending changes in the localStorage key `ewe.todo.pending.v1`.
Each entry contains the operation identifier and the change.
The browser origin identifies the scheme, host, and port of the page that writes the data.

Import these records into the Todo server namespace for that origin. Keep their operation identifiers.

1. Read the old map during startup. Validate every entry before writing any records.
2. Check an import receipt for each operation identifier inside one IndexedDB transaction.
3. For an identifier without a receipt, keep its existing destination record or insert the old record.
4. Save the receipts in the same transaction. Preserve the source map because another tab can still write to it.
5. If the transaction fails, report the error. Retry the import on the next open.
6. Test simultaneous imports, an interrupted import, invalid data, a late write from an old tab, and a reload after confirmation.

A receipt records an imported identifier. It prevents a retry from restoring an operation that the server already confirms.
The existing Feature 32 pending and retry screenshots apply to this storage change.

## Native transport selection

Use one screen entry point and one Todo core factory on each platform.
Use `ServerScreen` on iPhone and `RemoteEweScreen` on Android.
On Android, replace the separate `rpcBase` and `socketEndpoint` options with one transport selection.
The app supplies the server address and an action transport setting at startup.
Support the default HTTP action endpoint, Connect RPC over HTTP, and the WebSocket action endpoint.
The framework selects the sender and negotiates its capabilities. It passes the sender to the same factory for every selection.
The factory installs the pending store, resources, and typed delivery action once per core.
Keep the initial view request on HTTP for all three selections.
Route host actions locally. Route typed server actions through the configured sender.

Expose the same independent WebSocket service to Rust callers through the iPhone and Android host contexts.
The caller supplies the endpoint, opens a connection, sends and receives application messages, and closes the connection.
The native framework owns the platform socket. Each caller owns the lifetime of its connection.
Return connection and message errors to the caller. Keep independent messages outside the action dispatcher.

Test all three transport settings on both iPhone and Android. Use the same core factory for every setting on each platform.
Verify the initial HTTP load, local dialog, remote Add, and typed confirmation for each setting.
On each platform, open a separate native WebSocket from Rust while Todo uses its action socket.
Exchange an application message and close the independent socket. Verify Todo can still send an action and receive confirmation.

## Required verification

1. Start the full server against a new store. Add a record, stop the process, and reopen the same path.
2. Verify the record through a key lookup, a list operation, and the initial page. Use no restore command.
3. Delete the record, restart, and retry its original Add. Verify it stays deleted.
4. Exercise corrupt storage, interrupted writes, and independent pending writes from two views. Verify existing records survive failure.
5. Run replay recovery with an injected bounded channel. Check the snapshot position during a concurrent mutation.
6. Run the seeded data service from test support. Verify typed success and missing-record errors from Swift and Kotlin.
7. Build from an empty target directory. Run the moved development tree and the standalone release executable outside the checkout.
8. Verify one script request, module media types, local actions, Connect calls, and native resource reads. Reject missing generated inputs during build.
9. Verify immediate pending feedback, restart recovery, retry identity, delayed confirmation, duplicate delivery, and isolation of peer drafts and filters.
10. Exercise Add, Complete, Reopen, and Delete from Chrome, iPhone, and Android. Verify each change on both peers.
11. Inspect application sources for each retired mechanism. Moving the same mechanism into another application file fails acceptance.
12. Use the existing Feature 32 screenshots for pending and retry behavior. Record the tested commit, build profiles, commands, and platform results.

Review the production diff and the acceptance evidence before marking this feature complete.
An earlier passing test for a predecessor feature does not establish completion of this redesign.
