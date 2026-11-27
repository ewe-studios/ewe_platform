# Specification 69 completion audit

## Closure checklist, 27 November 2026

The audit uses `resume/combined` at `71466b255`. The specification contains 36 feature documents. Number 18 is reserved.
Use this checklist before the older findings below. Start no new feature until Specification 69 closes.
An open issue can describe a fix already present on this branch. Check the code and test before assigning implementation.
The status below uses source inspection, existing test records, and the open GitHub issues. This audit does not repeat the platform suites.

### Work order

#### Current assignments and next checks

The combined branch includes the resource publication fix at `aac514c9a`.

| Assignment | Current evidence | Next check |
|---|---|---|
| Feature 37 server | The repaired server returns 4,800 complete responses on reused connections. The reactor suite passes on macOS and Linux. | Run the subdirectory fixtures for #839. |
| Browser navigation | Issue #1143 tracks a separate driver error in page event routing and navigation results. | Test two simultaneous tabs, reload, failed navigation, and navigation within one document. Then verify native connection selection for #1062. |
| Android sound | Commit `e9c102726` removes the Todo launcher's unconditional mute flag. A host recording contains the three Mathalley intro notes. Mute reduces the recorded level by about 40 decibels. | Check landscape input with side insets for #843. Then verify the failure banner and Back behavior. Physical speaker output remains unverified. |

The earlier Android surface repair passes two scored game visits on the emulator.
The separate flash report still lacks a confirmed cause.
The browser publication repair passes both build orders and preserves the served files after a rejected build.
Commit `64f5f4fb7` keeps the first socket event during reactor registration. The browser driver has separate verification.
The owner approves Feature 37 steps 3 and 4. Use `pkg/` relative to the application mount for public assets.
Use `public/` on disk. Test the default mount, a prefix-only proxy, a project override, and relocation.
Use each released worker for the next independent check. Record the commit, result, and remaining exceptions at each handoff.
Keep the release consumer checks and all final gates in the work order below.

