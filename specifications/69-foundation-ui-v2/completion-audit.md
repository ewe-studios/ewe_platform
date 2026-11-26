# Specification 69 completion audit

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
