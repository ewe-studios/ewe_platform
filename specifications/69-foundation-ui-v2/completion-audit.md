# Specification 69 completion audit

## Remaining closure ledger, 28 November 2026

This review uses `resume/combined` at `67e2c59e0` and the live issue states.
Use the [specification-wide criteria](requirements.md#9-success-criteria-spec-wide) and the evidence below.
The 36 indexed features and their verified scenarios remain in the checkpoints.

### Software and hosted results

| Gate | Remaining proof |
|---|---|
| Affected platform recipes and pinned tools | Apply the [accepted coverage ledger](#completed-baseline-and-scoped-follow-up) to the current revision. Run the outstanding Linux and macOS recipes and the pinned toolchain checks. Match the compiled inputs to each result. The baseline completes with earlier failures. [Baseline run](https://github.com/ewe-studios/ewe_platform/actions/runs/46063284113) |
| macOS Hypertext Transfer Protocol (HTTP) | The local slow event-stream test passes 12 repeats. Local package recipes pass 178 and 181 tests. The combined plan enables both recipes. Collect the hosted result before closing the issue. [Issue 546](https://github.com/ewe-studios/ewe_platform/issues/546) |
| Windows HTTP and browser | Keep the combined exclusions until the portable recipes pass on Windows. HTTP omits the Unix overlay test dependency on Windows. The browser candidate fixes path diagnostics and reads full upgrade headers. Keep Unix server checks in Unix recipes. [Issue 531](https://github.com/ewe-studios/ewe_platform/issues/531) · [Portable recipes](https://github.com/ewe-studios/ewe_platform/issues/1203) · [Browser fixture](https://github.com/ewe-studios/ewe_platform/issues/1210) |
| Windows generated applications | The template candidate passes eight application tests and 12 packager tests on macOS. Verify all eight application tests in the Windows generator shard. The separate packager candidate passes a Windows-target type check. Run its 12 tests on Windows before removing its exception. [Generator issue 1211](https://github.com/ewe-studios/ewe_platform/issues/1211) · [Packager issue 527](https://github.com/ewe-studios/ewe_platform/issues/527) |
| Windows database and Todo server | The direct scratch run proves one Unicode store reopen. Verify the full portable database suite after the binding path repair. The Todo server still needs a Windows HTTP server implementation for a native Windows server run. Keep this boundary visible in the plan. [Database issue 519](https://github.com/ewe-studios/ewe_platform/issues/519) · [Todo issue 1194](https://github.com/ewe-studios/ewe_platform/issues/1194) · [Binding issue 1205](https://github.com/ewe-studios/ewe_platform/issues/1205) |
| Windows differential | The plan passes at `8829fde10`. The separate shards check HTTP, the generator, and the database. The packager and browser fixture revisions need their own recipe results. [Current run](https://github.com/ewe-studios/ewe_platform/actions/runs/46086332741) |
| Hosted setup | Verify the pinned Android sample and the Todo test shard prerequisites at the corrected revisions. Keep the native sample compile, archive, and signature checks in the run. [Android sample issue 1185](https://github.com/ewe-studios/ewe_platform/issues/1185) · [Todo setup issue 1195](https://github.com/ewe-studios/ewe_platform/issues/1195) |
| Application prepare help | The local command lists a web target and an offline option. The preparation task supports Android and iPhone. Correct the help and test the public command. [Issue 1207](https://github.com/ewe-studios/ewe_platform/issues/1207) |

### Owner and external gates

| Gate | Remaining action |
|---|---|
| Public native release and starters, Features 19, 21, 22, and 24 | The owner approves 0.1.2 at the exact candidate revision after credential setup. The release administrator must configure the protected environment and credentials. Name an authorized release reviewer or team. The unsigned preview passes. After publication, verify the public checksums and resolve both clean starters using public sources. [Release prerequisites](features/19-native-package-releases/release-prerequisites.md) · [Issue 845](https://github.com/ewe-studios/ewe_platform/issues/845) |
| Physical iPhone launch and VoiceOver | Verify the first launch, Delete action activation, and countdown announcements on an authorized iPhone. The simulator evidence records splash artwork, speech, and modal focus. [Launch issue 831](https://github.com/ewe-studios/ewe_platform/issues/831) · [Speech issue 1168](https://github.com/ewe-studios/ewe_platform/issues/1168) |
| Physical graphics and sound, Features 34 through 36 | Verify device presentation and transfer of the host texture on physical mobile hardware. Check speaker playback, mute, and interruption. Emulator evidence includes the 2400 by 1080 play and return and about 40 decibels of attenuation. Watch the intermittent focus failure on the next applicable run. [Physical results table](#mobile-release-and-graphics-evidence) · [Issue 1146](https://github.com/ewe-studios/ewe_platform/issues/1146) |

### Recorded historical clauses for owner decision

| Clause | Evidence and decision |
|---|---|
| Feature 12 section 10 requires four commits and no skipped or weakened tests. | The dated record shows more than four commits and 42 ignored tests. The original HTTP verification was focused. Keep the historical facts and record approval for those exceptions. Complete current warning-free affected recipes and preserve both retained textual references to removed types as listed below. [Feature 12 criteria](features/12-legacy-wire-retirement/feature.md#10-done-when) |
| Feature 14 section 10 leaves the command, per-scenario, and story checklist unsigned. | Use the Swift host, integrated simulator, and scenario results below. Feature 21 supersedes the sibling Swift package path. Record the integrated command mapping after the affected verification. [Feature 14 criteria](features/14-native-ios-renderer/feature.md#10-done-when) |
| Feature 16 section 10 requires identical behavior logic and a marker-only source diff. | All 14 JavaScript bodies match across `dd15675cf`. The change also updates imports, string delimiters, and scope. Record explicit acceptance of that larger source diff. Keep the scenario and command checks. [Feature 16 criteria](features/16-behaviour-matrix/feature.md#10-done-when) |
| D48 and the older mobile features describe a rejected second Delete. | Feature 32 makes repeat requests for the same deletion safe. The current browser and Android checks require successful repeat deletion. An unknown record still returns an error. Record the later acceptance contract against the older wording. [D48](requirements.md#2-decisions) · [Feature 32 deletion](features/32-portable-view-targets/feature.md#optimistic-deletion) |

Use the [scenario records](#unsigned-feature-12-14-and-16-checks) with the Story-Driven Specification checklist.
The checklist requires a test with a named observable result for each invariant and scenario.

### Open follow-ups with recorded scope

- Feature 11 defers general snapshot replacement. The Todo replay window is bounded and verified. [Replay scope](#replay-backlog-scope) · [Issue 548](https://github.com/ewe-studios/ewe_platform/issues/548)
- The approved mobile release uses the versioned Git source. Clean public checkout still needs verification. Track the large nested checkout, raw template discovery diagnostic, and composed Android output warning during that proof. [Issue 1200](https://github.com/ewe-studios/ewe_platform/issues/1200) · [Issue 1201](https://github.com/ewe-studios/ewe_platform/issues/1201) · [Issue 1178](https://github.com/ewe-studios/ewe_platform/issues/1178)
- The platform command registry lock and workspace crate publication retain separate gates. The native starter uses the approved Git, Maven, and Swift inputs. [Issue 672](https://github.com/ewe-studios/ewe_platform/issues/672) · [Issue 883](https://github.com/ewe-studios/ewe_platform/issues/883)
- Keep the explicit public-directory handler override and focused Chromium Enter replay follow-ups open. Feature 37 proves the generated asset override and direct browser interaction. [Issue 1156](https://github.com/ewe-studios/ewe_platform/issues/1156) · [Issue 1192](https://github.com/ewe-studios/ewe_platform/issues/1192)
- The strict Foundation WebAssembly default and web gates cover the fixed warnings. The broader embedded runtime testbed matrix belongs to the follow-up. The repository time-source guard has its separate continuous integration issue. [Issue 598](https://github.com/ewe-studios/ewe_platform/issues/598) · [Issue 1094](https://github.com/ewe-studios/ewe_platform/issues/1094) · [Issue 1180](https://github.com/ewe-studios/ewe_platform/issues/1180)
- The workspace dependency check passes without warnings. The manifest removed `reqwest` and `fs_extra` in `4098f8259`. Four workspace packages use `anyhow`. [Issue 596](https://github.com/ewe-studios/ewe_platform/issues/596)

## Earlier closure gates, 28 November 2026

The review covers all 36 features. The package and mobile tables cover 33.
The separate acceptance ledgers cover 14, 16, and 37. Number 18 is reserved.
The review uses the [specification-wide criteria](requirements.md#9-success-criteria-spec-wide).
The checkpoints below this section retain the results and assignments at their recorded revisions.

| Gate | Current evidence and remaining action |
|---|---|
| Todo action examples | Feature 37 passes its twelve checks at `b22d81cd0`. The packaged browser runs the D48 host page script and the shared filter with no server request. Seven native host tests verify the shared filter and exclude the web script and its binding. [Issue 1190](https://github.com/ewe-studios/ewe_platform/issues/1190) |
| Hosted platform and toolchain checks | Collect actual test results and the pinned toolchain check for all supported operating systems. The scoped run uses `89fde9934`. The revision coverage table identifies which recipes need a later result. The older macOS build job failed, and the older Windows check skipped. Track the actual package shards and their setup. [Scoped run](https://github.com/ewe-studios/ewe_platform/actions/runs/46063284113) [Runner issue 1185](https://github.com/ewe-studios/ewe_platform/issues/1185) |
| Configured test exclusions | Keep excluded recipes visible until their issues close and tests pass. The current plan excludes macOS HTTP, Windows browser and HTTP recipes, and Windows database tests. [Issue 546](https://github.com/ewe-studios/ewe_platform/issues/546) [Issue 531](https://github.com/ewe-studios/ewe_platform/issues/531) [Issue 519](https://github.com/ewe-studios/ewe_platform/issues/519) |
| Workspace-wide checks | The workspace formatter passes at `d735d249a`. The local UI lint variants pass at `673e2a7ef`. The repository time-source guard fails in the Linux `foundation_core` build recipe. That recipe is outside the scoped Specification 69 gate. [Formatting issue 1189](https://github.com/ewe-studios/ewe_platform/issues/1189) [Time-source issue 1180](https://github.com/ewe-studios/ewe_platform/issues/1180) |
| Historical criteria | Feature 12 records more than four commits, 42 ignored tests, and reduced HTTP coverage. Feature 16 preserves all 14 logic bodies across `dd15675cf`, with other source edits in the same change. The original commit, no-skip, and marker-only clauses remain unsigned. Record an explicit owner decision before marking those clauses complete. Finish the specification checklist review against the linked scenario ledgers. |
| Apple renderer package | Feature 21 replaces the sibling package path in Feature 14 with the integrated package. The earlier Mac host suite passes 25 tests. The simulator runs both Todo applications. Use the clean starter preview and then verify the approved public package route. |
| Native publication and public starters | Features 19, 21, 22, and 24 share this dependency. The version 0.1.2 preview stages both starters and both Android applications. Release approval and the public artifacts are still required. Resolve clean consumers from public inputs, check versions and digests, and run both generated host modes. Local release tests cover partial publication and conflict handling against a test registry. [Issue 845](https://github.com/ewe-studios/ewe_platform/issues/845) [Candidate run](https://github.com/ewe-studios/ewe_platform/actions/runs/46067419598) |
| Physical iPhone checks | The simulator records speech and first-launch results. Confirm first launch, VoiceOver Delete activation, and countdown cadence on a phone. [Issue 831](https://github.com/ewe-studios/ewe_platform/issues/831) [Issue 1168](https://github.com/ewe-studios/ewe_platform/issues/1168) |
| Native graphics and Mathalley | Features 34 and 35 record simulator, emulator, and desktop hardware results. The mobile records identify the limits of that coverage. Feature 36 still needs physical speaker and interruption checks. Android digital output records the game notes and about 40 decibels of mute attenuation. The old blank surface is fixed. Two full-resolution landscape runs score and return. The original intermittent focus timeout stays under observation. [Surface issue 1078](https://github.com/ewe-studios/ewe_platform/issues/1078) [Focus issue 1146](https://github.com/ewe-studios/ewe_platform/issues/1146) |

The portable-target fixture has results in Rust, TypeScript, Swift, and Kotlin.
The native writer tests cover web message filtering. Live mount tests cover stream and typed data updates.
Swift and Kotlin recover a failed native send and confirm its retry with a correlated reply.
The integrated native socket contract confirms from the server reply after a socket write.

## Earlier closure checkpoint, 27 November 2026

The audit uses `resume/combined` at `71466b255`. The specification contains 36 feature documents. Number 18 is reserved.
Use this checklist before the older findings below. Start no new feature until Specification 69 closes.
An open issue can describe a fix already present on this branch. Check the code and test before assigning implementation.
The status below uses source inspection, existing test records, and the open GitHub issues. This audit does not repeat the platform suites.

### Work order

#### Current assignments and next checks

The following checkpoint includes the native recovery evidence at `2045268eb`.

| Assignment | Current evidence | Next check |
|---|---|---|
| Todo and server | Feature 37 completes all twelve acceptance checks at `b22d81cd0`. Chrome, iPhone, and Android each send four mutations. Both peers receive each change. | Remove server-only build warnings (#789). Check the replay backlog requirement (#548). |
| Browser and Apple | Commit `474404fc2` fixes navigation across attached tabs. Commit `03e5a536f` fixes dedicated native connections. Apple and Android recover a pending change after a dropped socket. | Verify public release consumers (#845). Complete renderer, behavior, and supported operating system checks. |
| Android graphics and sound | The host captures three game notes. Mute reduces the signal by about 40 decibels. The replay confirms a landscape answer at 1560 by 720 and 2400 by 1080. | Monitor the original focus timeout (#1146). Physical speaker output remains unverified. |

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
| 4. Verify public consumers | 17, 19, 21, 22, 24 | Check public release inputs (#845) now, while local repairs run. Verify matching Rust and Android packages. Build and run independent Apple and Android starters using published inputs. Record versions and the source commit. Local packages only prove local consumption. Variant isolation (#800) and the pinned Android tool check (#615) pass locally. |
| 5. Complete acceptance evidence | 12, 14, 16, 20, 23, 32, 34–37 | Reconcile the unsigned Feature 12, 14 and 16 checks with tests and documented historical exceptions. Verify browser behavior parity and migration fixtures. Check spoken VoiceOver and TalkBack feedback. Record emulator, simulator and physical device coverage separately. Finish the Feature 37 table below. |
| 6. Close the specification | All | Run affected package tests, strict lint, formatting, generation, toolchain checks, and documented examples. Record results for each required operating system. Check the shared frame fixture on Rust, TypeScript, Swift and Kotlin. Reconcile open issues with fixes. Update the feature index and reports before closing. |

**Feature coverage.** The earlier audit records implementation or tests for 01–11, 13, 15, 20–22 and 24–31.
It also records working paths for 17, 23, and 32–35. The table above identifies the remaining checks and shared dependencies.
Feature 18 requires no work. Feature 19 has an external release dependency. Feature 36 remains open. The Feature 37 ledger below records its verification.
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
| 9. Pending and retry behavior | Chromium checks reload, independent retries, and countdown. Both native apps show pending and retry for held or failed adds. Peer filter and draft state stay intact. | Pass |
| 10. Three-host mutations | Chrome, the iPhone simulator, and the Android emulator each add, complete, reopen, and delete. Both peers verify each change. | Pass |
| 11. Retired mechanisms | Production source uses one live server, a shared resource mount, and framework browser setup. Seed and replay fixtures remain in tests. | Pass |
| 12. Record and captures | See the [commands and tested revisions](progress.md#feature-37-three-host-verification) and [Feature 32 captures](features/32-portable-view-targets/feature.md). | Pass |

See the [confirmed record tests](progress.md#confirmed-todo-records) and the [mounted package results](progress.md#mounted-todo-package).

The approved browser build emits one bundle. A page mounted at `/todo/` requests `/todo/pkg/bundle.js`.
The browser also supports a different asset directory and a separate service address.
See the [asset path contract](features/37-todo-server-build-and-state/feature.md#standard-paths-and-project-overrides).


### Issue reconciliation

- [Issue 837](https://github.com/ewe-studios/ewe_platform/issues/837) is closed. Commit `74202df65` selects the requested library for both Android architectures.
  The [native package preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46060536936/job/140713453224) builds both architectures, assembles the applications, installs them and passes both initial screen replays.
  [Issue 1181](https://github.com/ewe-studios/ewe_platform/issues/1181) tracks the later instrumentation result check.
- [Issue 879](https://github.com/ewe-studios/ewe_platform/issues/879) is closed. Commit `1a10ee15a` restores Android compilation. The full build stages both libraries. See the [Android build record](progress.md#android-client-continuation-compiles).
- [Issue 722](https://github.com/ewe-studios/ewe_platform/issues/722) is closed. Commit `eee45d716` fixes the graphics extent check.
  The macOS Metal test presents a 1206 by 2241 surface, rejects an oversized resize, and presents again on the same owner.
- Issues #715, #828, #782, #732, and #714 are closed. Preserve their evidence for relocation, fixture removal, host restart, and native delivery.
- Issues #696, #700, and #701 are closed. The Linux graphics process exits cleanly at default concurrency, and strict Clippy passes.
  The browser graphics test drains the shared and canvas queues on the Apple Metal adapter.
- [Issue 691](https://github.com/ewe-studios/ewe_platform/issues/691) is closed. The Android emulator and Linux fixtures accept Vulkan aggregate records and reject a missing feature.
  These fixtures use device creation records without creating a physical device grant.
- [Issue 713](https://github.com/ewe-studios/ewe_platform/issues/713) is closed. The current Linux native package passes strict Clippy and all 11 default integration tests.
- The external Cargo target fixture reports the exact missing migration number. [Issue 712](https://github.com/ewe-studios/ewe_platform/issues/712)
- The subscription constructors pass strict Clippy. All 96 wire tests pass. [Issue 846](https://github.com/ewe-studios/ewe_platform/issues/846)
- The old failure limiter is absent from the active runtime. The signer reports missing host services through typed errors. [Issue 619](https://github.com/ewe-studios/ewe_platform/issues/619)
- The pinned Android sample compiles Kotlin, builds the debug package, and passes manifest, archive, and signature checks. [Issue 615](https://github.com/ewe-studios/ewe_platform/issues/615)
- A fresh simulator install shows the launch artwork under debugger pause and the ready Todo view on normal launch.
  Issue 831 stays open for a physical iPhone repeat. No physical iPhone is available to this check. [Issue 831](https://github.com/ewe-studios/ewe_platform/issues/831)
- The HTTP server closes an idle connection when the peer closes input. The real server stays idle after a starter Refresh.
  The server-only Todo build now has no compiler warnings. [Issue 673](https://github.com/ewe-studios/ewe_platform/issues/673) [Issue 789](https://github.com/ewe-studios/ewe_platform/issues/789)
- The [replay backlog scope](#replay-backlog-scope) explains the deferred general snapshot requirement.
- Keep unrelated task commands and dependency cleanup outside this queue. Record an external dependency only when a named acceptance check needs it.

## Unsigned Feature 12, 14, and 16 checks

| Feature and requirement | Evidence | Remaining check |
|---|---|---|
| [12](features/12-legacy-wire-retirement/feature.md#10-done-when). Run all section 9 commands without warnings. | The seven gate checks precede retirement. The current browser suite passes 40 tests on macOS and serial Linux. | The workspace formatter passes at `d735d249a`. The hosted package checks continue. [Issue 1189](https://github.com/ewe-studios/ewe_platform/issues/1189) |
| 12. Four commits and no skipped tests. | The historical record gives more than four commits, 42 ignored tests, and a focused macOS Hypertext Transfer Protocol (HTTP) run. | Keep these as historical exceptions. The original commit count cannot change through another test run. |
| 12. Removed names and specification checklist. | The scan finds a comment and the compile-fail test that names the retired type. The browser runtime scan has no retired parser names. | Record these two scan exceptions during the checklist review. |
| [14](features/14-native-ios-renderer/feature.md#10-done-when). Swift package commands and scenarios. | The integrated Swift host suite passes 25 tests on macOS. The suite checks fixtures, web-member reports, ordering, observation, live mounts, and native commands. | The focused host probe applies the next valid row after an unknown node and delivers the capabilities to the core once. Review the package commands and the specification checklist. |
| 14. Package build and specification checklist. | The iOS 27 simulator runs both current Todo applications through the integrated package. Feature 21 replaces the sibling package path. | Verify the clean starter with the pending candidate. Keep the public starter check under issue 845. |
| [16](features/16-behaviour-matrix/feature.md#10-done-when). All commands and scenarios. | Both platform matrix checkers and the Python test pass. The explicit Node test file patterns pass 175 tests and skip one legacy fixture. The Kotlin core suite passes 20 tests, including both unknown behaviour forms. | Repeat the applicable package checks in the release lane. |
| 16. JavaScript logic and specification checklist. | All 14 machinery script bodies match across `dd15675cf`. The browser tests compare the four shared behavior bodies with the machinery source. | The change includes marker imports, raw string delimiters, and scope cleanup. The original marker-only condition stays unsigned. |

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
The scoped plan waits for hosted operating-system suites.

| Scenario group | Current checks |
|---|---|
| 14. Fixtures, columns, unsupported rows, and malformed frames | `ConformanceTests.testWireFixtures` compares the native snapshots. `testAllWebRowsAreReported` covers the web rows. `RendererTests.testMalformedFramesAndUnknownNode` covers rejected input and unknown nodes. The focused host probe checks the next valid row. |
| 14. Synchronous updates, ordering, observation, and live mounts | `RendererTests.testOrderingAndObservation` checks immediate answers and one observed record. `testGapReconnectsAndRemovalCloses` checks the resume cursor, channel removal and local mounts. |
| 14. Events, capabilities, native commands, behaviours, and view construction | `RendererTests.testEventUsesStateOnControlAndReportsFailure` checks event state and errors. The focused host probe checks one capabilities callback. `MobileOperationTests.testNativeCommandsWebViewAndCustomBehaviour`, `BehaviourTests.testUnknownBehaviourRetainsItsNumericID`, and `RendererTests.testEveryCoreViewLaysOut` cover native commands, behaviour reports and view layout. |
| 16. Matrix, identifiers, lowering, and version gate | `every_behaviour_and_component_matches_the_shipped_matrix`, `component_lists_match_machinery_calls`, `scoped_script_and_behaviours_belong_to_the_parent`, and `version_1_client_gets_no_behaviour` check the current declarations and wire output. |
| 16. Platform dispatch and known or unknown bindings | The writer checks native and web fields. The browser all-forms fixture reports all three mobile fields. The browser behaviour tests cover registry lookup and cleanup. Swift and Kotlin conformance fixtures store known behaviours. `testSharedBindingsAndSettings` and `standardBehavioursReachCompose` check the bindings. `RuntimeTest.unknownBehaviourReportsPreserveNamesAndApplyTheNextRow` checks both unknown forms in Kotlin. |

## Revision coverage after the scoped run

The scoped run uses commit `89fde9934`. This comparison uses commit `ed0b32817`.
[Scoped operating-system run](https://github.com/ewe-studios/ewe_platform/actions/runs/46063284113)

| Change after the tested revision | Coverage record |
|---|---|
| HTTP connection handling and wire subscription initialization | The affected Cargo recipes need results from the new source. Keep the HTTP platform exceptions. |
| Standalone browser runtime | The new runtime affects browser and resource consumers. The new testbed staging assertion has a focused two-test result. Windows testbed tests have exception 531. |
| iPhone keyboard and Android replay assertions | The targeted simulator and emulator checks pass. The native preview exercises both changes on hosted runners. |
| Documentation and the browser test task description | The test command and tool versions stay the same. The package tests can use earlier evidence. |

For each platform, check the configured recipe with the Cargo dependency tree.
Use its normal, build and development dependencies with that platform's target.
The compact and UI traits recipes have unchanged inputs. Reuse their successful test steps.
The macro changes now enter the configured database and reactor recipes.
The setup changes affect the Todo package shards and the separate macOS sample command.

The current changed package set adds macros, the project generator and the testbed.
Keep the platform exceptions in the package plan.
The follow-up applies only to missing test evidence after the first run reports its results.

The baseline macOS shards pass 11 packages. The runtime guide and Todo package fail.
Issue 1182 fixes the missing macro import. Issue 1195 adds the Todo binding program to the shard.
The Windows native task fails on the staged template path. Issue 1193 changes the path join.
The Windows Todo example fails in the old database dependency. Issue 1194 tracks the compatibility preview.
The libSQL 0.10 candidate stops in the generated bindings path on Windows. Both Todo and vector tests remain pending. [Issue 1205](https://github.com/ewe-studios/ewe_platform/issues/1205)
The follow-up stops before its four local libSQL test targets. Ordinary Cargo passes one Unicode reopen test in a scratch checkout. The full Windows database exclusion stays in effect under issue 519. [Windows diagnostic run](https://github.com/ewe-studios/ewe_platform/actions/runs/46080239956)

## Completed baseline and scoped follow-up

The run at `89fde9934` completes with package and setup failures.
Linux runs tests for eight packages. Six packages stop before their tests.
macOS passes 11 packages and fails two. Windows passes six and fails five.
[Completed baseline](https://github.com/ewe-studios/ewe_platform/actions/runs/46063284113)

| Gate | Current evidence |
|---|---|
| Linux components and resource lint | The component fix passes strict Clippy and 56 tests on Linux. The resource repair passes both strict variants and the Cargo hook regressions. |
| Linux wire and browser UI package | The baseline stops at formatting and a panic documentation lint. The fixes are in `5ae7b4aa0` and `673e2a7ef`. Local Linux checks run the current formatting, Clippy, and test commands. |
| Linux and macOS Todo and runtime | The follow-up uses product revision `40a3f13a2`. It also selects Linux resource tests, macOS HTTP tests, and the macOS Android sample. |
| Windows native, runtime, and wire | The current Windows shards pass the 29 native integration tests, 82 runtime integration tests, and 12 runtime documentation tests. The wire default and reference runs pass 96 and 102 tests. |
| Windows database, HTTP, and generator | The current follow-up tests the corrected caller path, four local database targets, both HTTP recipes, and the application generator. Windows full server and vector tests remain separate. |

[Linux and macOS follow-up](https://github.com/ewe-studios/ewe_platform/actions/runs/46088801494) · [Windows native, runtime, and wire](https://github.com/ewe-studios/ewe_platform/actions/runs/46080239956) · [Windows database, HTTP, and generator](https://github.com/ewe-studios/ewe_platform/actions/runs/46086332741)

## Apple screen reader checks

The Todo app on the iOS 27 simulator uses VoiceOver speech through Xcode 27.2.
The result includes Add, selected filters, Complete, selected Reopen, and the Todo title.
VoiceOver reaches Delete in the row's Actions menu. The simulated double tap does not open the confirmation.
The pause test checks the title, remaining seconds, modal focus, and Input resumed.
A physical iPhone check remains for Delete activation and countdown announcement cadence. [Issue 1168](https://github.com/ewe-studios/ewe_platform/issues/1168)


## Package examples and parity evidence

The [scoped hosted run](https://github.com/ewe-studios/ewe_platform/actions/runs/46063284113) is pinned to `89fde9934`.
The package names below identify its planned jobs.
The nightly configuration covers packages outside the scoped run.
The hosted plan passes. The example checks below include local results and earlier feature records.

| Feature | Example or parity evidence | Package gate |
|---|---|---|
| 01. Wire schema | The standalone text update runs. The generated bindings match. The TypeScript reader passes all 12 wire fixtures. | The scoped wire job covers the default and reference runs. |
| 02. Vocabulary | The wire guide's vocabulary example runs. The JavaScript vocabulary file passes its syntax check. | The scoped wire, browser UI, and component jobs cover the vocabulary consumers. |
| 03. Frame writer | The wire guide's writer example writes a text reply and an aligned batch of three rows. | The scoped wire job covers writer tests and the WebAssembly build. |
| 04. Signed state | The [runtime guide](../../backends/foundation_ui_runtime/README.md) gives a readable counter and a private counter. Both counters pass. Both WebAssembly host probes pass. | The scoped runtime, wire, and compact jobs cover the signer and its host inputs. |
| 05. Action runtime | The runtime guide passes 12 documentation tests, including the search input change binding. The native integration run covers local and remote dispatch. | The scoped runtime, native, and Todo jobs cover those owners. |
| 06. Action macro | The renamed action fixture builds for native and WebAssembly. The macro tests check invalid declarations. | The scoped runtime suite covers its fixture. The macro crate uses the nightly job and has the Windows exception in issue 531. |
| 07. Dispatcher | The runtime and native suites cover capabilities, routing, action errors, and returned frames. The clock parameter guard reports zero violations. | The scoped runtime and native jobs cover dispatch. The repository time source guard reports two agent shutdown calls outside this specification. [Issue 1180](https://github.com/ewe-studios/ewe_platform/issues/1180) |
| 08. Markup | The runtime and component guides show shared controls, reactive conditions, and named placement. The retained component checks compare shared and browser behaviour calls. | The scoped runtime, browser UI, and component jobs cover their consumers. The macro package uses the nightly job. |
| 09. Browser frames | The current browser tests compare Rust row and column fixtures and read every supported message form. The standalone import and both testbed integration tests pass at `b6a9a4e78`. [Issue 1177](https://github.com/ewe-studios/ewe_platform/issues/1177) | The scoped browser UI and browser driver jobs cover the earlier pinned source. The testbed crate uses the nightly job. |
| 10. Browser actions | The browser UI guide shows manager setup and a local or remote action. The current JavaScript suite checks frame requests and explicit transport selection. | The scoped browser UI, runtime, browser driver, and Todo jobs cover the action paths. |
| 11. Live delivery | The browser UI guide shows a mount and its resume cursor. The live fixtures exercise independent subscriptions. Android and iPhone retain a pending action after a socket closes. | The scoped browser UI, browser driver, and Todo jobs cover live fixtures. The core Wasm package uses the nightly job. |
| 12. Legacy removal | The negative fixture rejects the retired API. The previous seven parity checks ran before removal. The guide links to that record and runs the retained fixtures. | The scoped run includes the main UI packages. The legacy retirement checklist keeps its recorded command and history exceptions. |
| 13. Native commands | The native guide covers local actions, a host request, and the core builder. The current Linux suite passes 11 tests, including command helpers. | The scoped native job uses default, app, and tasks runs. Android and iPhone evidence stays separate. |
| 17. Toolchain | The [current toolchain guide](../../scripts/toolchain/README.md) invokes the Rust tasks. Local Apple and Android scope checks pass. | The hosted plan and platform setup run the pinned tools. The maintained shell fixtures invoke the current Rust tasks. |
| 20. Producer migration | The custom protocol example runs. The renamed reactive fixture builds. The wire suite passes 92 tests with default features disabled. Issue 629 covers detachment in the pre-switch Chrome record. | The scoped wire, runtime, browser UI, and browser driver jobs cover the current consumers. |
| 27. Single Todo package | The Todo guide records one package, its staged files, and native startup. Feature 37 proves browser, iPhone, and Android mutations on a shared server. | The scoped Todo, resource, native, and browser UI jobs cover those package owners. Public consumption remains under issue 845. |
| 28. Typed input | The native Linux suite forwards structured submission to local and remote handlers. Runtime and browser tests check rejected input and the selected encoding. | The scoped native, runtime, wire, and browser UI jobs cover those tests. |
| 29. Thread-safe signals | The [runtime selection guide](../../backends/foundation_signals/README.md#choose-a-runtime) describes local and shared runtimes. The feature record covers all four macOS configurations. | The signals nightly job declares all four configurations. The scoped runtime and browser UI jobs cover their consumers. |
| 30. Readers and routes | The wire guide shows file and asynchronous readers. The route guides explain lazy render and streaming. The current Todo peer run exercises live replay. | The scoped wire, runtime and Todo jobs cover their owners. The direct UI HTTP route crate uses the nightly job. |
| 31. Explicit controls | The wire guide shows a complete form and a confirmation. The retained browser tests render the controls. The feature record keeps the approved device captures. | The scoped wire, browser UI, native, component, and Todo jobs cover their package tests. |

The exact retired wire comparisons stay in the historical feature records.
Physical device checks require their own results.


## Mobile, release, and graphics evidence

| Feature | Evidence | Remaining gate |
|---|---|---|
| 15. Android renderer | The Kotlin conformance reader matches six shared fixtures. The Android 36 emulator verifies repeated delete, and Feature 37 verifies the live list and actions. | The native preview checks the packaged renderer. |
| 19. Native releases | The version 0.1.2 preview builds both starters on Apple and installs both Todo applications on Android. [Hosted preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46060536936) | Public registry and release artifacts were absent at the dependency check. Publication requires release approval and the public consumer replay. [Issue 845](https://github.com/ewe-studios/ewe_platform/issues/845) |
| 21. Apple package | The Apple preview builds the staged package and independent starters. The iPhone simulator runs offline and server Todo paths. | Public resolution and the physical device checks have their own release gates. |
| 22. Android package | The preview uses native provider outputs for both Android architectures. It assembles, installs, and checks both Todo applications. [Issue 837](https://github.com/ewe-studios/ewe_platform/issues/837) | Resolve the starter from the public Maven package after publication. |
| 23. Themes | The feature record keeps the mobile theme captures. The Android recording checks Add, filters, Complete, Reopen, and Delete with TalkBack. The iPhone recording checks the spoken modal and resume controls. | Confirm physical VoiceOver Delete activation and countdown cadence. [Issue 1168](https://github.com/ewe-studios/ewe_platform/issues/1168) |
| 24. Templates | The platform generator produces four projects. Both generated on-device counters advance with networking disabled. The package-only starters run against the staged packages. | Verify a clean public install with the release packages. |
| 25. WebSocket actions | The independent native socket check exchanges text and binary messages and closes its socket. The action socket still receives the rendered count. Both native apps recover a dropped Add. | The preview checks the packaged native hosts. |
| 26. Connect transport | The transport tests check split exchanges and independent failures. The browser sends its remote call without a request for the local dialog. Both native Feature 37 fixtures receive the typed value and error. | The hosted native and browser UI jobs cover the consumers. The direct transport package uses its nightly job. |
| 32. Named targets | The Todo migration uses named targets. The Android test checks countdown focus, a live peer update during the pause, persistence across restart, and resumed input. [Issue 714](https://github.com/ewe-studios/ewe_platform/issues/714) | Physical speech verification stays with issue 1168. |
| 33. Replay recovery | Chrome, the iPhone simulator, and the Android emulator recover a discarded history with one replacement snapshot. All three apply the next committed update. The native apps also recover a live socket drop and confirm the pending action. | The hosted browser, native, and Todo package jobs cover their respective recovery tests. |
| 34. Rust-owned graphics | The Android emulator and iPhone simulator present two targets. The iPhone test rejects an oversized extent before the next valid presentation. Chrome verifies rendered pixels on the Apple Metal 3 adapter. | Physical phone presentation still needs a device. |
| 35. Host-owned graphics | The iPhone simulator and Android emulator process the host texture on its original device. Chrome delays the completion fence. The Linux graphics tests cover cancellation, lost devices, stale generations, and returned leases. | A physical mobile device grant still needs its own run. |
| 36. Mathalley | Chrome, the iPhone simulator, and the Android emulator verify input and Todo updates during play. The Android digital output contains the game notes and measures about 40 decibels of attenuation when muted. The 2400 by 1080 replay scores and returns to Todos. | The intermittent Android focus timeout remains under issue 1146. Physical speaker playback and interruption checks still need a phone. |

The mobile graphics rechecks each pass three tests. The Android recheck uses the previously built isolated framework package.
The iPhone simulator recheck uses the current Swift fixture and the staged native archive.
The [current preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46067419598) covers the next native candidate.

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

### Replay backlog scope

Feature 11 permits a configured frame limit and keeps an unbounded default.
Section 4.3 defers snapshot replacement for streams that build a page from all received frames.
Invariant I9 keeps the previous Broadcaster behavior.
[Feature 11 requirements](features/11-live-mounts-and-server-streams/feature.md#43-must-have)

The Todo replay fixture injects a frame limit.
The live channel tests exercise typed expiry and replay after a new snapshot.
Feature 37 repeats synchronized delivery with a bounded channel.
[Todo acceptance](#feature-37-acceptance-ledger)

The legacy stream still retains all frames.
Issue 548 remains open for general snapshot replacement.
[Issue 548](https://github.com/ewe-studios/ewe_platform/issues/548)

### Strict Todo lint

The Feature 37 server and nested browser builds pass strict Clippy on macOS.
The same application changes pass 14 server, seven native, and three Chromium tests.
The native tests use the host core. The Chromium tests use the running Todo server.
The three-operating-system package test run has its own pinned revision in this audit.
[Todo lint results](progress.md#strict-todo-lint-check)


### Android template variant isolation

The generated on-device counter packages debug and release in both Gradle task orders.
Each package contains the x86-64 library for its own build type.
The native provider fixture also passes for the legacy path and preserves outputs after a failed rebuild.
The Android library inputs come from a scratch publication of the current source.
Feature 19 retains the public package installation gate.
[Android variant results](progress.md#android-template-build-variants)


### Physical device checklist for Todo

The available device inventory contains five iPhone simulator entries and one Android emulator.
No physical test device is available.

- Use the [physical Todo handoff](features/23-native-design-and-themes/feature.md#physical-iphone-todo-handoff) for launch and VoiceOver Delete. It covers issues 831 and 1168.
- Use the [physical countdown handoff](features/32-portable-view-targets/feature.md#physical-iphone-countdown-handoff) for speech and focus in issue 1168.
- Use the [physical sound handoff](features/36-mathalley-game-view/feature.md#physical-phone-sound-handoff) for speaker playback, interruptions and close behavior.

### Candidate and local package coverage

The native preview at `8ce2352b9` builds candidate starters and the separate Todo applications.
It precedes the current Android template split and the Todo lint and header changes.
The local Android package test covers both build orders with the updated template and provider.
The local Todo checks cover the lint and header changes.
The public package gate remains in Feature 19.
[Native preview](https://github.com/ewe-studios/ewe_platform/actions/runs/46067419598)

### Native release prerequisites

The generated mobile application now uses one native release Git source.
The scratch resolver checks the native and mixed browser locks and their feature sets.
A normal clone produces all 26 archives in the former registry resource graph.
The archive consumer stages Android launch and application assets.

The 0.1.2 publication approval, protected environment, and public starter checks remain pending.
The scratch mirror shows source resolution. The full checkout cost and public tags have separate evidence.
[Exact crate inventory and release steps](features/19-native-package-releases/release-prerequisites.md)