| Order | Features | Work and exit condition |
|---|---|---|
| 1. Keep Android Mathalley visible and interactive | 34–36 | The owner reports that the game flashes, then disappears. Identify the installed build. Distinguish a process crash, unexpected navigation, and a blank surface. Verify sustained visible play and input on first and repeat launch. This check is incomplete. [#1078](https://github.com/ewe-studios/ewe_platform/issues/1078): retire the old drawing surface in the shared host. Test a second consumer and two game visits in one process. Score on both visits and return to the original list. [#1083](https://github.com/ewe-studios/ewe_platform/issues/1083): test sound enabled and explicitly muted with the shared launcher. Check landscape input with side insets (#843). The earlier startup crash (#1074) has a separate verified fix. |
| 2. Stabilize the browser package | 27, 37 | Fix subdirectory requests (#839). Check the staging collision when a mobile build follows a browser build (#1060). Diagnose the navigation stall (#784). Verify installation under a subdirectory before testing all hosts together. See the two-script decision below. |
| 3. Close delivery and display defects | 11, 23, 25, 32, 33, 37 | Respect explicit dedicated native connections during replay (#1062). Verify failed asynchronous sends (#727), bounded pending reads (#726), and concurrent pending records (#728). Fix unreadable dark rows (#1069) and check the Android failure banner (#849). Reproduce the Android Back timeout (#692) and partial title (#737) before trusting those checks. |
| 4. Verify public consumers | 17, 19, 21, 22, 24 | Check public release inputs (#845) now, while local repairs run. Verify matching Rust and Android packages. Build and run independent Apple and Android starters using published inputs. Record versions and the source commit. Local packages only prove local consumption. Check variant isolation (#800) and the pinned Android tool check (#615). |
| 5. Complete acceptance evidence | 12, 14, 16, 20, 23, 32, 34–37 | Reconcile the unsigned Feature 12, 14 and 16 checks with tests and documented historical exceptions. Verify browser behavior parity and migration fixtures. Check spoken VoiceOver and TalkBack feedback. Record emulator, simulator and physical device coverage separately. Finish the Feature 37 table below. |
| 6. Close the specification | All | Run affected package tests, strict lint, formatting, generation, toolchain checks, and documented examples. Record results for each required operating system. Check the shared frame fixture on Rust, TypeScript, Swift and Kotlin. Reconcile open issues with fixes. Update the feature index and reports before closing. |

**Feature coverage.** The earlier audit records implementation or tests for 01–11, 13, 15, 20–22 and 24–31.
It also records working paths for 17, 23, and 32–35. The table above identifies the remaining checks and shared dependencies.
Feature 18 requires no work. Feature 19 has an external release dependency. Features 36 and 37 remain open.
Feature 32's named-target migration and countdown have later verification. Do not reopen the entire migration from the older audit.
Use the [specification-wide gates](requirements.md#9-success-criteria-spec-wide) for final sign-off.

### Feature 37 acceptance ledger

Numbers follow the [acceptance contract](features/37-todo-server-build-and-state/feature.md#required-verification).

| Checks | Existing evidence | Remaining action |
|---|---|---|
| 1–3. Persistence | Three server processes prove restart, lookup, initial page, deletion, and retry. The store suite also checks the list and key lookup. | Link the [durable server record](progress.md#confirmed-todo-records) in the final report. |
| 4–5. Failure and replay | Tests cover malformed stores, failed writes, cancellation, independent writes, bounded replay, and concurrent snapshots. | Reconcile #728 with native and browser tests. Check the remaining native failure cases in order 3. |
| 6. Independent data service | The seeded fixture and historical Swift and Kotlin calls cover success and missing records. The production server rejects the retired fixture flag. | Link both native runs to the fixture revision. |
| 7. Packaging | The moved development server serves its database and resources. A separate release run serves embedded resources outside the checkout. | Record a clean target build, moved development tree, and standalone Todo release against the final packaging revision. Resolve #1060 first. Verify the [standard paths and project overrides](features/37-todo-server-build-and-state/feature.md#standard-paths-and-project-overrides). |
| 8. Browser and native resources | Existing runs cover one script request, Connect calls, native icon reads, and the packaged game. | Fix #839. Test script and binary requests below a subdirectory, missing build inputs, and the selected output contract. |
| 9–10. Client behavior | Three clients exchange all four mutations at `b3fa90f7a`. Separate suites check retry identifiers, delayed replies, drafts, filters, and duplicate delivery. | Repeat affected paths after transport and packaging fixes. Include the failed-send and dedicated-connection cases. |
| 11. Retired code | Production startup uses the durable store, one static resource handler, and shared browser startup. The old module build script is gone. | Check all retired mechanisms against the final source, including moved wrappers and manual frame handling. |
| 12. Final evidence | Feature 32 contains pending and retry captures. Results and commands span two specifications and several revisions. | Gather the commit, commands, profiles, platforms, captures, and remaining exceptions in one closure record. |

**Browser output decision.** The Feature 37 amendment requests readable and minified application scripts.
The current build emits only `bundle.js`, and the package test requires that exact result.
The page requests `/bundle.js`. Confirm the amendment status before changing the output contract. Fix subdirectory loading under either contract.
See [package output](../../backends/foundation_resources/src/package/browser.rs), [package test](../../examples/ui_todo/tests/server_module.rs), and [page entry](../../examples/ui_todo/web/index.html).

### Issue reconciliation

- Commits `74202df65`, `1a10ee15a`, and `eee45d716` address #837, #879, and #722. All three are ancestors of the audit baseline.
- GitHub still lists those three issues as open. Confirm their existing test records and reconcile the issue states.
- Issues #715, #828, #782, #732, and #714 are closed. Preserve their evidence for relocation, fixture removal, host restart, and native delivery.
- Check graphics shutdown and validation issues #696, #700, #701, #713 and #691 against Features 34 and 35.
- Check migration fixture #712, wire lint #846, replay backlog #548, and missing-clock handling #619 against the final package gates.
- Check the first-install timeout (#831) against the launcher verification. The reviewed launcher and #1074 are already delivered.
- Track server idle activity (#673) and unused Todo server code (#789). Address each if it blocks an acceptance or warning gate.
- Keep unrelated task commands and dependency cleanup outside this queue. Record an external dependency only when a named acceptance check needs it.

## Historical audit

## Scope

The review covers all 35 feature documents in the combined worktree at base commit `279a5fa2b`, including local changes.
Feature 18 reserves a number. Feature 36 implementation uses the Mathalley worktree.
A past result identifies an earlier run. A source check identifies code or a test that exists today.
A passing suite supports only the behavior that its assertions exercise.

## Feature findings

| Feature | Evidence and finding |
|---|---|
| 01. Wire schema | Six schemas, generated bindings, aligned reads, and fixture tests exist in `backends/foundation_ui_wire`. The regenerated files match. |
| 02. Vocabulary | The wire vocabulary has tests for forms, properties, tokens, versions, and behaviors. Browser tests cover control and property mapping. |
| 03. Frame writer | Writer and sequencer tests cover table and column output, order, rejection, and round trips. |
| 04. Signed state | Runtime signer tests cover context binding, expiry, and both signers. The later signer design supersedes the prototype envelope. |
| 05. Action runtime | Runtime registry, manager, receiver, and action flow tests exist. The later context access methods implement the revised design. |
| 06. Action macro | Macro tests include invalid declarations and a consumer with a renamed dependency. |
| 07. Dispatcher | Dispatch tests cover delivery, negotiation, errors, and local dispatch. The repository clock check finds zero violations. The old open-check note is stale. |
| 08. Markup | View lowering and compile-failure tests exercise the shared vocabulary. Feature 32 owns the remaining application target migration. |
| 09. Browser frames | The JavaScript suite exercises frame application, vocabulary mapping, and disposal of a mounted Rust scope. |
| 10. Browser actions | The obsolete Todo browser wrapper is removed. Manager and transport tests exercise scope selection, structured events, and remote requests. The Todo browser test also exercises user input. |
| 11. Live delivery | Live channel tests and Todo routes exist. The fresh shared-connection test exercises replay and independent subscriptions. |
| 12. Legacy removal | The retired Rust operation appears only in an intentional compile-failure fixture. Historical process exceptions remain in the verification record. |
| 13. Native commands | Native core, host, command code, and listener tests exist. Feature 34 adds approved platform graphics bridges. Earlier native tests cover action dispatch. |
| 14. Apple renderer | The integrated Swift package has conformance and behavior tests. Feature 21 supersedes the sibling package path. Section 10 leaves three checks unsigned. |
| 15. Android renderer | The integrated Kotlin packages contain the renderer and tests. Historical emulator checks cover repeated deletion and live counts. Feature 22 changes the location. |
| 16. Behavior matrix | Both native matrix checkers and the Python checker test pass. Section 10 leaves four checks unsigned, including scenario coverage and JavaScript parity. |
| 17. Toolchain checks | The Android version check silently skipped the moved package. Issue 720 tracks the defect. The local correction checks the integrated project and rejects invalid inputs. |
| 18. Reserved | The requirements reserve this number for a later web view control. The count excludes this number. |
| 19. Native releases | Open. The repository has local candidate checks. The public release lookup for `native-ui-v0.1.1` returned no release. Issue 719 tracks version alignment. |
| 20. Producer migration | The specified Rust search finds only the intentional negative fixture. Frame application tests pass. The audit does not repeat all 19 historical browser comparisons. |
| 21. Apple package | The root Swift manifest, integrated renderer, independent starter, and two Todo hosts exist. Earlier simulator runs cover isolated stores and starter refresh. |
| 22. Android package | The Kotlin library, independent starter, and both Todo hosts exist. Earlier emulator and staged repository checks cover consumption. Public publication belongs to Feature 19. |
| 23. Themes | Swift and Kotlin theme tests exist. The feature has 18 mobile captures across two platforms. This audit has no fresh assistive technology walkthrough. |
| 24. Templates | The local platform executable generated all four projects in a fresh directory. Only on-device projects contain a Rust manifest. Public installation still depends on Feature 19. |
| 25. Shared connections | Browser connection tests and the live Todo server test pass. The latter checks shared socket actions and replay through two delivery transports. |
| 26. Connect transport | Exchange tests cover split messages and independent failures. The browser test checks a remote call and zero additional requests when opening a local dialog. Earlier mobile runs cover typed data. |
| 27. Single Todo package | One Cargo package supplies server, browser, and native entry points. Provider tests check service lookup. Feature 32 revises ownership of offline mutations. |
| 28. Typed action input | Browser structured event checks pass. Rust tests cover negotiation and rejection. Swift and Kotlin have structured input senders and tests. |
| 29. Thread-safe signals | Local and shared runtime tests exist. The fresh macOS run includes shared writer, replay, disposal, and concurrency cases. |
| 30. Frame readers and routes | Wire reader tests cover fragmented frames. Response tests exercise lazy encoding, short writes, and sink failures. Todo screen routes call the view helpers. |
| 31. Explicit controls | Browser control tests pass. The document writer checks explicit controls. Earlier Swift and Kotlin checks include keyboard placement and theme captures. |
| 32. Named targets | Partial. The count uses a name. Twelve application constants still assign numeric targets. Issue 714 covers the remaining migration and renderer checks. |
| 33. Replay recovery | Browser tests cover stale deliveries, retry, and two views sharing a connection. The server snapshot race passes. Both native platforms have recovery tests and earlier simulator results. |
| 34. Host graphics | Historical Chrome, iPhone simulator, and Android emulator checks exercise presented pixels, resize, and stale handles. Physical phone coverage remains unverified. |
| 35. Host device handoff | Native lease code and both mobile host graphics suites exist. Earlier three-host runs exercise device handoff. This audit does not repeat every race or physical device check. |
| 36. Mathalley | Active in its own worktree. Chrome opened the game, drew the Train scene, and returned to Todos. Native playback and view isolation checks remain open. |

## Checks from this review

- The JavaScript integration directory passed 157 tests. One optional legacy fixture was absent.
- Both Todo server integration tests passed against a local binary with an explicit browser module.
- Each server test started its own server and store.
- The template smoke check used an existing local executable. It did not rebuild or install the generated applications.
- Android drift checks passed for Gradle 9.9.1 and Kotlin 2.4.21 after the fix.
- Missing files and incorrect Gradle or Kotlin versions each failed the corrected gate.
- The native matrix checkers used the integrated packages. Both checks and the Python checker test passed.

Consult the feature documents and the dated progress entries for earlier platform runs.

The generated wire file check passes with the pinned compiler and frozen JavaScript dependencies.
The combined local and shared signal run passes 49 tests on macOS.

Feature 27 excludes the browser entry module from native builds through an explicit compiler condition.
Feature 06 emits action scope for dispatch. Its macro copies explicit compiler conditions onto the implementation.
A web scope alone leaves the shared handler body available for native compilation.
A native server can implement an action restricted to browser callers.

A fresh Chrome check also found an obsolete cursor lookup in the Feature 12 parity fixture.
Issue 721 tracks that fixture and its browser verification.
