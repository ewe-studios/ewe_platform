# Feature 21 progress

- The root package exposes the public `EweUI` product.
- The examples have distinct schemes and application identifiers.
- The starter depends on the public product. The todo apps also link their generated application bridge.
- The renderer supports an inset list, a segmented filter, a form sheet and a confirmation alert.
- The root Swift test run passed 11 tests, including replay of both presentation property encodings.
- Both apps passed their interface tests on the same iPhone 18 Pro simulator.
- Todo On Device added, completed, filtered, reopened and deleted a task while the server was stopped.
- Todo Server added, completed and reopened a task. A second client read its task from the server.
- The unsigned build for a physical iPhone passed. Signing and physical installation require a development team.
- The [simulator screenshot](../19-native-package-releases/design/todo-after.png) shows the updated screen.
- Exact tag installation requires the first public package release.

The full on-device test also passed with dark appearance and accessibility-extra-large text.
VoiceOver exposed the form field, Add and Cancel with their control types and labels.
