# LEARNINGS — Manifest Installer

## Split from spec 65 (2026-11-14)

The installer is one mission: read a manifest, install and start each entry, record the result,
and make a second run safe. It does not own the manifest format. It does not own health. It does
not own Cloudflare.

Three boundaries matter:

1. **The manifest contract is spec 78.** The installer parses it with its own types. A field
   change is a change to 78 and to the consumer's fixtures.
2. **Health is spec 82.** The installer proves that a program started. The registrar proves that
   it is healthy.
3. **The registrar finds entries by marks, not by the install record.** The install record is
   private to the installer. The marks (`EWE_ENTRY`, `EWE_VERSION`, Docker labels) are the
   public contract.

## Reuse

- `foundation_packager` renders the systemd unit and the bootstrap script. Do not hand-build
  strings.
- `foundation_deployment_docker` does all Docker work with generated types. Do not build a
  `docker` command string.
- `InstallContext` injects the HTTP client, command runner, Docker client, clock and state
  directory. No adapter reads a global.
