# Learnings

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
