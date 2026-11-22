# Feature 37. Simplify Todo startup, packaging, and state

Status: Required implementation work. This document is the acceptance contract for the Todo redesign.

## Terms

A key-value (KV) store reads and writes records by key.
A universally unique identifier (UUID) identifies one Todo throughout its lifetime.
A snapshot contains confirmed records and their matching stream position.
A resource provider opens a packaged asset by its logical name.
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
Expose one full server constructor that accepts the prepared Todo store and live channel. Its callers supply configuration independently.
The persistent store owns its file path and format. Opening the same store after restart makes previous writes available through KV operations.
Use the store's documented empty state for a new path. Propagate malformed contents and read failures. Preserve the original file on failure.
Keep confirmed server records and idempotency receipts durable. A delayed retry after deletion must never recreate the record.
Define a consistent recovery policy for the stream position and replay history across a restart.
A server with a fresh history must direct stale clients to a matching snapshot.
Retain account and endpoint isolation where hosts configure them.

Remove these paths from production startup:

- Remove `--data-only` and `data_app()`. Move the seeded service and its launcher under `examples/ui_todo/tests/`.
- Keep the real typed data procedure on the full server. Update the iPhone and Android test runners to launch the independent fixture.
- Remove `--restore` and backup parsing from normal startup. Put legacy import and conversion in maintenance tooling.
- Remove `app_with_todos`, `app_with_channel`, and other constructors that only select inputs for the full app.
- Remove the `EWE_TODO_HISTORY` startup branch. Configure replay capacity on the supplied channel. Replay tests construct their own channel.
- Remove `web_module()`, runtime module validation, and `EWE_TODO_MODULE`.

## Build and serve the assets

The app build script calls its service schema generator and one public framework packaging hook.
The hook uses the resource manifest, builds missing browser modules, generates Connect and resource bindings, and stages assets.
Framework providers own tool invocation, change tracking, output directories, validation, and failure reporting.
Use those same providers for command-line tasks. Retire `examples/ui_todo/build-module.sh` and migrate its callers.
A clean Cargo build must prepare the browser application without a separate script or a prebuilt module.
Produce one public JavaScript file, `/bundle.js`, including generated bindings. Preserve addresses for the required WebAssembly binaries.
Use readable JavaScript in development and minified JavaScript in release. Keep intermediate scripts outside the public tree.
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
