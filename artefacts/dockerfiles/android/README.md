# DEPRECATED — Migrated to infrastructure/docker/android/

This standalone Dockerfile built from `ubuntu:24.04` has been replaced by
`infrastructure/docker/android/` which follows the same QEMU base pattern
as macOS, Windows, and ChromeOS (`FROM scratch` + `COPY --from=ewestudios/qemu`).

See `specifications/52-tauri-foundation-platform/features/F39-android-qemu-image/feature.md`
for the feature spec and migration plan.

