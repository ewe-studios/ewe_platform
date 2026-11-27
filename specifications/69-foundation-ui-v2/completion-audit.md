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
| Feature 37 server | The repaired server returns 4,800 complete responses on reused connections. The moved development and standalone release packages pass. The project override uses separate routes. | Complete the native fixtures. |
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

| Check | Evidence | Result |
|---|---|---|
| 1. Start and reopen the store | The moved development program opens a new database, adds a record, and opens the same file in a second process. | Pass |
| 2. Lookup and initial page | The server store test checks the key-value lookup and list. The Chromium fixture checks the saved record on the initial page. | Pass |
| 3. Deletion and original retry | The server store test reopens the database after deletion. A retry of the original Add does not recreate the record. | Pass |
| 4. Storage errors and independent updates | The server tests preserve corrupt inputs and survive failed writes. Seven native host tests and the current two-tab test cover separate pending keys. | Pass |
| 5. Bounded replay and concurrent snapshots | The recovery fixture controls the history size. Store and channel tests check concurrent writers and the matching snapshot position. | Pass |
| 6. Independent typed data | Swift on the iPhone simulator and Kotlin on the Android emulator received the fixture record and its typed NotFound error. | Pass |
| 7. Move both package forms | Cargo creates a development executable with its public tree and a standalone release. Both run in scratch directories outside the checkout. | Pass |
| 8. Resources and failure cases | Chromium checks media, one script, Connect, local actions and named resources. Native bundles open the same icon. Missing inputs preserve the public tree. | Pass |
| 9. Pending and retry behavior | Current Chromium checks reload, independent retries, and countdown. iPhone pending input, lost reply, retry and confirmation passed. | Android-origin peer retry in progress |
| 10. Three-host mutations | Browser and iPhone completed all four mutations in the current synchronized run. Each peer observed those changes. | Repeat Android-origin mutations after #1163 |
| 11. Retired mechanisms | Production source uses one live server, a shared resource mount, and framework browser setup. Seed and replay fixtures remain in tests. | Pass |
| 12. Record and captures | The report includes tested revisions, build profiles, commands and [Feature 32 captures](features/32-portable-view-targets/feature.md). | Add the final three-host result |

See the [confirmed record tests](progress.md#confirmed-todo-records) and the [mounted package results](progress.md#mounted-todo-package).

The approved browser build emits one bundle. A page mounted at `/todo/` requests `/todo/pkg/bundle.js`.
The browser also supports a different asset directory and a separate service address.
See the [asset path contract](features/37-todo-server-build-and-state/feature.md#standard-paths-and-project-overrides).


### Issue reconciliation

- [Issue 837](https://github.com/ewe-studios/ewe_platform/issues/837) remains open. Commit `74202df65` selects the requested library for both Android architectures.
  The staged ARM64 and x86-64 libraries have the expected names and architectures. The installed application still needs a build record that links it to these provider outputs.
  The [native package preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46053814588) schedules the shared provider build and the Android application test.
- [Issue 879](https://github.com/ewe-studios/ewe_platform/issues/879) is closed. Commit `1a10ee15a` restores Android compilation. The full build stages both libraries. See the [Android build record](progress.md#android-client-continuation-compiles).
- [Issue 722](https://github.com/ewe-studios/ewe_platform/issues/722) is closed. Commit `eee45d716` fixes the graphics extent check.
  The macOS Metal test presents a 1206 by 2241 surface, rejects an oversized resize, and presents again on the same owner.
- Issues #715, #828, #782, #732, and #714 are closed. Preserve their evidence for relocation, fixture removal, host restart, and native delivery.
- Check graphics shutdown and validation issues #696, #700, #701, #713 and #691 against Features 34 and 35.
- Check migration fixture #712, wire lint #846, replay backlog #548, and missing-clock handling #619 against the final package gates.
- Check the first-install timeout (#831) against the launcher verification. The reviewed launcher and #1074 are already delivered.
- Track server idle activity (#673) and unused Todo server code (#789). Address each if it blocks an acceptance or warning gate.
- Keep unrelated task commands and dependency cleanup outside this queue. Record an external dependency only when a named acceptance check needs it.

## Unsigned Feature 12, 14, and 16 checks

| Feature and requirement | Evidence | Remaining check |
|---|---|---|
| [12](features/12-legacy-wire-retirement/feature.md#10-done-when). Run all section 9 commands without warnings. | The seven gate checks precede retirement. The current browser suite passes 40 tests on macOS and serial Linux. | Workspace formatting reports existing differences. Run the applicable package checks on Linux, macOS, and Windows under the continuous integration package plan. |
| 12. Four commits and no skipped tests. | The historical record gives more than four commits, 42 ignored tests, and a focused macOS Hypertext Transfer Protocol (HTTP) run. | Keep these as historical exceptions. The original commit count cannot change through another test run. |
| 12. Removed names and specification checklist. | The scan finds a comment and the compile-fail test that names the retired type. The browser runtime scan has no retired parser names. | Record these two scan exceptions during the checklist review. |
| [14](features/14-native-ios-renderer/feature.md#10-done-when). Swift package commands and scenarios. | The integrated Swift host suite passes 25 tests on macOS. The suite checks fixtures, web-member reports, ordering, observation, live mounts, and native commands. | Map each section 7 row to its current test. Check the next valid row after an unknown node and a single delivery of server capabilities to the core. |
| 14. Package build and specification checklist. | The historical iPhone simulator run covers the Todo actions. Feature 21 replaces the sibling package path with the integrated package. | Use the integrated package and current native build tasks for the clean iPhone simulator and starter check. Keep the public starter check under issue 845. |
| [16](features/16-behaviour-matrix/feature.md#10-done-when). All commands and scenarios. | Both platform matrix checkers and the Python test pass. The explicit Node test file patterns pass 175 tests and skip one legacy fixture. | Map the current tests to each section 7 row. Repeat the applicable Rust package and native platform checks in the release lane. |
| 16. JavaScript logic and specification checklist. | All 14 machinery script bodies match across `dd15675cf`. The browser tests compare the four shared behavior bodies with the machinery source. | Record the marker imports, raw string delimiters, and scope cleanup in the review of the original wording. |

The package plan records the Windows browser test exception under [issue 531](https://github.com/ewe-studios/ewe_platform/issues/531).
The Feature 12 record gives the macOS HTTP exception under [issue 546](https://github.com/ewe-studios/ewe_platform/issues/546).
Use the next available release-lane run for the remaining operating-system checks.
The Swift host tests run on macOS. Keep an iPhone simulator run separate from the host results.
The Rust reference suite passes 102 tests with the regenerated search input fixture.
The Swift fixture test and all six Kotlin conformance tests pass with the same files.
[Issue 1158](https://github.com/ewe-studios/ewe_platform/issues/1158) records the fixture repair.
The specification checklist stays unsigned while its scenario coverage review is open.

The retired API compile-fail test passes and reports three unused patch notices from its nested build.
The TypeScript wire reader passes 12 fixtures. The FlatBuffers generation check passes.
The runtime lowering suite passes seven tests. The component matrix suite passes three tests.
The CI planner selects the six requested crates after commit `6cde9ca0c`.
The plan check does not run the outstanding operating-system suites.

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
