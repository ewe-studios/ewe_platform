# Learnings

- The desktop platform value is in the wire schema. The current native core exposes only iOS and Android.
- The sibling GPUI checkout contains a redirect. Zed and GPUI Fast contain the renderer source.
- GPUI Kit supplies control behavior above GPUI. Foundation still owns frame application, bindings and action traffic.
- The GPUI Fast source and published packages can resolve different graphics code. Validate the source that the release ships.
- The Kit WebView wrapper has platform-specific limits. Keep its evaluation with the existing WebView work.
