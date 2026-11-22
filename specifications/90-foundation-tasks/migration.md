# Task migration inventory

| Owner | Current callers | Task entry |
|---|---|---|
| Native package | `scripts/ui_native/build-android.sh`, `build-ios.sh`, and native release preparation | `native:build-android`, `native:build-ios` |
| Native bindings | `generate-bindings.sh`, `sync-android.sh`, and `sync-ios.sh` | `native:bindings:generate`, `native:bindings:sync-android`, `native:bindings:sync-ios` |
| Native examples | Todo build and test scripts under `scripts/ui_native` | `native:todo:prepare-ios`, `native:todo:test-ios`, `native:todo:test-android` |
| Native release | Native release workflow, package release guide, and scripts under `scripts/ui_native/release` | `native:release:prepare`, `native:release:verify`, `native:release:publish` |
| Starter applications | Apple and Android independent application checks under `scripts/ui_native/release` | `native:release:test-ios-starter`, `native:release:test-android-starter` |
| Wire package | `scripts/ui_wire` and its repository automation callers | Future wire package set |
| Toolchain | `scripts/toolchain` and mise setup and check entries | Future toolchain set |
| Continuous integration | `scripts/ci`, `ci/packages.toml`, and workflow entry points under `.github/workflows` | Future build and verification sets |

The central `platform` executable and the package's `native-task` executable register the same native task set.
Existing release workflow script paths remain in service while platform parity is verified.
