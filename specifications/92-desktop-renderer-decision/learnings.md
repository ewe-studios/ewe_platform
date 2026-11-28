# Learnings

## Upstream GPUI counter

Start with the [code walkthrough](gpui-counter-walkthrough.md) to build the example and trace an input to the next frame.
The [standalone example](../../examples/gpui/README.md) has its own lockfile and uses one official Zed revision for both GPUI packages.

- Use the LLVM `uat` profile from the start for native code and for any code Cranelift cannot compile. [Repository build settings](../../mise.toml#L29)
- The `Counter` entity holds the count and focus handles. Each render creates named elements from the current state.
- The button's pointer and keyboard handlers call the same state update. GPUI also exposes the pointer handler as an accessibility action.
- The macOS check verifies launch, pointer and keyboard input, accessibility activation, and reset on relaunch. [Captured window](features/01-backend-evaluation/screenshots/gpui-counter-macos.jpg)
- VoiceOver, the window startup error, Windows, Linux, and installation remain open. [Verification record](features/01-backend-evaluation/feature.md#macos-counter-result)

## Earlier desktop findings

- The desktop platform value is in the wire schema. The current native core exposes only iOS and Android.
- The sibling GPUI checkout contains a redirect. Zed and GPUI Fast contain the renderer source.
- GPUI Kit supplies control behavior above GPUI. Foundation still owns frame application, bindings and action traffic.
- The GPUI Fast source and published packages can resolve different graphics code. Validate the source that the release ships.
- The Kit WebView wrapper has platform-specific limits. Keep its evaluation with the existing WebView work.
- Zed GPUI and GPUI Fast have platform accessibility bridges. Base supplies accessibility roles and actions for controls.
- The Iced checkout and its local release tag have text composition support. The reviewed source has no widget accessibility bridge.
- Iced development uses a Git windowing dependency and wgpu 30. The reviewed release tag uses registry windowing and wgpu 27.
- Base is a standalone crate. The prototype can use Base behavior with Foundation styles and its own desktop host.
- Tauri can reuse the Foundation browser frame applicator and action manager. The Web mount has its own operation vocabulary.
- The Tauri lockfile pins Rust wrapper versions. The WebView runtime and its accessibility and graphics behavior follow the target system.
- Bundled web assets enable an offline launch. A clean Windows install also needs an offline WebView2 plan.
- The standalone Tauri shell can load the Todo service without a browser build inside the shell or a native command bridge.
- A Todo development executable needs its matching staged browser resources. A cached executable can start when its browser bundle is absent.
- The checked macOS WebView draws the Mathalley Train round and returns to the same Todo filter.
- The GPUI Fast source uses the package names `gpui` and `gpui_platform`. Its compatibility packages use other names and versions.
- GPUI Kit Base selects registry snapshot `0.3.14` by default. The toolkit example uses that snapshot with the reviewed Base source.
- The current terminal wizard prints Desktop as unavailable. The target enum lists Web, Android and iOS.
- Web generation embeds its templates in the project crate. Its counter already calls shared Rust application logic.
- The current run operation starts a mobile device. Desktop launch requires a route that runs on the current host.
- The planned upstream GPUI example uses official Zed source. Its pin declares Rust `1.100.0`.
- Upstream marks its GPUI crates Apache-2.0. Review transitive licenses and assets before distribution.
- GPUI Kit and GPUI Fast findings record historical research. The current generator selects Tauri or upstream Zed GPUI.
